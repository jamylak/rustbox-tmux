# rustbox-tmux

🚧 Work-in-progress Rust port of `jamylak/gruvbox-tmux`, which is itself a fork
of [motaz-shokry/gruvbox-tmux](https://gitlab.com/motaz-shokry/gruvbox-tmux).

The goal is to keep the same main user-facing interface and overall plugin
shape as `jamylak/gruvbox-tmux`, but move the implementation toward one
long-lived daemon with precomputed state instead of lots of shell and subshell
work in the hot path.

This repo is not feature-complete yet. It is an active port with the renderer,
daemon, metrics, and git slices being brought over step by step.

Current subcommands:

- `help`: print usage
- `render`: print the current static status string
- `daemon`: publish the current status into a tmux user option

## Prerequisites

- Rust toolchain with working `cargo` and `rustc`

## Run It

Show help:

```bash
cargo run -- --help
```

Render the current status string:

```bash
cargo run -- render
```

Expected output:

```text
#[fg=colour142]▒  main#[fg=colour244] | #[fg=colour214]▒  --#[fg=colour244] | #[fg=colour109]▒ 🧠 --% #[fg=colour108]💾 --%
```

Try the daemon entrypoint:

```bash
cargo run -- daemon
```

Expected behavior:

- publishes `#[fg=colour142]▒  main#[fg=colour244] | #[fg=colour214]▒  --#[fg=colour244] | #[fg=colour109]▒ 🧠 --% #[fg=colour108]💾 --%` into `@rustbox_status_right`
- nudges tmux to redraw the initial status line once
- stays alive as the long-lived daemon process

## tmux Wiring

Point `status-right` at the user option managed by the daemon:

```tmux
set -g status-right "#{@rustbox_status_right}"
```

## Test It

Run the unit tests:

```bash
cargo test
```

The current tests cover command parsing, tmux argument construction, and the
static renderer output.
