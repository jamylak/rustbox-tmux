use std::collections::HashMap;
use std::env;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use crate::render::{RenderState, Renderer};
use crate::tmux::{
    current_pane_path, current_session_id, disable_theme, list_session_ids, publish_status,
    publish_status_for_session, refresh_status_line, set_option, set_session_option, show_option,
    show_session_option, theme_enabled, ACTIVE_PATH_OPTION, DAEMON_PID_OPTION,
    DEFAULT_GIT_REFRESH_SECS, GIT_REFRESH_OPTION, STATUS_OPTION,
};
use crate::widgets::{forge_section, git_section_string, metrics_section_string};

const METRICS_REFRESH_SECS: u64 = 5;
const MIN_GIT_REFRESH_SECS: u64 = 5;
const PATH_SUBSCRIPTION_NAME: &str = "path";
const PATH_SUBSCRIPTION_FORMAT: &str = "path:%*:#{pane_id} #{pane_current_path}";

// Replace the previous daemon on `init` so config reloads pick up a rebuilt
// binary instead of leaving an old process running forever.
pub fn ensure_daemon(binary_path: &Path) -> Result<(), String> {
    if let Some(pid) = show_option(DAEMON_PID_OPTION).and_then(|value| value.parse::<u32>().ok()) {
        if process_is_running(pid) && process_is_rustbox_daemon(pid) {
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
//   -> terminate that daemon if it is any rustbox daemon for this server,
//      even if the binary path changed between TPM/local/rebuilt copies
//   -> clear the stored pid
pub fn stop_current_server(_binary_path: &Path) -> Result<(), String> {
    disable_theme()?;

    if let Some(pid) = show_option(DAEMON_PID_OPTION).and_then(|value| value.parse::<u32>().ok()) {
        if process_is_running(pid) && process_is_rustbox_daemon(pid) {
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
        remember_active_path(path)?;
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
// - wait up to 5s for a tmux-native pane-path subscription event
// - publish immediately when tmux reports a pane cwd change
// - otherwise publish on the 5s cadence for metrics freshness
// - reuse the cached git section unless the repo changed or the git-specific
//   refresh interval has expired
fn run_idle_loop(mut state: DaemonState) -> ! {
    let mut path_subscription = PathSubscription::attach();
    let pid = std::process::id();

    loop {
        // Ownership check 👑
        //
        // This daemon only stays alive while tmux still points
        // `@rustbox_daemon_pid` at this exact pid.
        //
        // That makes stale daemons die off when:
        // - the tmux server is killed and the option disappears
        // - a newer daemon replaces this one during reload/startup
        // - the sandbox script boots a fresh tmux server on the same socket
        if !daemon_still_owned(pid) {
            drop(path_subscription);
            std::process::exit(0);
        }

        // Let `rustbox-tmux stop` shut the daemon down cleanly even if the
        // explicit SIGTERM race-misses and the process survives until the next
        // wake-up.
        if !theme_enabled() {
            drop(path_subscription);
            let _ = set_option(DAEMON_PID_OPTION, "");
            std::process::exit(0);
        }

        match path_subscription.as_ref().map(|subscription| {
            subscription.wait_for_change(Duration::from_secs(METRICS_REFRESH_SECS))
        }) {
            Some(Ok(PathEvent::Changed)) | Some(Err(RecvTimeoutError::Timeout)) => {
                let _ = publish_with_daemon_state(None, &mut state);
            }
            Some(Err(RecvTimeoutError::Disconnected)) => {
                path_subscription = PathSubscription::attach();
            }
            None => {
                thread::sleep(Duration::from_secs(METRICS_REFRESH_SECS));
                let _ = publish_with_daemon_state(None, &mut state);
            }
        }
    }
}

// Prefer an explicit CLI path, then the live tmux pane path, then the
// current session's remembered tmux path, and finally the process cwd.
fn resolve_render_path(path: Option<&Path>) -> Option<PathBuf> {
    path.map(Path::to_path_buf)
        .or_else(current_pane_path)
        .or_else(active_path)
        .or_else(|| env::current_dir().ok())
}

fn active_path() -> Option<PathBuf> {
    // Session-first path lookup 🧭
    //
    current_session_id()
        .and_then(|session_id| show_session_option(&session_id, ACTIVE_PATH_OPTION))
        .map(PathBuf::from)
}

fn publish_with_daemon_state(path: Option<&Path>, state: &mut DaemonState) -> Result<(), String> {
    // Publish router 🚦
    //
    // Two modes exist now:
    //
    // 1. direct publish (`rustbox-tmux publish`)
    //    -> we know which currently-focused session changed
    //    -> update just that session
    //
    // 2. background daemon tick
    //    -> no explicit path/session came in
    //    -> refresh every remembered session in the server
    //
    // This split is what fixes the "three tabs, three sessions, one server"
    // case without needing one daemon per session.
    if path.is_some() {
        publish_current_session(path, state)?;
    } else {
        publish_all_sessions(state)?;
    }

    refresh_status_line()?;
    Ok(())
}

fn publish_current_session(path: Option<&Path>, state: &mut DaemonState) -> Result<(), String> {
    // Focused-session fast path ⚡
    //
    // Hook fires / manual publish runs
    //   -> resolve the active repo path for the current session
    //   -> remember it on that session
    //   -> refresh only that session's git cache
    //   -> publish only that session's rendered status
    //
    // This keeps hook-driven updates cheap and avoids stomping unrelated
    // sessions that happen to live on the same tmux server.
    if !theme_enabled() {
        return Ok(());
    }

    let resolved_path = resolve_render_path(path);
    if let Some(path) = resolved_path.as_deref() {
        remember_active_path(path)?;
    }

    let session_id = current_session_id()
        .ok_or_else(|| "failed to determine current tmux session".to_string())?;

    let git_section = state.git_section_for_session(&session_id, resolved_path.as_deref());
    let render_state = current_render_state_with_git_section(git_section);
    let mut renderer = Renderer::new();
    publish_status_for_session(&session_id, renderer.render(&render_state))?;

    Ok(())
}

fn publish_all_sessions(state: &mut DaemonState) -> Result<(), String> {
    // Daemon fan-out refresh 🌐
    //
    // One daemon still owns one tmux server, but the server may have many
    // sessions, each pinned to a different repo. So every background wake-up:
    //
    // tmux server
    //   -> list sessions
    //   -> read each session's remembered path
    //   -> refresh that session's cached git section if needed
    //   -> publish that session's private status payload
    //
    // Metrics are still global/live-per-render, but git context is now kept
    // isolated per session so switching tabs no longer cross-contaminates the
    // branch/ahead/dirty widget.
    if !theme_enabled() {
        return Ok(());
    }

    let session_ids = list_session_ids();
    if session_ids.is_empty() {
        // Detached server / no sessions attached yet:
        // nothing session-scoped exists to refresh.
        return Ok(());
    }

    let mut renderer = Renderer::new();
    for session_id in session_ids {
        let path = show_session_option(&session_id, ACTIVE_PATH_OPTION).map(PathBuf::from);
        let git_section = state.git_section_for_session(&session_id, path.as_deref());
        let render_state = current_render_state_with_git_section(git_section);
        publish_status_for_session(&session_id, renderer.render(&render_state))?;
    }

    Ok(())
}
fn remember_active_path(path: &Path) -> Result<(), String> {
    // Session memory write 📝
    //
    // That remembered path is what lets the background daemon keep repo A tied
    // to session A and repo B tied to session B between focus changes.
    let session_id = current_session_id()
        .ok_or_else(|| "failed to determine current tmux session".to_string())?;
    set_session_option(&session_id, ACTIVE_PATH_OPTION, &path.to_string_lossy())
}

fn git_refresh_interval_secs() -> u64 {
    // Keep git on its own slower cadence than metrics so the daemon can stay
    // responsive without shelling out to `git` every 5 seconds forever.
    show_option(GIT_REFRESH_OPTION)
        .and_then(|value| value.parse::<u64>().ok())
        .map(|value| value.max(MIN_GIT_REFRESH_SECS))
        .unwrap_or(DEFAULT_GIT_REFRESH_SECS)
}

fn daemon_still_owned(pid: u32) -> bool {
    daemon_pid_matches(show_option(DAEMON_PID_OPTION).as_deref(), pid)
}

fn daemon_pid_matches(stored_pid: Option<&str>, pid: u32) -> bool {
    stored_pid
        .and_then(|value| value.parse::<u32>().ok())
        .map(|owner_pid| owner_pid == pid)
        .unwrap_or(false)
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

fn process_is_rustbox_daemon(pid: u32) -> bool {
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
        .map(|command| command_looks_like_rustbox_daemon(&command))
        .unwrap_or(false)
}

fn command_looks_like_rustbox_daemon(command: &str) -> bool {
    command.contains("rustbox-tmux") && command.contains(" daemon")
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
    // Per-session git memory 🧠
    //
    // Key: tmux session id like `$1`
    // Val: cached git widget state for that session's remembered repo
    git_caches: HashMap<String, GitSectionCache>,
    git_refresh_interval: Duration,
}

impl DaemonState {
    fn new(git_refresh_secs: u64) -> Self {
        Self {
            git_caches: HashMap::new(),
            git_refresh_interval: Duration::from_secs(git_refresh_secs),
        }
    }

    fn git_section_for_session(&mut self, session_id: &str, path: Option<&Path>) -> String {
        // Cache ownership rule 👑
        //
        // Each session gets its own git cache entry. That means:
        // session `$1` changing repos does not invalidate session `$2`
        // session `$2` staying idle does not lose its last git snapshot
        self.git_caches
            .entry(session_id.to_string())
            .or_insert_with(|| GitSectionCache::new(self.git_refresh_interval))
            .section_for(path)
    }
}

enum PathEvent {
    Changed,
}

struct PathSubscription {
    child: Child,
    _stdin: ChildStdin,
    events: Receiver<PathEvent>,
}

impl PathSubscription {
    // Native tmux path watcher 🎯
    //
    // daemon
    //   -> open one hidden control-mode client on the current session
    //   -> subscribe to `%*` pane path changes
    //   -> tmux emits `%subscription-changed` when a pane cwd changes
    //   -> wake the daemon immediately instead of polling `tmux` every second
    fn attach() -> Option<Self> {
        let session_id = current_session_id()?;
        let mut child = Command::new("tmux")
            .args([
                "-C",
                "attach-session",
                "-t",
                &session_id,
                "-f",
                "no-output,ignore-size,read-only",
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;

        let mut stdin = child.stdin.take()?;
        writeln!(stdin, "refresh-client -B \"{PATH_SUBSCRIPTION_FORMAT}\"").ok()?;
        stdin.flush().ok()?;

        let stdout = child.stdout.take()?;
        let (sender, receiver) = mpsc::channel();

        thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                let Ok(line) = line else {
                    break;
                };
                if line.starts_with(&format!("%subscription-changed {PATH_SUBSCRIPTION_NAME} ")) {
                    let _ = sender.send(PathEvent::Changed);
                }
            }
        });

        Some(Self {
            child,
            _stdin: stdin,
            events: receiver,
        })
    }

    fn wait_for_change(&self, timeout: Duration) -> Result<PathEvent, RecvTimeoutError> {
        self.events.recv_timeout(timeout)
    }
}

impl Drop for PathSubscription {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
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
        // Git refresh policy ⏱️
        //
        // path changed
        //   -> refresh immediately because repo context is different
        //
        // same path + cache still fresh
        //   -> reuse the old git section
        //
        // same path + cache expired
        //   -> refresh now
        //
        // This is the piece that keeps the daemon responsive without running
        // `git` constantly on every 5s metrics wake-up.
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
    use super::{
        command_looks_like_rustbox_daemon, current_render_state, daemon_pid_matches, DaemonState,
        GitSectionCache, PATH_SUBSCRIPTION_FORMAT,
    };
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
        let first = state.git_section_for_session("$1", Some(Path::new("/tmp/one")));
        let second = state.git_section_for_session("$1", Some(Path::new("/tmp/two")));

        assert_eq!(first, "");
        assert_eq!(second, "");
        assert_eq!(
            state.git_caches["$1"].repo_path.as_deref(),
            Some(Path::new("/tmp/two"))
        );
    }

    #[test]
    fn keeps_git_cache_until_interval_expires() {
        let mut state = DaemonState::new(30);
        let cache = state
            .git_caches
            .entry("$1".to_string())
            .or_insert_with(|| GitSectionCache::new(state.git_refresh_interval));
        cache.repo_path = Some(Path::new("/tmp/demo").to_path_buf());
        cache.section = "cached".to_string();
        cache.refreshed_at = Some(Instant::now());
        cache.refresh_interval = Duration::from_secs(30);

        assert_eq!(
            state.git_section_for_session("$1", Some(Path::new("/tmp/demo"))),
            "cached"
        );
    }

    #[test]
    fn keeps_independent_git_caches_per_session() {
        let mut state = DaemonState::new(30);

        state.git_section_for_session("$1", Some(Path::new("/tmp/one")));
        state.git_section_for_session("$2", Some(Path::new("/tmp/two")));

        assert_eq!(
            state.git_caches["$1"].repo_path.as_deref(),
            Some(Path::new("/tmp/one"))
        );
        assert_eq!(
            state.git_caches["$2"].repo_path.as_deref(),
            Some(Path::new("/tmp/two"))
        );
    }

    #[test]
    fn path_subscription_targets_all_panes() {
        assert_eq!(
            PATH_SUBSCRIPTION_FORMAT,
            "path:%*:#{pane_id} #{pane_current_path}"
        );
    }

    #[test]
    fn detects_rustbox_daemon_commands_across_binary_paths() {
        assert!(command_looks_like_rustbox_daemon(
            "/Users/james/.tmux/plugins/rustbox-tmux/target/release/rustbox-tmux daemon"
        ));
        assert!(command_looks_like_rustbox_daemon(
            "/Users/james/proj/rustbox-tmux/target/release/rustbox-tmux daemon"
        ));
        assert!(!command_looks_like_rustbox_daemon(
            "/usr/bin/tmux attach-session"
        ));
    }

    #[test]
    fn ownership_check_only_keeps_the_matching_pid_alive() {
        assert!(daemon_pid_matches(Some("12345"), 12345));
        assert!(!daemon_pid_matches(Some("99999"), 12345));
        assert!(!daemon_pid_matches(None, 12345));
        assert!(!daemon_pid_matches(Some("not-a-pid"), 12345));
    }
}
