mod daemon;
mod render;
mod tmux;
mod widgets;

use crate::render::render_to_stdout;
use std::env;
use std::path::PathBuf;
use std::process::ExitCode;

fn main() -> ExitCode {
    match parse_command(env::args()) {
        Ok(Command::Help) => {
            print_help();
            ExitCode::SUCCESS
        }
        Ok(Command::Init) => match run_init() {
            Ok(()) => ExitCode::SUCCESS,
            Err(message) => {
                eprintln!("{message}");
                ExitCode::from(1)
            }
        },
        Ok(Command::Stop) => match run_stop() {
            Ok(()) => ExitCode::SUCCESS,
            Err(message) => {
                eprintln!("{message}");
                ExitCode::from(1)
            }
        },
        Ok(Command::Daemon) => match daemon::run_daemon() {
            Ok(()) => ExitCode::SUCCESS,
            Err(message) => {
                eprintln!("{message}");
                ExitCode::from(1)
            }
        },
        Ok(Command::Render { path }) => {
            render_to_stdout(&daemon::current_render_state(path.as_deref()));
            ExitCode::SUCCESS
        }
        Ok(Command::Publish { path }) => match daemon::publish_once(path.as_deref()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(message) => {
                eprintln!("{message}");
                ExitCode::from(1)
            }
        },
        Err(message) => {
            eprintln!("{message}");
            print_help();
            ExitCode::from(2)
        }
    }
}

// `init` is the tmux-facing bootstrap path.
//
// Diagram:
// 1. configure tmux so `status-right` reads `#{@rustbox_status_right}`
// 2. publish one fresh value right now
// 3. ensure one background updater exists for later refreshes
fn run_init() -> Result<(), String> {
    let binary_path =
        env::current_exe().map_err(|error| format!("failed to locate binary: {error}"))?;
    let binary_path = binary_path
        .to_str()
        .ok_or_else(|| "binary path is not valid UTF-8".to_string())?;

    tmux::configure_theme(binary_path)?;
    daemon::publish_once(None)?;
    daemon::ensure_daemon(&PathBuf::from(binary_path))?;

    Ok(())
}

// `stop` is the tmux-facing unload path for the current server only.
//
// Diagram:
// 1. mark rustbox disabled so old hooks become harmless no-ops
// 2. clear the live rustbox status wiring if tmux is still using it
// 3. terminate the current rustbox daemon for this tmux server
fn run_stop() -> Result<(), String> {
    let binary_path =
        env::current_exe().map_err(|error| format!("failed to locate binary: {error}"))?;
    daemon::stop_current_server(&binary_path)
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Command {
    Help,
    Init,
    Stop,
    Daemon,
    Render { path: Option<PathBuf> },
    Publish { path: Option<PathBuf> },
}

fn parse_command(args: impl IntoIterator<Item = String>) -> Result<Command, String> {
    let mut args = args.into_iter();
    let _program = args.next();

    match args.next().as_deref() {
        None | Some("help") | Some("--help") | Some("-h") => Ok(Command::Help),
        Some("init") => reject_extra_args(args).map(|()| Command::Init),
        Some("stop") => reject_extra_args(args).map(|()| Command::Stop),
        Some("daemon") => reject_extra_args(args).map(|()| Command::Daemon),
        Some("render") => parse_path_command(args).map(|path| Command::Render { path }),
        Some("publish") => parse_path_command(args).map(|path| Command::Publish { path }),
        Some(other) => Err(format!("unknown subcommand: {other}")),
    }
}

// Shared parser for commands that accept at most one target path.
//
// Accepted forms:
// - `render`                    -> `None`
// - `render foo`                -> `Some("foo")`
// - `render --path foo`         -> `Some("foo")`
//
// Rejected forms:
// - `render foo bar`            -> duplicate path error
// - `render foo --path bar`     -> duplicate path error
// - `render --wat`              -> unknown flag error
//
// Flow:
// 1. Scan tokens from left to right.
// 2. `--path` consumes the following token as the path value.
// 3. Any other `-flag` is rejected.
// 4. Any bare token is treated as the positional path.
fn parse_path_command(mut args: impl Iterator<Item = String>) -> Result<Option<PathBuf>, String> {
    let mut path = None;

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--path" => {
                let value = args
                    .next()
                    .ok_or_else(|| "missing value after --path".to_string())?;
                assign_path(&mut path, value)?;
            }
            value if value.starts_with('-') => return Err(format!("unknown flag: {value}")),
            value => assign_path(&mut path, value.to_string())?,
        }
    }

    Ok(path.map(PathBuf::from))
}

// Keep the "one path only" rule in one place so positional and `--path`
// inputs behave the same way instead of whichever branch runs last winning.
fn assign_path(path: &mut Option<String>, value: String) -> Result<(), String> {
    if path.replace(value).is_some() {
        Err("path may only be provided once".to_string())
    } else {
        Ok(())
    }
}

// Keep zero-arg commands strict so bad tmux hook invocations fail loudly.
fn reject_extra_args(args: impl Iterator<Item = String>) -> Result<(), String> {
    let extra: Vec<String> = args.collect();
    if extra.is_empty() {
        Ok(())
    } else {
        Err(format!("unexpected extra arguments: {}", extra.join(" ")))
    }
}

fn print_help() {
    println!("rustbox-tmux");
    println!();
    println!("USAGE:");
    println!("    rustbox-tmux <SUBCOMMAND> [OPTIONS]");
    println!();
    println!("SUBCOMMANDS:");
    println!(
        "    init      Configure tmux status-right, publish once, and replace/start the updater"
    );
    println!("    stop      Disable rustbox in the current tmux server and stop its updater");
    println!("    daemon    Start the long-lived status daemon");
    println!("    render    Print the current rendered status string");
    println!("    publish   Publish the current rendered status into tmux");
    println!("    help      Show this help text");
}

#[cfg(test)]
mod tests {
    use super::{parse_command, Command};
    use std::path::PathBuf;

    fn vec_of(items: &[&str]) -> Vec<String> {
        items.iter().map(|item| item.to_string()).collect()
    }

    #[test]
    fn defaults_to_help() {
        let command = parse_command(vec_of(&["rustbox-tmux"])).unwrap();
        assert_eq!(command, Command::Help);
    }

    #[test]
    fn parses_render() {
        let command = parse_command(vec_of(&["rustbox-tmux", "render"])).unwrap();
        assert_eq!(command, Command::Render { path: None });
    }

    #[test]
    fn parses_stop() {
        let command = parse_command(vec_of(&["rustbox-tmux", "stop"])).unwrap();
        assert_eq!(command, Command::Stop);
    }

    #[test]
    fn parses_publish_with_path_flag() {
        let command =
            parse_command(vec_of(&["rustbox-tmux", "publish", "--path", "/tmp/demo"])).unwrap();
        assert_eq!(
            command,
            Command::Publish {
                path: Some(PathBuf::from("/tmp/demo")),
            }
        );
    }

    #[test]
    fn rejects_unknown_subcommand() {
        let error = parse_command(vec_of(&["rustbox-tmux", "wat"])).unwrap_err();
        assert!(error.contains("unknown subcommand"));
    }

    #[test]
    fn rejects_duplicate_paths() {
        let error =
            parse_command(vec_of(&["rustbox-tmux", "render", "/tmp/one", "/tmp/two"])).unwrap_err();
        assert!(error.contains("only be provided once"));
    }
}
