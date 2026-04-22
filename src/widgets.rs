use std::fs;
use std::path::Path;
use std::process::Command;

const RESET: &str = "#[fg=#fbf1c7,bg=#282828,nobold,noitalics,nounderscore,nodim]";
const THEME_BACKGROUND: &str = "#282828";
const THEME_FOREGROUND: &str = "#fbf1c7";
const THEME_GREEN: &str = "#98971a";
const THEME_RED: &str = "#cc241d";
const THEME_YELLOW: &str = "#d79921";
const THEME_BLACK: &str = "#282828";
const THEME_BPURPLE: &str = "#d3869b";
const THEME_BRED: &str = "#fb4934";
const FORGE_SECTION_STUB: &str = "#[fg=#282828,bg=#d3869b]  #[fg=#fbf1c7,bg=#282828]--";
const SHOW_FORGE_SECTION: bool = false;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitSnapshot {
    pub branch: String,
    pub sync_mode: GitSyncMode,
    pub changed_count: u32,
    pub insertions_count: u32,
    pub deletions_count: u32,
    pub untracked_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitSyncMode {
    Clean,
    Dirty,
    NeedPush,
    RemoteDiff,
}

pub fn git_section_string(path: Option<&Path>) -> String {
    current_git_snapshot(path)
        .map(format_git_section)
        .unwrap_or_default()
}

pub fn forge_section() -> &'static str {
    if SHOW_FORGE_SECTION {
        FORGE_SECTION_STUB
    } else {
        ""
    }
}

pub fn metrics_section_string() -> String {
    let cpu = cpu_percent().unwrap_or(0);
    let ram = ram_percent().unwrap_or(0);

    format!(
        "{RESET}#[fg=#fabd2f,bg=#282828,bold]▒ #[fg=#d79921]🧠 {} #[fg=#d79921]{cpu}% \
#[fg=#fbf1c7]💾 {} #[fg=#fe8019]{ram}% ",
        usage_blocks(cpu),
        usage_blocks(ram),
    )
}

pub fn current_git_snapshot(path: Option<&Path>) -> Option<GitSnapshot> {
    let repo_root = command_output_in_dir(path?, "git", &["rev-parse", "--show-toplevel"])?;
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
    let untracked_count = untracked_output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count() as u32;
    let sync_mode = git_sync_mode(
        repo_root,
        changed_count,
        insertions_count,
        deletions_count,
        untracked_count,
    );

    Some(GitSnapshot {
        branch: truncate_branch(branch),
        sync_mode,
        changed_count,
        insertions_count,
        deletions_count,
        untracked_count,
    })
}

pub fn format_git_section(snapshot: GitSnapshot) -> String {
    let (status_color, status_icon) = git_status_style(snapshot.sync_mode);
    let mut section = format!(
        "{RESET}#[bg={THEME_BACKGROUND},fg={status_color},bold]▒ {status_icon} \
{RESET}#[fg={THEME_FOREGROUND},bg={THEME_BACKGROUND}]{}",
        snapshot.branch
    );

    if snapshot.changed_count > 0 {
        section.push_str(&format!(
            " {RESET}#[fg={THEME_YELLOW},bg={THEME_BACKGROUND},bold] {}",
            snapshot.changed_count
        ));
    }

    if snapshot.insertions_count > 0 {
        section.push_str(&format!(
            " {RESET}#[fg={THEME_GREEN},bg={THEME_BACKGROUND},bold] {}",
            snapshot.insertions_count
        ));
    }

    if snapshot.deletions_count > 0 {
        section.push_str(&format!(
            " {RESET}#[fg={THEME_RED},bg={THEME_BACKGROUND},bold] {}",
            snapshot.deletions_count
        ));
    }

    if snapshot.untracked_count > 0 {
        section.push_str(&format!(
            " {RESET}#[fg={THEME_BLACK},bg={THEME_BACKGROUND},bold] {}",
            snapshot.untracked_count
        ));
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
    let top_output = command_output("top", &["-l", "1", "-n", "0"])?;
    parse_macos_top_cpu_percent(&top_output)
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

    macos_ram_percent_from_pages(
        page_size,
        active,
        inactive,
        wired,
        compressed,
        speculative,
        free,
    )
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

fn parse_macos_top_cpu_percent(top_output: &str) -> Option<u8> {
    let cpu_line = top_output
        .lines()
        .find(|line| line.trim_start().starts_with("CPU usage:"))?;
    let user = parse_macos_cpu_field(cpu_line, "% user")?;
    let sys = parse_macos_cpu_field(cpu_line, "% sys")?;

    Some(clamp_percent((user + sys).round() as i64))
}

fn parse_macos_cpu_field(line: &str, suffix: &str) -> Option<f64> {
    let end = line.find(suffix)?;
    let value = line[..end]
        .rsplit_once(char::is_whitespace)
        .map(|(_, value)| value)
        .unwrap_or(&line[..end]);
    value.trim().parse().ok()
}

fn git_sync_mode(
    repo_root: &str,
    changed_count: u32,
    insertions_count: u32,
    deletions_count: u32,
    untracked_count: u32,
) -> GitSyncMode {
    if changed_count > 0 || insertions_count > 0 || deletions_count > 0 {
        return GitSyncMode::Dirty;
    }

    if git_upstream_ahead_count(repo_root).unwrap_or(0) > 0 {
        return GitSyncMode::NeedPush;
    }

    if git_has_remote_diff(repo_root) {
        return GitSyncMode::RemoteDiff;
    }

    if untracked_count > 0 {
        return GitSyncMode::NeedPush;
    }

    GitSyncMode::Clean
}

fn git_upstream_ahead_count(repo_root: &str) -> Option<u32> {
    let output =
        command_output_in_dir(repo_root, "git", &["rev-list", "--count", "@{push}..HEAD"])?;
    output.trim().parse().ok()
}

fn git_has_remote_diff(repo_root: &str) -> bool {
    let Some(upstream) = command_output_in_dir(
        repo_root,
        "git",
        &["rev-parse", "--abbrev-ref", "@{upstream}"],
    ) else {
        return false;
    };
    let upstream = upstream.trim();
    if upstream.is_empty() {
        return false;
    }

    command_output_in_dir(repo_root, "git", &["diff", "--numstat", "HEAD", upstream])
        .map(|output| !output.trim().is_empty())
        .unwrap_or(false)
}

fn git_status_style(mode: GitSyncMode) -> (&'static str, &'static str) {
    match mode {
        GitSyncMode::Dirty => (THEME_BRED, "󱓎"),
        GitSyncMode::NeedPush => (THEME_RED, "󰛃"),
        GitSyncMode::RemoteDiff => (THEME_BPURPLE, "󰛀"),
        GitSyncMode::Clean => (THEME_GREEN, ""),
    }
}

fn macos_ram_percent_from_pages(
    page_size: u64,
    active: u64,
    inactive: u64,
    wired: u64,
    compressed: u64,
    speculative: u64,
    free: u64,
) -> Option<u8> {
    // Match the old shell theme:
    //
    // used  = active + wired + compressed
    // total = active + wired + compressed + inactive + speculative + free
    //
    // `inactive` memory is available for reuse, so counting it as already-used
    // badly overstates pressure on macOS.
    let used_bytes = (active + wired + compressed).saturating_mul(page_size);
    let total_bytes =
        (active + wired + compressed + inactive + speculative + free).saturating_mul(page_size);

    percent_from_used_total(used_bytes, total_bytes)
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
        insertions += added
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(0);
        deletions += removed
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(0);
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

fn usage_blocks(percent: u8) -> &'static str {
    match percent {
        0..=12 => "□□□□",
        13..=37 => "■□□□",
        38..=62 => "■■□□",
        63..=87 => "■■■□",
        _ => "■■■■",
    }
}

#[cfg(test)]
mod tests {
    use super::{
        clamp_percent, forge_section, format_git_section, git_section_string, git_status_style,
        macos_ram_percent_from_pages, meminfo_value_kib, metrics_section_string,
        parse_diff_numstat, parse_macos_top_cpu_percent, parse_vm_stat_count,
        parse_vm_stat_page_size, percent_from_used_total, truncate_branch, GitSnapshot,
        GitSyncMode, FORGE_SECTION_STUB, SHOW_FORGE_SECTION,
    };

    #[test]
    fn builds_current_widget_sections() {
        let current_dir = std::env::current_dir().unwrap();
        assert!(!git_section_string(Some(current_dir.as_path())).is_empty());
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
    fn parses_macos_top_cpu_output() {
        let sample = "Processes: 520 total, 2 running, 518 sleeping, 2484 threads\nCPU usage: 4.34% user, 11.11% sys, 84.54% idle\n";

        assert_eq!(parse_macos_top_cpu_percent(sample), Some(15));
    }

    #[test]
    fn computes_macos_ram_like_the_old_theme() {
        assert_eq!(
            macos_ram_percent_from_pages(1, 200, 300, 100, 50, 25, 25),
            Some(50)
        );
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
            sync_mode: GitSyncMode::Dirty,
            changed_count: 2,
            insertions_count: 5,
            deletions_count: 1,
            untracked_count: 3,
        };

        assert_eq!(
            format_git_section(snapshot),
            "#[fg=#fbf1c7,bg=#282828,nobold,noitalics,nounderscore,nodim]#[bg=#282828,fg=#fb4934,bold]▒ 󱓎 #[fg=#fbf1c7,bg=#282828,nobold,noitalics,nounderscore,nodim]#[fg=#fbf1c7,bg=#282828]main #[fg=#fbf1c7,bg=#282828,nobold,noitalics,nounderscore,nodim]#[fg=#d79921,bg=#282828,bold] 2 #[fg=#fbf1c7,bg=#282828,nobold,noitalics,nounderscore,nodim]#[fg=#98971a,bg=#282828,bold] 5 #[fg=#fbf1c7,bg=#282828,nobold,noitalics,nounderscore,nodim]#[fg=#cc241d,bg=#282828,bold] 1 #[fg=#fbf1c7,bg=#282828,nobold,noitalics,nounderscore,nodim]#[fg=#282828,bg=#282828,bold] 3"
        );
    }

    #[test]
    fn exposes_git_sync_styles() {
        assert_eq!(git_status_style(GitSyncMode::Clean), ("#98971a", ""));
        assert_eq!(git_status_style(GitSyncMode::Dirty), ("#fb4934", "󱓎"));
        assert_eq!(git_status_style(GitSyncMode::NeedPush), ("#cc241d", "󰛃"));
        assert_eq!(git_status_style(GitSyncMode::RemoteDiff), ("#d3869b", "󰛀"));
    }
}
