#!/usr/bin/env bash
# builds the loop CLI in release mode and installs it on the PATH as `loop`
set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
INSTALL_DIR="${LOOP_INSTALL_DIR:-$HOME/.local/bin}"
MARKER="# added by loop-sdk installer"

echo "building loop-cli (release)..."
cargo build --release -p loop-cli --manifest-path "$REPO_DIR/crates/Cargo.toml"

mkdir -p "$INSTALL_DIR"
install -m 755 "$REPO_DIR/crates/target/release/loop-cli" "$INSTALL_DIR/loop"
echo "installed $INSTALL_DIR/loop"

profile_for_shell() {
    case "$(basename "${SHELL:-/bin/sh}")" in
        zsh) echo "${ZDOTDIR:-$HOME}/.zshrc" ;;
        bash) [ "$(uname)" = "Darwin" ] && echo "$HOME/.bash_profile" || echo "$HOME/.bashrc" ;;
        *) echo "$HOME/.profile" ;;
    esac
}

case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *)
        PROFILE="$(profile_for_shell)"
        if ! grep -qs "$MARKER" "$PROFILE"; then
            printf '\nexport PATH="%s:$PATH" %s\n' "$INSTALL_DIR" "$MARKER" >>"$PROFILE"
            echo "added $INSTALL_DIR to PATH in $PROFILE"
            echo "restart your shell (or run: source $PROFILE) to pick it up"
        fi
        ;;
esac

"$INSTALL_DIR/loop" --version
echo "done — try: loop --help"
