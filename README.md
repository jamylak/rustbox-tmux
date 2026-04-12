# rustbox-tmux

Minimal Rust bootstrap for a tmux status system.

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
