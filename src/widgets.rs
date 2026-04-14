use std::fs;
use std::path::Path;
use std::process::Command;

const GIT_SECTION_STUB: &str = "#[fg=colour142]▒  main";
const FORGE_SECTION_STUB: &str = "#[fg=colour214]▒  --";
const SHOW_FORGE_SECTION: bool = false;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitSnapshot {
    pub branch: String,
    pub changed_count: u32,
    pub insertions_count: u32,
    pub deletions_count: u32,
    pub untracked_count: u32,
}

pub fn git_section_string() -> String {
    current_git_snapshot()
        .map(format_git_section)
        .unwrap_or_else(|| GIT_SECTION_STUB.to_string())
}

pub fn forge_section() -> &'static str {
    if SHOW_FORGE_SECTION {
        FORGE_SECTION_STUB
    } else {
        ""
    }
}

pub fn metrics_section() -> &'static str {
    ""
}

pub fn metrics_section_string() -> String {
    let cpu = cpu_percent().unwrap_or(0);
    let ram = ram_percent().unwrap_or(0);

    format!(
        "#[fg=colour109]▒ 🧠 {cpu}% #[fg=colour108]💾 {ram}%"
    )
}

pub fn current_git_snapshot() -> Option<GitSnapshot> {
    let repo_root = command_output("git", &["rev-parse", "--show-toplevel"])?;
    let repo_root = repo_root.trim();
    if repo_root.is_empty() {
        return None;
    }

    let branch = command_output_in_dir(repo_root, "git", &["branch", "--show-current"])?;
    let branch = branch.trim();
    if branch.is_empty() {
        return None;
    }

    let diff_numstat = command_output_in_dir(repo_root, "git", &["diff", "--numstat"])?;
    let (changed_count, insertions_count, deletions_count) = parse_diff_numstat(&diff_numstat);

    let untracked_output = command_output_in_dir(
        repo_root,
        "git",
        &["ls-files", "--other", "--directory", "--exclude-standard"],
    )?;
    let untracked_count = untracked_output.lines().filter(|line| !line.trim().is_empty()).count() as u32;

    Some(GitSnapshot {
        branch: truncate_branch(branch),
        changed_count,
        insertions_count,
        deletions_count,
        untracked_count,
    })
}

pub fn format_git_section(snapshot: GitSnapshot) -> String {
    let mut section = format!("#[fg=colour142]▒  {}", snapshot.branch);

    if snapshot.changed_count > 0 {
        section.push_str(&format!(" #[fg=colour214] {}", snapshot.changed_count));
    }

    if snapshot.insertions_count > 0 {
        section.push_str(&format!(" #[fg=colour107] {}", snapshot.insertions_count));
    }

    if snapshot.deletions_count > 0 {
        section.push_str(&format!(" #[fg=colour167] {}", snapshot.deletions_count));
    }

    if snapshot.untracked_count > 0 {
        section.push_str(&format!(" #[fg=colour223] {}", snapshot.untracked_count));
    }

    section
}

pub fn cpu_percent() -> Option<u8> {
    match std::env::consts::OS {
        "linux" => linux_cpu_percent(),
        "macos" => macos_cpu_percent(),
        _ => None,
    }
}

pub fn ram_percent() -> Option<u8> {
    match std::env::consts::OS {
        "linux" => linux_ram_percent(),
        "macos" => macos_ram_percent(),
        _ => None,
    }
}

fn linux_cpu_percent() -> Option<u8> {
    let stat = fs::read_to_string("/proc/stat").ok()?;
    let line = stat.lines().next()?;
    let mut fields = line.split_whitespace();
    let _cpu = fields.next()?;
    let numbers: Vec<u64> = fields.filter_map(|field| field.parse().ok()).collect();
    if numbers.len() < 4 {
        return None;
    }

    let total: u64 = numbers.iter().sum();
    let idle = numbers[3] + numbers.get(4).copied().unwrap_or(0);
    percent_from_used_total(total.saturating_sub(idle), total)
}

fn linux_ram_percent() -> Option<u8> {
    let meminfo = fs::read_to_string("/proc/meminfo").ok()?;
    let total = meminfo_value_kib(&meminfo, "MemTotal:")?;
    let available = meminfo_value_kib(&meminfo, "MemAvailable:")?;
    percent_from_used_total(total.saturating_sub(available), total)
}

fn macos_cpu_percent() -> Option<u8> {
    let logical_cpu_output = command_output("sysctl", &["-n", "hw.logicalcpu"])?;
    let logical_cpus: f64 = logical_cpu_output.trim().parse().ok()?;
    if logical_cpus <= 0.0 {
        return None;
    }

    let cpu_output = command_output("ps", &["-A", "-o", "%cpu"])?;
    let summed_cpu: f64 = cpu_output
        .lines()
        .skip(1)
        .filter_map(|line| line.trim().parse::<f64>().ok())
        .sum();

    Some(clamp_percent((summed_cpu / logical_cpus).round() as i64))
}

fn macos_ram_percent() -> Option<u8> {
    let vm_stat_output = command_output("vm_stat", &[])?;
    let page_size = parse_vm_stat_page_size(&vm_stat_output)?;
    let free = parse_vm_stat_count(&vm_stat_output, "Pages free")?;
    let speculative = parse_vm_stat_count(&vm_stat_output, "Pages speculative")?;
    let active = parse_vm_stat_count(&vm_stat_output, "Pages active")?;
    let inactive = parse_vm_stat_count(&vm_stat_output, "Pages inactive")?;
    let wired = parse_vm_stat_count(&vm_stat_output, "Pages wired down")?;
    let compressed = parse_vm_stat_count(&vm_stat_output, "Pages occupied by compressor")?;

    let used_bytes = (active + inactive + wired + compressed).saturating_mul(page_size);
    let total_bytes = used_bytes + (free + speculative).saturating_mul(page_size);

    percent_from_used_total(used_bytes, total_bytes)
}

fn command_output(program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }

    String::from_utf8(output.stdout).ok()
}

fn command_output_in_dir(dir: impl AsRef<Path>, program: &str, args: &[&str]) -> Option<String> {
    let output = Command::new(program)
        .current_dir(dir)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    String::from_utf8(output.stdout).ok()
}

fn meminfo_value_kib(meminfo: &str, key: &str) -> Option<u64> {
    let line = meminfo.lines().find(|line| line.starts_with(key))?;
    line.split_whitespace().nth(1)?.parse().ok()
}

fn parse_vm_stat_page_size(vm_stat_output: &str) -> Option<u64> {
    let line = vm_stat_output.lines().next()?;
    let start = line.find("page size of ")? + "page size of ".len();
    let end = line[start..].find(" bytes")? + start;
    line[start..end].parse().ok()
}

fn parse_vm_stat_count(vm_stat_output: &str, label: &str) -> Option<u64> {
    let line = vm_stat_output
        .lines()
        .find(|line| line.trim_start().starts_with(label))?;
    let value = line.split(':').nth(1)?.trim().trim_end_matches('.');
    value.parse().ok()
}

fn parse_diff_numstat(diff_numstat: &str) -> (u32, u32, u32) {
    let mut changed = 0;
    let mut insertions = 0;
    let mut deletions = 0;

    for line in diff_numstat.lines() {
        let mut fields = line.split_whitespace();
        let added = fields.next();
        let removed = fields.next();
        let path = fields.next();

        if added.is_none() || removed.is_none() || path.is_none() {
            continue;
        }

        changed += 1;
        insertions += added.and_then(|value| value.parse::<u32>().ok()).unwrap_or(0);
        deletions += removed.and_then(|value| value.parse::<u32>().ok()).unwrap_or(0);
    }

    (changed, insertions, deletions)
}

fn truncate_branch(branch: &str) -> String {
    const MAX_BRANCH_LEN: usize = 25;

    let mut chars = branch.chars();
    let truncated: String = chars.by_ref().take(MAX_BRANCH_LEN).collect();

    if chars.next().is_some() {
        format!("{truncated}…")
    } else {
        truncated
    }
}

fn percent_from_used_total(used: u64, total: u64) -> Option<u8> {
    if total == 0 {
        return None;
    }

    let percent = ((used as f64 / total as f64) * 100.0).round() as i64;
    Some(clamp_percent(percent))
}

fn clamp_percent(value: i64) -> u8 {
    value.clamp(0, 100) as u8
}

#[cfg(test)]
mod tests {
    use super::{
        clamp_percent, forge_section, format_git_section, git_section_string, meminfo_value_kib,
        metrics_section, metrics_section_string, parse_diff_numstat, parse_vm_stat_count,
        parse_vm_stat_page_size, percent_from_used_total, truncate_branch, GitSnapshot,
        FORGE_SECTION_STUB, SHOW_FORGE_SECTION,
    };

    #[test]
    fn builds_current_widget_sections() {
        assert!(!git_section_string().is_empty());
        assert_eq!(metrics_section(), "");
        assert!(metrics_section_string().contains("🧠"));
        assert!(metrics_section_string().contains("💾"));

        if SHOW_FORGE_SECTION {
            assert_eq!(forge_section(), FORGE_SECTION_STUB);
        } else {
            assert_eq!(forge_section(), "");
        }
    }

    #[test]
    fn parses_proc_meminfo_values() {
        let meminfo = "MemTotal:       1000 kB\nMemAvailable:    250 kB\n";

        assert_eq!(meminfo_value_kib(meminfo, "MemTotal:"), Some(1000));
        assert_eq!(meminfo_value_kib(meminfo, "MemAvailable:"), Some(250));
    }

    #[test]
    fn parses_vm_stat_output() {
        let sample = "Mach Virtual Memory Statistics: (page size of 16384 bytes)\nPages free:                               100.\nPages active:                             200.\n";

        assert_eq!(parse_vm_stat_page_size(sample), Some(16384));
        assert_eq!(parse_vm_stat_count(sample, "Pages free"), Some(100));
        assert_eq!(parse_vm_stat_count(sample, "Pages active"), Some(200));
    }

    #[test]
    fn computes_percentages() {
        assert_eq!(percent_from_used_total(25, 100), Some(25));
        assert_eq!(percent_from_used_total(0, 0), None);
        assert_eq!(clamp_percent(101), 100);
        assert_eq!(clamp_percent(-5), 0);
    }

    #[test]
    fn parses_git_diff_numstat() {
        let sample = "10\t2\tsrc/lib.rs\n3\t0\tREADME.md\n";

        assert_eq!(parse_diff_numstat(sample), (2, 13, 2));
    }

    #[test]
    fn truncates_long_branch_names() {
        assert_eq!(truncate_branch("short-branch"), "short-branch");
        assert_eq!(
            truncate_branch("this-is-a-very-long-branch-name"),
            "this-is-a-very-long-branc…"
        );
    }

    #[test]
    fn formats_git_snapshot() {
        let snapshot = GitSnapshot {
            branch: "main".to_string(),
            changed_count: 2,
            insertions_count: 5,
            deletions_count: 1,
            untracked_count: 3,
        };

        assert_eq!(
            format_git_section(snapshot),
            "#[fg=colour142]▒  main #[fg=colour214] 2 #[fg=colour107] 5 #[fg=colour167] 1 #[fg=colour223] 3"
        );
    }
}
