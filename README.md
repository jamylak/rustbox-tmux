# rustbox-tmux

🚧 Work-in-progress Rust port of `jamylak/gruvbox-tmux`, which is itself a fork
of [motaz-shokry/gruvbox-tmux](https://gitlab.com/motaz-shokry/gruvbox-tmux).

The goal is to keep the same main user-facing interface and overall plugin
shape as `jamylak/gruvbox-tmux`, but move the implementation toward one
long-lived daemon with precomputed state instead of lots of shell and subshell
work in the hot path.

This repo is not feature-complete yet. It currently packages the existing Rust
status widgets as a runnable tmux status-right theme entrypoint.

Current subcommands:

- `help`: print usage
- `render`: print the current static status string
- `publish`: publish the current status into a tmux user option
- `daemon`: keep the published status fresh in the background
- `init`: configure `status-right`, publish once, and ensure the updater exists

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

Publish the current status into tmux once:

```bash
cargo run -- publish
```

Expected behavior:

- publishes the current status into `@rustbox_status_right`
- nudges tmux to redraw the status line once

Start the background updater:

```bash
cargo run -- daemon
```

## tmux Wiring

Use the repo-root loader from `.tmux.conf`:

```tmux
run-shell "/Users/james/proj/rustbox-tmux/rustbox.tmux"
```

That loader reuses the existing release binary when it is up to date. If the
source tree is newer, it rebuilds, then points `status-right` at
`#{@rustbox_status_right}`, installs the minimal refresh hooks, publishes an
initial value, and starts the background updater if it is not already running.

## Test It

Run the unit tests:

```bash
cargo test
```

The current tests cover command parsing, tmux argument construction, and the
static renderer output.

## How It Works

Big picture:

```text
┌──────────────────────────────────────────────────────────────────────┐
│ .tmux.conf                                                          │
│   run-shell "/Users/james/proj/rustbox-tmux/rustbox.tmux"           │
└──────────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌──────────────────────────────────────────────────────────────────────┐
│ rustbox.tmux                                                        │
│ 1. check whether `target/release/rustbox-tmux` is stale            │
│ 2. `cargo build --release` only if missing/outdated                │
│ 3. run `rustbox-tmux init`                                         │
└──────────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌──────────────────────────────────────────────────────────────────────┐
│ rustbox-tmux init                                                   │
│ 1. set `status-right` -> `#{@rustbox_status_right}`                │
│ 2. install tmux hooks for context changes                          │
│ 3. publish one fresh status value immediately                      │
│ 4. ensure one background daemon is running                         │
└──────────────────────────────────────────────────────────────────────┘
                         │                              │
                         │                              │
                         ▼                              ▼
┌─────────────────────────────────────┐    ┌──────────────────────────┐
│ tmux hooks                          │    │ background daemon         │
│ - after-select-pane                 │    │ loop every 5s             │
│ - after-select-window               │    │ -> `publish_once(None)`   │
│ - after-new-window                  │    │                           │
│ - after-split-window                │    └──────────────────────────┘
│ - client-attached                   │
│                                     │
│ all run: `rustbox-tmux publish`     │
└─────────────────────────────────────┘
                         │
                         ▼
┌──────────────────────────────────────────────────────────────────────┐
│ rustbox-tmux publish                                                │
│ 1. ask tmux for the active pane path                                │
│ 2. render git + forge stub + metrics                                │
│ 3. write result into `@rustbox_status_right`                        │
│ 4. ask tmux to redraw                                               │
└──────────────────────────────────────────────────────────────────────┘
                                  │
                                  ▼
┌──────────────────────────────────────────────────────────────────────┐
│ tmux status line                                                    │
│ `status-right "#{@rustbox_status_right}"`                          │
│                                                                      │
│ tmux redraws the already-published value instead of running a shell │
│ script for every status-line paint.                                 │
└──────────────────────────────────────────────────────────────────────┘
```

### When It Runs

- `.tmux.conf` load / re-source:
  `rustbox.tmux` runs once to bootstrap the compiled binary.
- pane/window/client context changes:
  tmux fires a hook, which runs `rustbox-tmux publish`.
- background refresh:
  the daemon wakes up every 5 seconds to keep metrics fresh even if you are
  sitting in one pane.
- status redraw:
  tmux only reads `#{@rustbox_status_right}`. It does not run `cargo`, the
  loader script, or a widget shell script on every redraw.

### Current Inefficiencies

This is the honest list of what is still not ideal:

- `rustbox.tmux` still does a few file timestamp checks on startup/reload.
  That is cheap, but it is still shell work.
- tmux hooks still spawn a short-lived `rustbox-tmux publish` process on every
  pane/window/split/attach event.
- the daemon polls every 5 seconds even when nothing changed.
- the git widget still shells out to `git`.
- macOS metrics still shell out to `sysctl`, `ps`, and `vm_stat`.
- the forge section is still just the current stub, so that part of the
  architecture is present but not useful yet.

### Hot Path vs Cold Path

```text
Cold path  🔧
  startup / config reload
    -> loader checks timestamps
    -> maybe build
    -> init

Warm path  🔁
  pane/window/client events
    -> short-lived `publish`

Warm path  ⏱
  every 5s
    -> daemon refresh

Hot path   ⚡
  tmux redraw
    -> read `#{@rustbox_status_right}`
    -> no Cargo
    -> no loader
    -> no widget shell script
```
