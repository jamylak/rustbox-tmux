#!/usr/bin/env bash

set -eu

CURRENT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BASE_CONF="${RUSTBOX_TEST_BASE_CONF:-$HOME/.tmux.conf}"
SOCKET_PATH="${RUSTBOX_TEST_SOCKET:-/tmp/rustbox-tmux-sandbox.sock}"
SESSION_NAME="${RUSTBOX_TEST_SESSION:-rustbox-sandbox}"
TEMP_CONF="$(mktemp /tmp/rustbox-tmux-sandbox.XXXXXX.conf)"
GIT_REFRESH_SECS="${RUSTBOX_GIT_REFRESH_SECONDS:-30}"

cleanup() {
    rm -f "${TEMP_CONF}"
}

trap cleanup EXIT

if [[ ! -f "${BASE_CONF}" ]]; then
    echo "tmux base config not found: ${BASE_CONF}" >&2
    exit 1
fi

cat >"${TEMP_CONF}" <<EOF
# rustbox sandbox flow 🧪
# base tmux config
#   -> source your real dotfiles config
#   -> append a local rustbox overlay from this checkout
#   -> boot a separate tmux server/socket
#
# This keeps your normal tmux server alone while letting you inspect how the
# local rustbox checkout behaves with your actual keybinds/plugins/settings.
source-file "${BASE_CONF}"
set -g @rustbox_git_refresh_seconds ${GIT_REFRESH_SECS}
run-shell "${CURRENT_DIR}/rustbox.tmux"
EOF

tmux -S "${SOCKET_PATH}" kill-server >/dev/null 2>&1 || true

printf 'rustbox sandbox\n'
printf '  socket  %s\n' "${SOCKET_PATH}"
printf '  session %s\n' "${SESSION_NAME}"
printf '  base    %s\n' "${BASE_CONF}"
printf '  overlay %s\n' "${TEMP_CONF}"

exec tmux -f "${TEMP_CONF}" -S "${SOCKET_PATH}" new-session -A -s "${SESSION_NAME}"
