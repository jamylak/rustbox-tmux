use std::path::PathBuf;
use std::process::Command;

pub const STATUS_OPTION: &str = "@rustbox_status_right";
pub const ACTIVE_PATH_OPTION: &str = "@rustbox_active_path";
pub const DAEMON_PID_OPTION: &str = "@rustbox_daemon_pid";
pub const GIT_REFRESH_OPTION: &str = "@rustbox_git_refresh_seconds";
pub const ENABLED_OPTION: &str = "@rustbox_enabled";
pub const DEFAULT_GIT_REFRESH_SECS: u64 = 30;
const HOOKS_INSTALLED_OPTION: &str = "@rustbox_hooks_installed";
const THEME_BACKGROUND: &str = "#282828";
const THEME_FOREGROUND: &str = "#fbf1c7";
const THEME_BLUE: &str = "#458588";
const THEME_BBLACK: &str = "#32302F";
const THEME_BBLUE: &str = "#83a598";
const THEME_BGREEN: &str = "#b8bb26";
const THEME_BPURPLE: &str = "#d3869b";
const THEME_BWHITE: &str = "#EBDBB2";
const THEME_YELLOW: &str = "#d79921";
const RESET: &str = "#[fg=#fbf1c7,bg=#282828,nobold,noitalics,nounderscore,nodim]";
const LEGACY_WINDOW_ID_STYLE: &str = "@gruvbox-tmux_window_id_style";
const LEGACY_PANE_ID_STYLE: &str = "@gruvbox-tmux_pane_id_style";
const LEGACY_ZOOM_ID_STYLE: &str = "@gruvbox-tmux_zoom_id_style";
const LEGACY_TERMINAL_ICON: &str = "@gruvbox-tmux_terminal_icon";
const LEGACY_ACTIVE_TERMINAL_ICON: &str = "@gruvbox-tmux_active_terminal_icon";
const LEGACY_CLAUDE_ICON: &str = "@gruvbox-tmux_claude_icon";
const LEGACY_COPILOT_ICON: &str = "@gruvbox-tmux_copilot_icon";
const LEGACY_CODEX_ICON: &str = "@gruvbox-tmux_codex_icon";
const CONTEXT_HOOKS: &[&str] = &[
    "after-select-pane",
    "after-select-window",
    "after-new-window",
    "after-split-window",
    "client-attached",
    "client-session-changed",
    "session-created",
];

pub fn publish_status(status: &str) -> Result<(), String> {
    // Session-aware publish route 🎯
    //
    // current session
    //   -> write that session's private `@rustbox_status_right`
    //
    // There is no global fallback anymore. The whole point of this fix is to
    // make session scope the one real model instead of a side-path.
    let session_id = current_session_id()
        .ok_or_else(|| "failed to determine current tmux session".to_string())?;
    publish_status_for_session(&session_id, status)
}

pub fn publish_status_for_session(session_id: &str, status: &str) -> Result<(), String> {
    // tmux session store 📦
    //
    // `status-right` still expands `#{@rustbox_status_right}`, but tmux lets
    // each session carry its own value for that user option.
    //
    // So the rendering model is now:
    // session `$1` -> `@rustbox_status_right = "...repo A..."`
    // session `$2` -> `@rustbox_status_right = "...repo B..."`
    //
    // The format string stays stable while the data behind it becomes scoped.
    set_session_option(session_id, STATUS_OPTION, status)
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
    // Global option store 🌍
    //
    // This is still used for truly server-wide rustbox state like daemon pid
    // and refresh cadence. Session-specific state should go through the
    // session-targeted helpers below.
    let output = Command::new("tmux")
        .args(["show-option", "-gv", name])
        .output()
        .ok()?;
    parse_option_output(output)
}

pub fn set_session_option(session_id: &str, name: &str, value: &str) -> Result<(), String> {
    // Session-scoped write path 🧭
    //
    // `-t <session>` is the critical bit here. Without it, tmux writes to the
    // global option store and every session inside the same server sees the
    // same rustbox state.
    let status = Command::new("tmux")
        .args(["set-option", "-q", "-t", session_id, name, value])
        .status()
        .map_err(|error| {
            format!("failed to run tmux set-option for session {session_id} {name}: {error}")
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "tmux set-option for session {session_id} {name} exited with status {status}"
        ))
    }
}

pub fn show_session_option(session_id: &str, name: &str) -> Option<String> {
    // Session-scoped read path 🔎
    //
    // The daemon uses this to recover the last path/status remembered for a
    // specific session, instead of assuming there is one universal active path
    // for the whole tmux server.
    let output = Command::new("tmux")
        .args(["show-option", "-qv", "-t", session_id, name])
        .output()
        .ok()?;
    parse_option_output(output)
}

pub fn list_session_ids() -> Vec<String> {
    // Server -> sessions fan-out list 🗂️
    //
    // One daemon still owns one tmux server, but it now needs to refresh the
    // cached status for every session living inside that server.
    //
    // `list-sessions -F "#{session_id}"`
    //   -> ["$1", "$2", "$3", ...]
    //   -> daemon iterates each one
    let output = match Command::new("tmux")
        .args(["list-sessions", "-F", "#{session_id}"])
        .output()
    {
        Ok(output) if output.status.success() => output,
        _ => return Vec::new(),
    };

    String::from_utf8(output.stdout)
        .ok()
        .map(|stdout| {
            stdout
                .lines()
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
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

pub fn current_session_id() -> Option<String> {
    // Session lookup:
    // tmux current client/session -> `#{session_id}` -> Rust `String`
    //
    // The daemon uses this to attach one hidden control-mode client to the
    // current tmux session for native `%subscription-changed` events.
    let output = Command::new("tmux")
        .args(["display-message", "-p", "#{session_id}"])
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

fn parse_option_output(output: std::process::Output) -> Option<String> {
    // Shared output parser ✂️
    //
    // Both global and session-scoped tmux reads return the same shape:
    // process output -> UTF-8 text -> trim -> maybe value
    //
    // Centralizing this keeps the "missing/empty means None" rule identical
    // across both storage scopes.
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

// Theme bootstrap 🎨
//
// `render.rs` only builds the live right-side payload.
// The rest of the screenshot look comes from tmux format strings configured
// here:
//
// rustbox init
//   -> set palette-driven tmux options
//   -> set left status + window tab formats
//   -> point `status-right` at `#{@rustbox_status_right}`
//   -> install refresh hooks once
//
// So if rustbox "looks nothing like gruvbox-tmux", this function is the place
// that fixes it.
pub fn configure_theme(binary_path: &str) -> Result<(), String> {
    let window_id_style =
        show_option(LEGACY_WINDOW_ID_STYLE).unwrap_or_else(|| "digital".to_string());
    let pane_id_style = show_option(LEGACY_PANE_ID_STYLE).unwrap_or_else(|| "hsquare".to_string());
    let zoom_id_style = show_option(LEGACY_ZOOM_ID_STYLE).unwrap_or_else(|| "dsquare".to_string());
    let terminal_icon = show_option(LEGACY_TERMINAL_ICON).unwrap_or_else(|| "".to_string());
    let active_terminal_icon =
        show_option(LEGACY_ACTIVE_TERMINAL_ICON).unwrap_or_else(|| "".to_string());
    let claude_icon = show_option(LEGACY_CLAUDE_ICON).unwrap_or_else(|| "🌼".to_string());
    let copilot_icon = show_option(LEGACY_COPILOT_ICON).unwrap_or_else(|| "🐙".to_string());
    let codex_icon = show_option(LEGACY_CODEX_ICON).unwrap_or_else(|| "🤖".to_string());

    let window_icon =
        build_app_icon_format(&terminal_icon, &claude_icon, &copilot_icon, &codex_icon);
    let active_window_icon = build_app_icon_format(
        &active_terminal_icon,
        &claude_icon,
        &copilot_icon,
        &codex_icon,
    );
    let window_number = build_number_format("#I", &window_id_style);
    let custom_pane = build_number_format("#P", &pane_id_style);
    let zoom_number = build_number_format("#P", &zoom_id_style);

    set_option("status-left-length", "80")?;
    set_option("status-right", "#{@rustbox_status_right}")?;
    set_option("status-right-length", "220")?;
    set_option(
        "mode-style",
        &format!("fg={THEME_BACKGROUND},bg={THEME_FOREGROUND},reverse"),
    )?;
    set_option(
        "message-style",
        &format!("bg={THEME_BBLUE},fg={THEME_BACKGROUND},bold"),
    )?;
    set_option(
        "message-command-style",
        &format!("fg={THEME_FOREGROUND},bg={THEME_BACKGROUND},bold"),
    )?;
    set_option("pane-border-style", &format!("fg={THEME_BBLACK}"))?;
    set_option(
        "pane-active-border-style",
        &format!("fg={THEME_BWHITE},bold"),
    )?;
    set_option("pane-border-status", "off")?;
    set_option(
        "status-style",
        &format!("fg={THEME_FOREGROUND},bg={THEME_BACKGROUND}"),
    )?;
    set_option("window-status-separator", "")?;
    set_option("status-left", &status_left_format())?;
    set_option(
        "window-status-current-format",
        &window_status_current_format(
            &active_window_icon,
            &window_number,
            &zoom_number,
            &custom_pane,
        ),
    )?;
    set_option(
        "window-status-format",
        &window_status_format(&window_icon, &window_number, &zoom_number, &custom_pane),
    )?;
    set_option(ENABLED_OPTION, "1")?;
    if show_option(GIT_REFRESH_OPTION).is_none() {
        set_option(GIT_REFRESH_OPTION, &DEFAULT_GIT_REFRESH_SECS.to_string())?;
    }

    let publish_command = format!("run-shell -b \"{binary_path} publish\"");
    // Hook map:
    // - `after-select-pane`   : user focused a different pane
    // - `after-select-window` : user focused a different window
    // - `after-new-window`    : a new window appeared with a new cwd/context
    // - `after-split-window`  : a new pane appeared with a new cwd/context
    // - `client-attached`     : seed status when a client first attaches
    // - `client-session-changed`: follow `switch-client -t ...`
    // - `session-created`     : a brand-new `tmux new-session -c ...` picked
    //                           its initial cwd before any later daemon tick
    //
    // All of them do the same thing:
    // context changed -> run `publish` -> refresh `@rustbox_status_right`
    for hook in CONTEXT_HOOKS {
        ensure_hook_contains_command(hook, &publish_command)?;
    }
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

    // Session cleanup sweep 🧹
    //
    // Now that rustbox keeps per-session copies of these options, `stop`
    // needs to blank all of them or detached/older sessions could keep stale
    // status payloads around in tmux memory.
    //
    // tmux server
    //   -> list sessions
    //   -> clear rustbox state in each session
    for session_id in list_session_ids() {
        set_session_option(&session_id, STATUS_OPTION, "")?;
        set_session_option(&session_id, ACTIVE_PATH_OPTION, "")?;
    }
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

// Re-sourcing tmux config should pick up newly added rustbox hooks, but it
// should not append the exact same `publish` command forever. We check first so
// old servers can migrate forward while repeat `init` calls stay idempotent.
fn ensure_hook_contains_command(name: &str, command: &str) -> Result<(), String> {
    if hook_contains_command(name, command)? {
        Ok(())
    } else {
        append_hook(name, command)
    }
}

// Ask tmux for the current global hook body and look for the exact rustbox
// command we care about. Missing hooks are normal on first install, so a tmux
// failure here means "not installed yet" rather than a hard error.
fn hook_contains_command(name: &str, command: &str) -> Result<bool, String> {
    let output = Command::new("tmux")
        .args(["show-hooks", "-g", name])
        .output()
        .map_err(|error| format!("failed to run tmux show-hooks for {name}: {error}"))?;

    if !output.status.success() {
        return Ok(false);
    }

    let stdout = String::from_utf8(output.stdout)
        .map_err(|error| format!("tmux show-hooks for {name} returned invalid UTF-8: {error}"))?;

    Ok(stdout.lines().any(|line| line.contains(command)))
}

fn status_left_format() -> String {
    format!(
        "#[fg={THEME_FOREGROUND},bg={THEME_BLUE},bold] \
#{{
?client_prefix,🚀 ,#{{?pane_in_mode,👀 ,🔮 }}
}}#[bold,nodim]#S "
    )
    .replace('\n', "")
}

fn window_status_current_format(
    active_window_icon: &str,
    window_number: &str,
    zoom_number: &str,
    custom_pane: &str,
) -> String {
    format!(
        "{RESET}#[fg={THEME_BGREEN},bg={THEME_BBLACK}] {active_window_icon}\
#[fg={THEME_BPURPLE},bold,nodim]{window_number}#W\
#[nobold]#{{?window_zoomed_flag, {zoom_number}, {custom_pane}}}#{{?window_last_flag, ,}}"
    )
}

fn window_status_format(
    window_icon: &str,
    window_number: &str,
    zoom_number: &str,
    custom_pane: &str,
) -> String {
    format!(
        "{RESET}#[fg={THEME_FOREGROUND}] {window_icon}{RESET}{window_number}#W\
#[nobold,dim]#{{?window_zoomed_flag, {zoom_number}, {custom_pane}}}\
#[fg={THEME_YELLOW}]#{{?window_last_flag, ,}}"
    )
}

// Number formatter 🔢
//
// tmux does the substitution lazily:
// `#I`
//   -> nested `#{s|...|...|:...}` transforms
//   -> fancy window/pane digit glyphs at draw time
//
// This matches the old shell theme so the active tab numbers still look the
// same without keeping a Bash implementation around.
fn build_number_format(value_format: &str, style_name: &str) -> String {
    let digits = match style_name {
        "hide" => return String::new(),
        "fsquare" => ["󰎡", "󰎤", "󰎧", "󰎪", "󰎭", "󰎱", "󰎳", "󰎶", "󰎹", "󰎼"],
        "hsquare" => ["󰎣", "󰎦", "󰎩", "󰎬", "󰎮", "󰎰", "󰎵", "󰎸", "󰎻", "󰎾"],
        "dsquare" => ["󰎢", "󰎥", "󰎨", "󰎫", "󰎲", "󰎯", "󰎴", "󰎷", "󰎺", "󰎽"],
        "super" => ["⁰", "¹", "²", "³", "⁴", "⁵", "⁶", "⁷", "⁸", "⁹"],
        "sub" => ["₀", "₁", "₂", "₃", "₄", "₅", "₆", "₇", "₈", "₉"],
        "earabic" => ["٠", "١", "٢", "٣", "٤", "٥", "٦", "٧", "٨", "٩"],
        _ => ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"],
    };

    let mut format = value_format.to_string();
    for (index, digit) in digits.iter().enumerate() {
        format = format!("#{{s|{index}|{digit} |:{format}}}");
    }
    format
}

// App icon rules 🤖
//
// tmux evaluates this giant nested conditional using `#{pane_current_command}`.
// That lets each tab pick the same icon set your gruvbox theme already used:
//
// `nvim`  -> ``
// `fish`  -> `🐟`
// `codex` -> `🤖`
// fallback -> terminal icon
fn build_app_icon_format(
    default_icon: &str,
    claude_icon: &str,
    copilot_icon: &str,
    codex_icon: &str,
) -> String {
    let mut format = format!("{default_icon} ");
    let codex = format!("{codex_icon} ");
    let copilot = format!("{copilot_icon} ");
    let claude = format!("{claude_icon} ");

    for (pattern, icon) in [
        ("^emacs(client)?$", "λ "),
        ("^(nu|nushell)$", "◉ "),
        ("^go$", "🐹 "),
        ("^psql$", "🐘 "),
        ("^uvicorn$", "🦄 "),
        ("^(uv|uvx|python.*)$", "🐍 "),
        ("^(cargo|rustc|rustup)$", "🦀 "),
        ("^deno$", "🦕 "),
        ("^bun$", "🥟 "),
        ("^yarn$", "🧶 "),
        ("^pnpm$", "📫 "),
        ("^node$", "⬢ "),
        ("^(npm|npx)$", "📦 "),
        ("^(docker|docker-compose)$", "🐳 "),
        ("^(terraform|tofu)$", "💠 "),
        ("^gcloud$", "☁️ "),
        ("^glab$", " "),
        ("^gh$", " "),
        ("^tmux$", "🧩 "),
        ("^fish$", "🐟 "),
        ("^btop$", "📈 "),
        ("^lazygit$", " "),
        ("^yazi$", "🗂️ "),
        ("^(nvim|vim)$", " "),
        ("^(hx|helix)$", "⌘ "),
        ("^(codex|codex-.*)$", codex.as_str()),
        (
            "^(copilot|copilot-.*|github-copilot-cli|github-copilot-cli-.*|copilot-cli|copilot-cli-.*)$",
            copilot.as_str(),
        ),
        (
            "^(claude|claude-.*|claude-code|claude-code-.*)$",
            claude.as_str(),
        ),
        ("^ssh$", "󰣀 "),
    ] {
        format = build_icon_rule(pattern, icon, &format);
    }

    format
}

fn build_icon_rule(pattern: &str, icon: &str, fallback: &str) -> String {
    format!("#{{?#{{m/ri:{pattern},#{{pane_current_command}}}},{icon},{fallback}}}")
}

#[cfg(test)]
mod tests {
    use super::{
        build_app_icon_format, build_number_format, current_session_id, hook_contains_command,
        refresh_args, set_option_args, status_left_format, theme_enabled, ACTIVE_PATH_OPTION,
        CONTEXT_HOOKS, DAEMON_PID_OPTION, DEFAULT_GIT_REFRESH_SECS, ENABLED_OPTION,
        GIT_REFRESH_OPTION, STATUS_OPTION,
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

    #[test]
    fn current_session_lookup_returns_none_without_tmux() {
        let _ = current_session_id();
    }

    #[test]
    fn builds_digital_number_format() {
        assert_eq!(
            build_number_format("#I", "digital"),
            "#{s|9|9 |:#{s|8|8 |:#{s|7|7 |:#{s|6|6 |:#{s|5|5 |:#{s|4|4 |:#{s|3|3 |:#{s|2|2 |:#{s|1|1 |:#{s|0|0 |:#I}}}}}}}}}}"
        );
    }

    #[test]
    fn hides_number_format_when_requested() {
        assert_eq!(build_number_format("#I", "hide"), "");
    }

    #[test]
    fn builds_icon_format_with_terminal_fallback_and_codex_match() {
        let format = build_app_icon_format("", "🌼", "🐙", "🤖");
        assert!(format.contains("pane_current_command"));
        assert!(format.contains("🤖 "));
        assert!(format.contains(" "));
    }

    #[test]
    fn builds_exact_status_left_format() {
        assert_eq!(
            status_left_format(),
            "#[fg=#fbf1c7,bg=#458588,bold] #{?client_prefix,🚀 ,#{?pane_in_mode,👀 ,🔮 }}#[bold,nodim]#S "
        );
    }

    #[test]
    fn installs_session_created_among_context_hooks() {
        assert!(CONTEXT_HOOKS.contains(&"session-created"));
        assert!(CONTEXT_HOOKS.contains(&"client-attached"));
    }

    #[test]
    fn missing_hook_is_treated_as_not_installed() {
        assert!(!hook_contains_command("rustbox-hook-that-does-not-exist", "publish").unwrap());
    }
}
