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
#[fg=green]rustbox-tmux bootstrap
```

Try the daemon entrypoint:

```bash
cargo run -- daemon
```

Expected behavior:

- publishes `#[fg=green]rustbox-tmux bootstrap` into `@rustbox_status_right`
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

## Current Local Issue

On this machine, the project does not currently build because the installed Rust toolchain is broken before crate compilation starts. `cargo run` and `cargo test` both fail while invoking `rustc -vV` with a dynamic linker error caused by a Rust/LLVM mismatch:

```text
dyld: Symbol not found ... librustc_driver ... libLLVM.dylib
```

Until that toolchain issue is fixed, the commands above are the right way to run and test the project, but they will fail locally before executing this crate.
