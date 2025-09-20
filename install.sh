#!/bin/bash
INSTALL_ROOT=$HOME/.local

cargo build --release
mkdir -p $INSTALL_ROOT/share/icons
cp -Rv data/icons/* $INSTALL_ROOT/share/icons
mkdir -p $INSTALL_ROOT/share/applications
cp -v data/people-db.gtk-desktop $INSTALL_ROOT/share/applications
mkdir -p $INSTALL_ROOT/bin
cp -v target/release/people-db $INSTALL_ROOT/bin
cp -v target/release/people-db-gtk $INSTALL_ROOT/bin

# update gnome icon cache
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t $INSTALL_ROOT/share/icons/hicolor
else
    echo "gtk-update-icon-cache not found, skipping icon cache refresh" >&2
fi

# copy translations
for f in locales/*; do
    mkdir -p $INSTALL_ROOT/share/en_CA/LC_MESSAGES
    cp locales/en_CA/*.po $INSTALL_ROOT/share/en_CA/LC_MESSAGES
done

# check if installed file is in user's path
if ! command -v people-db-gtk >/dev/null 2>&1; then
    echo "Warning: people-db-gtk not found in your PATH after install!" >&2
else
    echo "Found: $(command -v people-db-gtk)"
fi

