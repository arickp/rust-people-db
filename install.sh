#!/bin/bash
set -euo pipefail

INSTALL_ROOT=${INSTALL_ROOT:-$HOME/.local}
APP_DOMAIN="people-db"

echo "== Building binaries =="
cargo build --release

mkdir -p "$INSTALL_ROOT/share/icons"
cp -Rv data/icons/* "$INSTALL_ROOT/share/icons"

mkdir -p "$INSTALL_ROOT/share/applications"
cp -v "data/${APP_DOMAIN}.gtk-desktop" \
      "$INSTALL_ROOT/share/applications/${APP_DOMAIN}.desktop"

mkdir -p "$INSTALL_ROOT/bin"
cp -v target/release/${APP_DOMAIN} "$INSTALL_ROOT/bin"
cp -v target/release/${APP_DOMAIN}-gtk "$INSTALL_ROOT/bin"

echo
echo "== Updating GNOME icon cache =="
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "$INSTALL_ROOT/share/icons/hicolor"
else
    echo "gtk-update-icon-cache not found, skipping icon cache refresh" >&2
fi

echo
echo "== Installing translations =="
if ! command -v msgfmt >/dev/null 2>&1; then
    echo "Error: msgfmt (gettext) is required but not installed." >&2
    exit 1
fi

for dir in locales/*/; do
    lang=$(basename "$dir")
    target_dir="$INSTALL_ROOT/share/locale/$lang/LC_MESSAGES"
    mkdir -p "$target_dir"
    msgfmt "$dir/people-db.po" -o "$target_dir/people-db.mo"
done

echo
echo "== Checking binary availability =="
if ! command -v ${APP_DOMAIN}-gtk >/dev/null 2>&1; then
    echo "Warning: ${APP_DOMAIN}-gtk not found in your PATH after install!" >&2
else
    echo "Found: $(command -v ${APP_DOMAIN}-gtk)"
fi

echo
echo "Installation complete."

