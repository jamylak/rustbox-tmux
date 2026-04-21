#!/usr/bin/env bash

set -eu

CURRENT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BINARY_PATH="${RUSTBOX_TMUX_BIN:-${CURRENT_DIR}/target/release/rustbox-tmux}"
ACTION="${1:-init}"

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

case "${ACTION}" in
    init)
        if binary_needs_build; then
            cargo build --quiet --release --manifest-path "${CURRENT_DIR}/Cargo.toml"
        fi
        ;;
    stop)
        # `stop` is the teardown path, so do not trigger a rebuild just to
        # disable the current tmux server.
        ;;
    *)
        echo "usage: rustbox.tmux [init|stop]" >&2
        exit 2
        ;;
esac

if [[ ! -x "${BINARY_PATH}" ]]; then
    echo "rustbox-tmux binary not found at ${BINARY_PATH}" >&2
    exit 1
fi

# Loader handoff:
# `init`
#   -> configure tmux, publish once, replace/start daemon
# `stop`
#   -> disable rustbox in the current tmux server and stop its daemon
"${BINARY_PATH}" "${ACTION}"
