#!/usr/bin/env bash
# removes the loop CLI installed by install.sh
set -euo pipefail

INSTALL_DIR="${LOOP_INSTALL_DIR:-$HOME/.local/bin}"
MARKER="# added by loop-sdk installer"

if [ -f "$INSTALL_DIR/loop" ]; then
    rm "$INSTALL_DIR/loop"
    echo "removed $INSTALL_DIR/loop"
else
    echo "nothing installed at $INSTALL_DIR/loop"
fi

for profile in "${ZDOTDIR:-$HOME}/.zshrc" "$HOME/.bash_profile" "$HOME/.bashrc" "$HOME/.profile"; do
    if grep -qs "$MARKER" "$profile"; then
        tmp="$(mktemp)"
        grep -v "$MARKER" "$profile" >"$tmp"
        mv "$tmp" "$profile"
        echo "removed PATH entry from $profile"
    fi
done

echo "done"
