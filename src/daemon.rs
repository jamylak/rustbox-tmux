use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::render::{RenderState, Renderer};
use crate::tmux::{
    current_pane_path, disable_theme, publish_status, refresh_status_line, set_option, show_option,
    theme_enabled, ACTIVE_PATH_OPTION, DAEMON_PID_OPTION, DEFAULT_GIT_REFRESH_SECS,
    GIT_REFRESH_OPTION, STATUS_OPTION,
};
use crate::widgets::{forge_section, git_section_string, metrics_section_string};

const METRICS_REFRESH_SECS: u64 = 5;
const MIN_GIT_REFRESH_SECS: u64 = 5;

// Replace the previous daemon on `init` so config reloads pick up a rebuilt
// binary instead of leaving an old process running forever.
pub fn ensure_daemon(binary_path: &Path) -> Result<(), String> {
    if let Some(pid) = show_option(DAEMON_PID_OPTION).and_then(|value| value.parse::<u32>().ok()) {
        if process_is_running(pid) && process_is_our_daemon(pid, binary_path) {
            stop_process(pid)?;
        }
    }

    set_option(DAEMON_PID_OPTION, "")?;
    Command::new(binary_path)
        .arg("daemon")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("failed to spawn rustbox daemon: {error}"))?;

    Ok(())
}

pub fn run_daemon() -> Result<(), String> {
    if !theme_enabled() {
        set_option(DAEMON_PID_OPTION, "")?;
        return Ok(());
    }

    let mut state = DaemonState::new(git_refresh_interval_secs());
    set_option(DAEMON_PID_OPTION, &std::process::id().to_string())?;
    publish_with_daemon_state(None, &mut state)?;

    log_startup();

    run_idle_loop(state);
}

// Stop flow 🛑
//
// current tmux server
//   -> disable rustbox inside tmux first
//   -> read the stored daemon pid
//   -> terminate that daemon if it matches this rustbox binary
//   -> clear the stored pid
pub fn stop_current_server(binary_path: &Path) -> Result<(), String> {
    disable_theme()?;

    if let Some(pid) = show_option(DAEMON_PID_OPTION).and_then(|value| value.parse::<u32>().ok()) {
        if process_is_running(pid) && process_is_our_daemon(pid, binary_path) {
            stop_process(pid)?;
        }
    }

    set_option(DAEMON_PID_OPTION, "")?;
    Ok(())
}

// Publish one snapshot now and remember the resolved path so the background
// loop can keep refreshing the same repo context.
pub fn publish_once(path: Option<&Path>) -> Result<(), String> {
    if !theme_enabled() {
        return Ok(());
    }

    let resolved_path = resolve_render_path(path);
    if let Some(path) = resolved_path.as_deref() {
        set_option(ACTIVE_PATH_OPTION, &path.to_string_lossy())?;
    }

    let state = current_render_state(resolved_path.as_deref());
    let mut renderer = Renderer::new();
    publish_status(renderer.render(&state))?;
    refresh_status_line()?;

    Ok(())
}

pub fn current_render_state(path: Option<&Path>) -> RenderState {
    current_render_state_with_git_section(git_section_string(path))
}

fn current_render_state_with_git_section(git_section: String) -> RenderState {
    RenderState {
        git_section,
        forge_section: forge_section().to_string(),
        metrics_section: metrics_section_string(),
    }
}

fn log_startup() {
    eprintln!("rustbox-tmuxd started");
    eprintln!("published initial status to {STATUS_OPTION}");
}

// Background loop:
// - wake every 5s for metrics freshness
// - reuse the cached git section unless the repo changed or the git-specific
//   refresh interval has expired
fn run_idle_loop(mut state: DaemonState) -> ! {
    loop {
        // Let `rustbox-tmux stop` shut the daemon down cleanly even if the
        // explicit SIGTERM race-misses and the process survives until the next
        // wake-up.
        if !theme_enabled() {
            let _ = set_option(DAEMON_PID_OPTION, "");
            std::process::exit(0);
        }

        thread::sleep(Duration::from_secs(METRICS_REFRESH_SECS));
        let _ = publish_with_daemon_state(None, &mut state);
    }
}

// Prefer an explicit CLI path, then the live tmux pane path, then the last
// remembered tmux path, and finally the process cwd as a fallback.
fn resolve_render_path(path: Option<&Path>) -> Option<PathBuf> {
    path.map(Path::to_path_buf)
        .or_else(current_pane_path)
        .or_else(active_path)
        .or_else(|| env::current_dir().ok())
}

fn active_path() -> Option<PathBuf> {
    show_option(ACTIVE_PATH_OPTION).map(PathBuf::from)
}

fn publish_with_daemon_state(path: Option<&Path>, state: &mut DaemonState) -> Result<(), String> {
    if !theme_enabled() {
        return Ok(());
    }

    let resolved_path = resolve_render_path(path);
    if let Some(path) = resolved_path.as_deref() {
        set_option(ACTIVE_PATH_OPTION, &path.to_string_lossy())?;
    }

    let git_section = state.git_cache.section_for(resolved_path.as_deref());
    let render_state = current_render_state_with_git_section(git_section);
    let mut renderer = Renderer::new();
    publish_status(renderer.render(&render_state))?;
    refresh_status_line()?;

    Ok(())
}

fn git_refresh_interval_secs() -> u64 {
    // Keep git on its own slower cadence than metrics so the daemon can stay
    // responsive without shelling out to `git` every 5 seconds forever.
    show_option(GIT_REFRESH_OPTION)
        .and_then(|value| value.parse::<u64>().ok())
        .map(|value| value.max(MIN_GIT_REFRESH_SECS))
        .unwrap_or(DEFAULT_GIT_REFRESH_SECS)
}

// `kill -0` is a liveness probe, not a termination signal.
//
// It asks the kernel:
// - does this pid exist?
// - if so, am I allowed to signal it?
//
// Exit status `0` means "yes". A non-zero exit usually means "no such pid" or
// "it exists but I do not have permission". For this tmux daemon we only care
// about the same-user happy path, so non-zero is treated as "not reusable".
fn process_is_running(pid: u32) -> bool {
    Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn process_is_our_daemon(pid: u32, binary_path: &Path) -> bool {
    let Some(binary_path) = binary_path.to_str() else {
        return false;
    };
    let output = Command::new("ps")
        .args(["-o", "command=", "-p", &pid.to_string()])
        .output()
        .ok();
    let Some(output) = output else {
        return false;
    };
    if !output.status.success() {
        return false;
    }

    String::from_utf8(output.stdout)
        .ok()
        .map(|command| command.contains(binary_path) && command.contains(" daemon"))
        .unwrap_or(false)
}

fn stop_process(pid: u32) -> Result<(), String> {
    let status = Command::new("kill")
        .arg(pid.to_string())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("failed to stop old rustbox daemon {pid}: {error}"))?;
    if !status.success() {
        if !process_is_running(pid) {
            return Ok(());
        }
        return Err(format!("failed to stop old rustbox daemon {pid}: {status}"));
    }

    for _ in 0..20 {
        if !process_is_running(pid) {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }

    Err(format!(
        "old rustbox daemon {pid} did not exit after SIGTERM"
    ))
}

struct DaemonState {
    git_cache: GitSectionCache,
}

impl DaemonState {
    fn new(git_refresh_secs: u64) -> Self {
        Self {
            git_cache: GitSectionCache::new(Duration::from_secs(git_refresh_secs)),
        }
    }
}

struct GitSectionCache {
    repo_path: Option<PathBuf>,
    section: String,
    refreshed_at: Option<Instant>,
    refresh_interval: Duration,
}

impl GitSectionCache {
    fn new(refresh_interval: Duration) -> Self {
        Self {
            repo_path: None,
            section: String::new(),
            refreshed_at: None,
            refresh_interval,
        }
    }

    // Git cache flow:
    // path changed            -> refresh now
    // same path + interval ok -> reuse cached section
    // same path + stale       -> refresh now
    fn section_for(&mut self, path: Option<&Path>) -> String {
        let path_changed = self.repo_path.as_deref() != path;
        let refresh_due = self
            .refreshed_at
            .map(|instant| instant.elapsed() >= self.refresh_interval)
            .unwrap_or(true);

        if path_changed || refresh_due {
            self.repo_path = path.map(Path::to_path_buf);
            self.section = git_section_string(path);
            self.refreshed_at = Some(Instant::now());
        }

        self.section.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::{current_render_state, DaemonState};
    use crate::widgets::{forge_section, git_section_string};
    use std::path::Path;
    use std::time::{Duration, Instant};

    #[test]
    fn builds_render_state_from_current_sections() {
        let state = current_render_state(None);

        assert_eq!(state.git_section, git_section_string(None));
        assert_eq!(state.forge_section, forge_section());
        assert!(state.metrics_section.contains("🧠"));
        assert!(state.metrics_section.contains("💾"));
    }

    #[test]
    fn refreshes_git_cache_when_repo_changes() {
        let mut state = DaemonState::new(30);
        let first = state.git_cache.section_for(Some(Path::new("/tmp/one")));
        let second = state.git_cache.section_for(Some(Path::new("/tmp/two")));

        assert_eq!(first, "");
        assert_eq!(second, "");
        assert_eq!(
            state.git_cache.repo_path.as_deref(),
            Some(Path::new("/tmp/two"))
        );
    }

    #[test]
    fn keeps_git_cache_until_interval_expires() {
        let mut state = DaemonState::new(30);
        state.git_cache.repo_path = Some(Path::new("/tmp/demo").to_path_buf());
        state.git_cache.section = "cached".to_string();
        state.git_cache.refreshed_at = Some(Instant::now());
        state.git_cache.refresh_interval = Duration::from_secs(30);

        assert_eq!(
            state.git_cache.section_for(Some(Path::new("/tmp/demo"))),
            "cached"
        );
    }
}
