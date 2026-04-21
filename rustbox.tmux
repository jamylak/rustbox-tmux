#!/usr/bin/env bash

set -eu

CURRENT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY_PATH="${RUSTBOX_TMUX_BIN:-${CURRENT_DIR}/target/release/rustbox-tmux}"

# Startup/reload flow:
# tmux `run-shell rustbox.tmux`
#   -> maybe `cargo build --release`
#   -> run `target/release/rustbox-tmux init`
#
# Important:
# - this script runs on tmux startup / config reload
# - it is NOT part of the steady-state status redraw path
# - the extra checks below are just file timestamp checks, which are cheap next
#   to a real Cargo build
binary_needs_build() {
    # No compiled binary yet -> build one.
    if [[ ! -x "${BINARY_PATH}" ]]; then
        return 0
    fi

    # Cargo manifest changed -> build settings / deps may have changed.
    if [[ "${CURRENT_DIR}/Cargo.toml" -nt "${BINARY_PATH}" ]]; then
        return 0
    fi

    # Lockfile changed -> dependency resolution changed.
    if [[ -f "${CURRENT_DIR}/Cargo.lock" && "${CURRENT_DIR}/Cargo.lock" -nt "${BINARY_PATH}" ]]; then
        return 0
    fi

    # Any Rust source newer than the binary means the code changed.
    while IFS= read -r source_path; do
        if [[ "${source_path}" -nt "${BINARY_PATH}" ]]; then
            return 0
        fi
    done < <(find "${CURRENT_DIR}/src" -type f -name '*.rs')

    # Nothing relevant changed -> reuse the current release binary.
    return 1
}

if binary_needs_build; then
    cargo build --quiet --release --manifest-path "${CURRENT_DIR}/Cargo.toml"
fi

# `init` configures tmux once, publishes one status value, and makes sure the
# background updater exists. Hooks then call the compiled binary directly.
"${BINARY_PATH}" init
