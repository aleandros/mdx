#!/usr/bin/env bash
set -e

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
HOOKS_DIR="$REPO_ROOT/.git/hooks"

for hook in pre-commit pre-push; do
    src="$REPO_ROOT/hooks/$hook"
    dst="$HOOKS_DIR/$hook"

    if [ -e "$dst" ] && [ ! -L "$dst" ]; then
        echo "Warning: $dst exists and is not a symlink. Backing up to ${dst}.bak"
        mv "$dst" "${dst}.bak"
    fi

    ln -sf "$src" "$dst"
    chmod +x "$src"
    echo "Installed $hook hook"
done

echo "Done. Hooks installed."
