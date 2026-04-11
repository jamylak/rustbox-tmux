use std::process::Command;
use std::thread;
use std::time::Duration;

const STATIC_STATUS: &str = "#[fg=green]rustbox-tmux bootstrap";
const TMUX_STATUS_OPTION: &str = "@rustbox_status_right";

pub fn run_daemon() -> Result<(), String> {
    publish_to_tmux(STATIC_STATUS)?;

    // Force one initial status redraw so attached clients pick up the first
    // published value immediately. Steady-state updates should rely on tmux
    // redrawing `#{@rustbox_status_right}` on its normal cadence.
    refresh_tmux()?;

    eprintln!("rustbox-tmuxd started");
    eprintln!("published initial status to {TMUX_STATUS_OPTION}");

    loop {
        thread::sleep(Duration::from_secs(60));
    }
}

fn publish_to_tmux(status: &str) -> Result<(), String> {
    let status = Command::new("tmux")
        .args(tmux_set_option_args(status))
        .status()
        .map_err(|error| format!("failed to run tmux set-option: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("tmux set-option exited with status {status}"))
    }
}

// This is intentionally a startup-only nudge, not the normal path for every
// daemon update.
fn refresh_tmux() -> Result<(), String> {
    let status = Command::new("tmux")
        .args(tmux_refresh_args())
        .status()
        .map_err(|error| format!("failed to run tmux refresh-client: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("tmux refresh-client exited with status {status}"))
    }
}

fn tmux_set_option_args(status: &str) -> [&str; 4] {
    ["set-option", "-gq", TMUX_STATUS_OPTION, status]
}

fn tmux_refresh_args() -> [&'static str; 2] {
    ["refresh-client", "-S"]
}

#[cfg(test)]
mod tests {
    use super::{tmux_refresh_args, tmux_set_option_args, TMUX_STATUS_OPTION};

    #[test]
    fn builds_tmux_set_option_args() {
        let args = tmux_set_option_args("hello");
        assert_eq!(args, ["set-option", "-gq", TMUX_STATUS_OPTION, "hello"]);
    }

    #[test]
    fn builds_tmux_refresh_args() {
        let args = tmux_refresh_args();
        assert_eq!(args, ["refresh-client", "-S"]);
    }
}
