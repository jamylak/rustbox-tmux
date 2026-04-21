use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use crate::render::{RenderState, Renderer};
use crate::tmux::{
    current_pane_path, publish_status, refresh_status_line, set_option, show_option,
    ACTIVE_PATH_OPTION, DAEMON_PID_OPTION, STATUS_OPTION,
};
use crate::widgets::{forge_section, git_section_string, metrics_section_string};

const IDLE_LOOP_SLEEP_SECS: u64 = 5;

// Reuse an existing updater when tmux reloads config instead of spawning one
// daemon per `run-shell`.
pub fn ensure_daemon(binary_path: &Path) -> Result<(), String> {
    if let Some(pid) = show_option(DAEMON_PID_OPTION).and_then(|value| value.parse::<u32>().ok()) {
        if process_is_running(pid) {
            return Ok(());
        }
    }

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
    set_option(DAEMON_PID_OPTION, &std::process::id().to_string())?;
    publish_once(None)?;

    log_startup();

    run_idle_loop();
}

// Publish one snapshot now and remember the resolved path so the background
// loop can keep refreshing the same repo context.
pub fn publish_once(path: Option<&Path>) -> Result<(), String> {
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
    RenderState {
        git_section: git_section_string(path),
        forge_section: forge_section().to_string(),
        metrics_section: metrics_section_string(),
    }
}

fn log_startup() {
    eprintln!("rustbox-tmuxd started");
    eprintln!("published initial status to {STATUS_OPTION}");
}

fn run_idle_loop() -> ! {
    loop {
        thread::sleep(Duration::from_secs(IDLE_LOOP_SLEEP_SECS));
        let _ = publish_once(None);
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
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::current_render_state;
    use crate::widgets::{forge_section, git_section_string};

    #[test]
    fn builds_render_state_from_current_sections() {
        let state = current_render_state(None);

        assert_eq!(state.git_section, git_section_string(None));
        assert_eq!(state.forge_section, forge_section());
        assert!(state.metrics_section.contains("🧠"));
        assert!(state.metrics_section.contains("💾"));
    }
}
