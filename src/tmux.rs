use std::path::PathBuf;
use std::process::Command;

pub const STATUS_OPTION: &str = "@rustbox_status_right";
pub const ACTIVE_PATH_OPTION: &str = "@rustbox_active_path";
pub const DAEMON_PID_OPTION: &str = "@rustbox_daemon_pid";
pub const GIT_REFRESH_OPTION: &str = "@rustbox_git_refresh_seconds";
pub const ENABLED_OPTION: &str = "@rustbox_enabled";
pub const DEFAULT_GIT_REFRESH_SECS: u64 = 30;
const HOOKS_INSTALLED_OPTION: &str = "@rustbox_hooks_installed";

pub fn publish_status(status: &str) -> Result<(), String> {
    set_option(STATUS_OPTION, status)
}

// `refresh-client -S` fails when there is no attached client; treat that as a
// harmless no-op so detached tmux servers can still preload status.
pub fn refresh_status_line() -> Result<(), String> {
    let output = Command::new("tmux")
        .args(refresh_args())
        .output()
        .map_err(|error| format!("failed to run tmux refresh-client: {error}"))?;

    if output.status.success() {
        Ok(())
    } else if String::from_utf8_lossy(&output.stderr).contains("no current client") {
        Ok(())
    } else {
        Err(format!(
            "tmux refresh-client exited with status {}",
            output.status
        ))
    }
}

#[cfg(test)]
fn set_option_args(status: &str) -> [&str; 4] {
    ["set-option", "-gq", STATUS_OPTION, status]
}

fn refresh_args() -> [&'static str; 2] {
    ["refresh-client", "-S"]
}

pub fn set_option(name: &str, value: &str) -> Result<(), String> {
    // Write path:
    // rustbox -> `tmux set-option -gq <name> <value>` -> tmux global option store
    //
    // We use tmux user options as the handoff point between the Rust updater
    // and the status line format string that tmux renders later.
    let status = Command::new("tmux")
        .args(["set-option", "-gq", name, value])
        .status()
        .map_err(|error| format!("failed to run tmux set-option for {name}: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "tmux set-option for {name} exited with status {status}"
        ))
    }
}

pub fn show_option(name: &str) -> Option<String> {
    // Read path:
    // rustbox -> `tmux show-option -gv <name>` -> current tmux value
    //
    // Return `None` when:
    // - tmux rejects the lookup
    // - the value is not valid UTF-8
    // - the value is empty after trimming
    let output = Command::new("tmux")
        .args(["show-option", "-gv", name])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let value = String::from_utf8(output.stdout).ok()?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub fn current_pane_path() -> Option<PathBuf> {
    // Active-context lookup:
    // tmux active pane -> `#{pane_current_path}` -> Rust `PathBuf`
    //
    // This is how `publish` learns which repo the user is actually looking at
    // without guessing from the daemon's own cwd.
    let output = Command::new("tmux")
        .args(["display-message", "-p", "#{pane_current_path}"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let value = String::from_utf8(output.stdout).ok()?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(PathBuf::from(trimmed))
    }
}

pub fn theme_enabled() -> bool {
    // Compatibility rule:
    // missing option => enabled
    //
    // That keeps old installs working, while `stop` can flip the option to `0`
    // and turn future hook-driven `publish` calls into no-ops.
    !matches!(
        show_option(ENABLED_OPTION).as_deref(),
        Some("0") | Some("false") | Some("off")
    )
}

// Minimal tmux setup for the current Rust status-right feature set only.
//
// Result:
// 1. `status-right` reads `#{@rustbox_status_right}`
// 2. a few context-change hooks run `rustbox-tmux publish`
// 3. reloads stay idempotent instead of stacking duplicate hooks
pub fn configure_theme(binary_path: &str) -> Result<(), String> {
    set_option("status-right", "#{@rustbox_status_right}")?;
    set_option("status-right-length", "160")?;
    set_option(ENABLED_OPTION, "1")?;
    if show_option(GIT_REFRESH_OPTION).is_none() {
        set_option(GIT_REFRESH_OPTION, &DEFAULT_GIT_REFRESH_SECS.to_string())?;
    }

    // Guard hook installation so re-sourcing tmux config does not duplicate
    // the same `run-shell ... publish` hook entries.
    if show_option(HOOKS_INSTALLED_OPTION).as_deref() == Some("1") {
        return Ok(());
    }

    let publish_command = format!("run-shell -b \"{binary_path} publish\"");
    // Hook map:
    // - `after-select-pane`   : user focused a different pane
    // - `after-select-window` : user focused a different window
    // - `after-new-window`    : a new window appeared with a new cwd/context
    // - `after-split-window`  : a new pane appeared with a new cwd/context
    // - `client-attached`     : seed status when a client first attaches
    //
    // All of them do the same thing:
    // context changed -> run `publish` -> refresh `@rustbox_status_right`
    append_hook("after-select-pane", &publish_command)?;
    append_hook("after-select-window", &publish_command)?;
    append_hook("after-new-window", &publish_command)?;
    append_hook("after-split-window", &publish_command)?;
    append_hook("client-attached", &publish_command)?;
    set_option(HOOKS_INSTALLED_OPTION, "1")
}

// Live unload flow 🛑
//
// `stop` should make the current tmux server stop behaving like rustbox even
// though tmux keeps hook definitions in server memory:
//
// 1. `@rustbox_enabled = 0`
//    -> old hook-driven `publish` calls become harmless no-ops
// 2. if `status-right` still points at `#{@rustbox_status_right}`
//    -> blank it so the rustbox status disappears immediately
// 3. clear the visible rustbox state options
// 4. refresh the status line once
pub fn disable_theme() -> Result<(), String> {
    set_option(ENABLED_OPTION, "0")?;

    if show_option("status-right").as_deref() == Some("#{@rustbox_status_right}") {
        set_option("status-right", "")?;
    }

    set_option(STATUS_OPTION, "")?;
    set_option(ACTIVE_PATH_OPTION, "")?;
    refresh_status_line()
}

// Append tmux hooks instead of replacing unrelated user hooks on the same
// event.
fn append_hook(name: &str, command: &str) -> Result<(), String> {
    let status = Command::new("tmux")
        .args(["set-hook", "-ag", name, command])
        .status()
        .map_err(|error| format!("failed to run tmux set-hook for {name}: {error}"))?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "tmux set-hook for {name} exited with status {status}"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        refresh_args, set_option_args, theme_enabled, ACTIVE_PATH_OPTION, DAEMON_PID_OPTION,
        DEFAULT_GIT_REFRESH_SECS, ENABLED_OPTION, GIT_REFRESH_OPTION, STATUS_OPTION,
    };

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

    #[test]
    fn exposes_tmux_user_option_names() {
        assert_eq!(STATUS_OPTION, "@rustbox_status_right");
        assert_eq!(ACTIVE_PATH_OPTION, "@rustbox_active_path");
        assert_eq!(DAEMON_PID_OPTION, "@rustbox_daemon_pid");
        assert_eq!(GIT_REFRESH_OPTION, "@rustbox_git_refresh_seconds");
        assert_eq!(ENABLED_OPTION, "@rustbox_enabled");
        assert_eq!(DEFAULT_GIT_REFRESH_SECS, 30);
    }

    #[test]
    fn treats_missing_enabled_flag_as_enabled() {
        assert!(theme_enabled());
    }
}
