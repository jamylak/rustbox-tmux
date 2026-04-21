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
- `init`: configure `status-right`, publish once, and replace/start the updater

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
set -g @rustbox_git_refresh_seconds 30
```

That loader reuses the existing release binary when it is up to date. If the
source tree is newer, it rebuilds, then points `status-right` at
`#{@rustbox_status_right}`, installs the minimal refresh hooks, publishes an
initial value, and replaces the old daemon so a rebuilt binary actually takes
over after reload.

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
│ 4. replace/start one background daemon for this tmux server        │
└──────────────────────────────────────────────────────────────────────┘
                         │                              │
                         │                              │
                         ▼                              ▼
┌─────────────────────────────────────┐    ┌──────────────────────────┐
│ tmux hooks                          │    │ background daemon         │
│ - after-select-pane                 │    │ loop every 5s             │
│ - after-select-window               │    │ -> metrics refresh        │
│ - after-new-window                  │    │ -> git refresh every 30s* │
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

`*` configurable via `@rustbox_git_refresh_seconds`

### When It Runs

- `.tmux.conf` load / re-source:
  `rustbox.tmux` runs once to bootstrap the compiled binary.
- pane/window/client context changes:
  tmux fires a hook, which runs `rustbox-tmux publish`.
- background refresh:
  the daemon wakes up every 5 seconds to keep metrics fresh even if you are
  sitting in one pane.
- git polling:
  the daemon reuses the cached git section until `@rustbox_git_refresh_seconds`
  expires. The default is 30 seconds.
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
- the git widget still shells out to `git`, though background polling is now
  rate-limited separately from the 5-second metrics loop.
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
    -> daemon metrics refresh

Warm path  🐙
  every 30s by default
    -> daemon git refresh for the current path

Hot path   ⚡
  tmux redraw
    -> read `#{@rustbox_status_right}`
    -> no Cargo
    -> no loader
    -> no widget shell script
```

## FAQ

### When is tmux actually refreshing UI stuff, and why?

There are two different things happening:

- status content refresh:
  Rust recalculates the string and writes a new value into
  `@rustbox_status_right`.
- tmux UI redraw:
  tmux repaints the status line using whatever value is already sitting in
  `@rustbox_status_right`.

Current refresh triggers:

- startup / config reload:
  `rustbox.tmux` runs `rustbox-tmux init`, which publishes once immediately.
- pane or window context changes:
  the installed hooks run `rustbox-tmux publish` so the status follows the
  currently focused pane path.
- idle background refresh:
  the daemon wakes up every 5 seconds so CPU/RAM numbers do not stay stale
  forever when you sit in one pane.
- background git refresh:
  the daemon keeps the last git section cached and only refreshes it when the
  git interval expires. The default is 30 seconds.

Why the explicit redraw call?

```text
Rust publishes a new value
  -> tmux still has to repaint
  -> `refresh-client -S` nudges that repaint now
```

Without that redraw nudge, the updated value would still land in tmux, but you
would wait for tmux's next natural redraw point to see it.

### Is 5 seconds a normal amount?

For lightweight system metrics, yes, a 5-10 second poll is a pretty normal
"fresh enough" interval.

For git polling, the default is now separate and slower:

- metrics loop:
  `5s`
- git background polling:
  `30s` by default via `@rustbox_git_refresh_seconds`

That split is more reasonable for the current architecture:

- CPU/RAM want a short interval
- git does not need to be hammered at the same cadence
- hook-driven `publish` still updates git immediately when pane/window context
  changes

So the current answer is:

- `5s` is normal for metrics
- `30s` is the more acceptable default for background git polling here

### Is there one daemon for many tmux sessions and repos?

Per tmux server, yes.

```text
one tmux server/socket
  -> one `@rustbox_daemon_pid`
  -> one `@rustbox_status_right`
  -> one `@rustbox_active_path`
  -> one daemon process
```

That means:

- multiple sessions inside the same tmux server share the same daemon
- multiple windows/panes inside the same tmux server share the same published
  status option
- the daemon does not iterate over every repo in every pane
- it refreshes only the single currently remembered active path

This is an important current limitation:

- if two clients on the same tmux server are focused on different repos, the
  last `publish` wins
- this is not yet a per-session or per-client status architecture

If you use separate tmux servers via different sockets, each server can end up
with its own daemon.

### Do we still shell out to git every 5 seconds?

Not in the background loop anymore.

Current path:

```text
every 5s
  -> daemon refreshes metrics

every 30s by default
  -> daemon refreshes cached git section for the current path
  -> git widget shells out to `git`
```

Also, every hook-driven `publish` does an immediate git refresh for the current
pane path.

So the current design is:

- not "scan every repo every 5 seconds"
- not "run git every 5 seconds in the background"
- but still "run git immediately on context-change hooks"
- and "run git periodically for the current remembered path"

That is acceptable for a small current-feature prototype, but it is one of the
remaining inefficiencies called out above.

### What happens if I refresh tmux config?

Reloading config runs the loader again.

Current behavior:

- the loader reuses the release binary unless the source tree is newer
- `init` republishes the current status once
- hook installation is guarded, so hooks do not stack on every reload
- `init` terminates the previous daemon and starts a fresh one
- that means a rebuilt binary actually replaces the old in-memory daemon after
  reload

So config reload now does two things:

- self-heal if the daemon died
- force an upgrade if the binary changed

### What happens if I remove the `run-shell` line from tmux config?

Removing it from config does **not** automatically unload what is already
running in the current tmux server.

In the current live server:

- the already-installed hooks remain in tmux memory
- the existing daemon keeps running
- `status-right` stays pointed at `#{@rustbox_status_right}` until changed

On the next fresh tmux server start:

- nothing bootstraps
- no hooks get installed
- no daemon gets started

So removing the line prevents future startup, but it does not retroactively
tear down the current server state.

Current practical solution:

```text
1. remove the `run-shell` line from config
2. kill the rustbox daemon pid
3. restart the tmux server
```

Example shell command for step 2:

```bash
kill "$(tmux show-option -gv @rustbox_daemon_pid)"
```

That is the clean unload path today because the hooks live in tmux server
memory for the lifetime of that server.

### How do I kill the daemon?

Current daemon pid:

```bash
tmux show-option -gv @rustbox_daemon_pid
```

Kill it:

```bash
kill "$(tmux show-option -gv @rustbox_daemon_pid)"
```

Optional cleanup:

```bash
tmux set-option -gu @rustbox_daemon_pid
tmux set-option -gu @rustbox_status_right
```

The cleanest full unload is still:

```text
1. remove the `run-shell` line from config
2. kill the daemon pid
3. restart the tmux server
```

That avoids trying to surgically remove only the hook entries that rustbox
added.
