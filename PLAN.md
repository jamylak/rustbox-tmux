# rustbox-tmux Plan

## Goal

Recreate the jamylak `gruvbox-tmux` plugin experience, but with a faster
architecture and a cleaner long-term data path.

The end result should look and feel like the original plugin:

- a polished tmux status line, not a generic debug bar
- the same overall widget mix and day-to-day usefulness as the original plugin
- support custom icons for selected processes and contexts
- fast steady-state redraws driven by precomputed state
- tiny reviewable diffs while the implementation grows toward parity

## Target Plugin Shape

The plugin should present as a complete tmux theme/status plugin rather than a
single technical widget.

At a high level, the finished plugin should include:

- a left/right status layout that feels recognizably aligned with the original plugin
- cohesive separators, spacing, colors, and section ordering
- support for custom per-process icons where that improves the status display
- a mix of always-visible local system state plus repository-aware context
- optional forge/network-backed context that refreshes in the background
- graceful degradation when data sources are unavailable

The status line should be built from concrete sections such as:

- session and tmux context
- system metrics
  - CPU / load style summary
  - memory summary
- local git state
  - branch
  - dirty state
  - counts
  - ahead / behind
- forge state later
  - PR / MR summary
  - review state
  - CI state

The exact ordering and formatting can evolve, but the final result should read
like a polished daily-driver status line with the same general surface area as
the original plugin.

Build that tmux status system with the render hot path engineered for raw speed:

- no shell scripting in the hot path
- no `git`, `gh`, `glab`, `jq`, `top`, `vm_stat`, `pmset`, or `free` subprocesses in the hot path
- one long-lived daemon process
- tmux reads precomputed state, not live-computed widget logic
- tiny reviewable diffs at every step

## Non-Goals For Early Iterations

- full feature parity on day one
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
   - preferred final phase: daemon updates a tmux user option
   - `status-right` reads `#{@rustbox_status_right}`

3. widget engines
   - git: direct repository access
   - metrics: direct OS sampling
   - forge: direct HTTP clients and cached auth-aware polling

## Planned Widget Inventory

These are the main plugin sections we expect to add over time:

- tmux/session context
  - custom icons for selected processes or tools when relevant
- metrics
  - CPU or load summary
  - memory summary
- local git
  - branch
  - dirty state
  - counts
  - ahead / behind
- forge
  - pull request or merge request summary
  - review status
  - CI status

Not every section needs to land immediately, but the plan should keep moving
toward this overall plugin surface.

## Tiny-Diff Roadmap

1. Bootstrap crate and command surface.
   - Minimal binary with subcommands.
   - No dependencies unless required.

2. Add a daemon skeleton and basic tmux publication path.
   - Single-threaded event loop first.
   - Static rendered output.
   - `status-right` reads daemon-owned state.

3. Add renderer state model.
   - Preallocated string assembly where useful.
   - No widget work in the client.

4. Get the feel of the plugin right before chasing parity.
   - Land the basic layout and composition path first.
   - Make the status line feel like a real plugin, not scaffolding.
   - Prefer placeholder-backed sections over premature heavy integrations.
   - Establish the section ordering and visual rhythm of the final plugin.

5. Add the quickest low-risk widgets first.
   - Prefer local state with simple direct reads.
   - Validate spacing, separators, truncation, and update flow.
   - Start filling in the planned widget inventory from the easiest wins upward.

6. Add metrics widget with direct integration only.
   - Linux: `/proc`.
   - macOS: stage 1 may use command-backed collection to get real numbers on screen quickly.
   - Replace macOS command spawning with direct/native or daemon-cached collection later.

7. Add local git widget with no shell-outs.
   - Start with branch and dirty state.
   - Then counts.
   - Then ahead/behind.
   - Treat this as an early performance-critical widget, not a shell-out placeholder.

8. Add forge widget later.
   - Direct GitHub/GitLab APIs.
   - Strict background refresh only.

9. Expand toward broader parity with the original plugin.
   - Fill in the remaining useful widgets one by one.
   - Keep the final shape recognizably aligned with the original plugin.

10. Optional long-term polish.
   - Consider a short session startup animation.
   - Keep it optional and bounded, for example a first ~3 second sequence.
   - Never let animation compromise steady-state redraw cost.

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
- Preferred steady state: daemon publishes rendered output into a tmux user option.
- `status-right` should read `#{@rustbox_status_right}` so redraws stay inside tmux.
- Every new widget must define:
  - data source
  - refresh policy
  - invalidation policy
  - worst-case latency impact
- Current optimization note:
  - Linux metrics can stay on direct `/proc` reads.
  - macOS metrics currently need an explicit follow-up to remove command-backed collection from the long-term path.

## Near-Term Next Session Work

1. Keep tightening the renderer and daemon data path.
2. Make the basic status line shape feel intentional.
3. Add the next easiest useful widget with a direct integration.
4. Keep the diff tiny.
   - One subsystem only.
   - No parallel feature work.

## Notes

- The target is not merely "a fast tmux status line"; it is a fast reimplementation
  of the original plugin's experience.
- Short-term delivery order matters:
  - first make the plugin feel real
  - then land the fastest low-risk widgets
  - then tackle heavier git and metrics work
- Long-term polish can exist, but only after the core plugin is already useful.
  - Custom icons are part of the intended surface.
  - Startup animation is optional future polish.
