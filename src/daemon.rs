use std::process::Command;
use std::thread;
use std::time::Duration;

const STATIC_STATUS: &str = "#[fg=green]rustbox-tmux bootstrap";
const TMUX_STATUS_OPTION: &str = "@rustbox_status_right";

pub fn run_daemon() -> Result<(), String> {
    publish_to_tmux(STATIC_STATUS)?;

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

fn tmux_set_option_args(status: &str) -> [&str; 4] {
    ["set-option", "-gq", TMUX_STATUS_OPTION, status]
}

#[cfg(test)]
mod tests {
    use super::{tmux_set_option_args, TMUX_STATUS_OPTION};

    #[test]
    fn builds_tmux_set_option_args() {
        let args = tmux_set_option_args("hello");
        assert_eq!(args, ["set-option", "-gq", TMUX_STATUS_OPTION, "hello"]);
    }
}
