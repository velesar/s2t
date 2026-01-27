#!/bin/bash
set -e

echo "Building release version..."
cargo build --release

BINARY="target/release/voice-dictation"
INSTALL_DIR="$HOME/.local/bin"
DESKTOP_FILE="voice-dictation.desktop"
APPS_DIR="$HOME/.local/share/applications"
AUTOSTART_DIR="$HOME/.config/autostart"

echo "Installing binary to $INSTALL_DIR..."
mkdir -p "$INSTALL_DIR"
cp "$BINARY" "$INSTALL_DIR/"

echo "Installing desktop file to $APPS_DIR..."
mkdir -p "$APPS_DIR"
cp "$DESKTOP_FILE" "$APPS_DIR/"

echo "Enabling autostart..."
mkdir -p "$AUTOSTART_DIR"
cp "$DESKTOP_FILE" "$AUTOSTART_DIR/"

echo ""
echo "Installation complete!"
echo ""
echo "Make sure $INSTALL_DIR is in your PATH."
echo "The app will start automatically on next login."
echo ""
echo "To start now, run: voice-dictation"
