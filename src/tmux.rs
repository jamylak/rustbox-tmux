use std::process::Command;

pub const STATUS_OPTION: &str = "@rustbox_status_right";

pub fn publish_status(status: &str) -> Result<(), String> {
    let status = Command::new("tmux")
        .args(set_option_args(status))
        .status()
        .map_err(|error| format!("failed to run tmux set-option: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("tmux set-option exited with status {status}"))
    }
}

pub fn refresh_status_line() -> Result<(), String> {
    let status = Command::new("tmux")
        .args(refresh_args())
        .status()
        .map_err(|error| format!("failed to run tmux refresh-client: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("tmux refresh-client exited with status {status}"))
    }
}

fn set_option_args(status: &str) -> [&str; 4] {
    ["set-option", "-gq", STATUS_OPTION, status]
}

fn refresh_args() -> [&'static str; 2] {
    ["refresh-client", "-S"]
}

#[cfg(test)]
mod tests {
    use super::{refresh_args, set_option_args, STATUS_OPTION};

    #[test]
    fn builds_tmux_set_option_args() {
        let args = set_option_args("hello");
        assert_eq!(args, ["set-option", "-gq", STATUS_OPTION, "hello"]);
    }

    #[test]
    fn builds_tmux_refresh_args() {
        let args = refresh_args();
        assert_eq!(args, ["refresh-client", "-S"]);
    }
}
