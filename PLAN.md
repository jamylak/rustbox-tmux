# rustbox-tmux Plan

## Goal

Build a tmux status system with the render hot path engineered for raw speed first:

- no shell scripting in the hot path
- no `git`, `gh`, `glab`, `jq`, `top`, `vm_stat`, `pmset`, or `free` subprocesses in the hot path
- one long-lived daemon process
- tmux reads precomputed state, not live-computed widget logic
- tiny reviewable diffs at every step

## Non-Goals For Early Iterations

- feature parity with the Bash plugin
- portability beyond macOS/Linux
- pretty abstractions before the data path is correct
- background network integrations before local-state performance is solid

## First Principles

- Persistent process beats repeated process startup.
- In-memory cache beats filesystem cache on the hot path.
- Push-based invalidation beats polling where practical.
- Direct library/API integration beats shelling out.
- A slower architecture in Rust is still a slower architecture.

## Target Architecture

1. `rustbox-tmuxd` daemon
   - long-lived process
   - owns all state and refresh scheduling
   - renders the final right-status string

2. tmux integration layer
   - initial phase: small client prints current rendered string
   - later phase: daemon updates a tmux user option or socket-fed client

3. widget engines
   - git: direct repository access
   - metrics: direct OS sampling
   - battery: direct OS integration
   - forge: direct HTTP clients and cached auth-aware polling

## Tiny-Diff Roadmap

1. Bootstrap crate and command surface.
   - Minimal binary with subcommands.
   - No dependencies unless required.

2. Add a daemon skeleton.
   - Single-threaded event loop first.
   - Static rendered output.

3. Add a cheap IPC path.
   - Unix socket preferred.
   - Client asks for the latest rendered string.

4. Add renderer state model.
   - Preallocated string assembly where useful.
   - No widget work in the client.

5. Add metrics widget with direct integration only.
   - Linux: `/proc`.
   - macOS: native APIs if required.

6. Add battery widget with direct integration only.
   - Linux: `/sys/class/power_supply`.
   - macOS: IOKit / power APIs.

7. Add local git widget with no shell-outs.
   - Start with branch and dirty state.
   - Then counts.
   - Then ahead/behind.

8. Add tmux wiring.
   - Small integration snippet.
   - Measure redraw cost.

9. Add forge widget later.
   - Direct GitHub/GitLab APIs.
   - Strict background refresh only.

## Performance Rules

- No shell-outs in the render hot path.
- Avoid dynamic allocation churn where simple reuse works.
- Avoid threads until measurement justifies them.
- Prefer stable, predictable latency over bursty freshness.
- Daemon startup must be idempotent in the Rust binary, not in tmux shell glue.
- Use one well-known per-user control socket or lock path.
- On daemon start:
  - if a live daemon answers on the control socket, exit immediately
  - if the socket is stale, remove it and become the daemon
- Every new widget must define:
  - data source
  - refresh policy
  - invalidation policy
  - worst-case latency impact

## Near-Term Next Session Work

1. Implement a real `daemon` subcommand.
2. Implement a real `render` subcommand backed by shared daemon state.
3. Decide whether the first IPC should be:
   - Unix domain socket
   - lock-free shared file
4. Keep the diff tiny.
   - One subsystem only.
   - No parallel feature work.

## Notes

- Local `rustc` is currently broken on this machine, so initial scaffolding may not compile until the toolchain is repaired.
- That does not change the architecture direction.
