mod daemon;
mod render;

use crate::render::Renderer;
use std::env;
use std::process::ExitCode;

fn main() -> ExitCode {
    match parse_command(env::args()) {
        Ok(Command::Help) => {
            print_help();
            ExitCode::SUCCESS
        }
        Ok(Command::Daemon) => match daemon::run_daemon() {
            Ok(()) => ExitCode::SUCCESS,
            Err(message) => {
                eprintln!("{message}");
                ExitCode::from(1)
            }
        },
        Ok(Command::Render) => {
            let mut renderer = Renderer::new();
            println!("{}", renderer.render());
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("{message}");
            print_help();
            ExitCode::from(2)
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    Help,
    Daemon,
    Render,
}

fn parse_command(args: impl IntoIterator<Item = String>) -> Result<Command, String> {
    let mut args = args.into_iter();
    let _program = args.next();

    match args.next().as_deref() {
        None | Some("help") | Some("--help") | Some("-h") => Ok(Command::Help),
        Some("daemon") => Ok(Command::Daemon),
        Some("render") => Ok(Command::Render),
        Some(other) => Err(format!("unknown subcommand: {other}")),
    }
}

fn print_help() {
    println!("rustbox-tmux");
    println!();
    println!("USAGE:");
    println!("    rustbox-tmux <SUBCOMMAND>");
    println!();
    println!("SUBCOMMANDS:");
    println!("    daemon    Start the long-lived status daemon");
    println!("    render    Print the current rendered status string");
    println!("    help      Show this help text");
}

#[cfg(test)]
mod tests {
    use super::{parse_command, Command};

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
        assert_eq!(command, Command::Render);
    }

    #[test]
    fn rejects_unknown_subcommand() {
        let error = parse_command(vec_of(&["rustbox-tmux", "wat"])).unwrap_err();
        assert!(error.contains("unknown subcommand"));
    }
}
