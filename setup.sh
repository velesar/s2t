#!/bin/bash
# Setup script for Voice Dictation on Fedora
# Скрипт встановлення для Fedora

set -e

echo "🎤 Встановлення Voice Dictation"
echo "================================"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Check if running on Fedora
if ! grep -q "Fedora" /etc/os-release 2>/dev/null; then
    echo -e "${YELLOW}⚠️  Цей скрипт оптимізовано для Fedora. На інших дистрибутивах можуть бути відмінності.${NC}"
fi

echo ""
echo "📦 Встановлення системних залежностей..."
sudo dnf install -y \
    gcc gcc-c++ cmake pkg-config \
    gtk4-devel \
    alsa-lib-devel \
    dbus-devel \
    clang llvm-devel

# Check for Rust
if ! command -v cargo &> /dev/null; then
    echo ""
    echo "🦀 Rust не знайдено. Встановлюю..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
else
    echo -e "${GREEN}✓ Rust вже встановлено${NC}"
fi

# Download Whisper model
WHISPER_DIR="$HOME/.local/share/whisper"
MODEL_FILE="$WHISPER_DIR/ggml-base.bin"

if [ ! -f "$MODEL_FILE" ]; then
    echo ""
    echo "🧠 Завантаження моделі Whisper..."
    mkdir -p "$WHISPER_DIR"

    echo "Яку модель завантажити?"
    echo "  1) tiny   (~75MB)  - швидка, базова якість"
    echo "  2) base   (~150MB) - швидка, прийнятна якість [рекомендовано]"
    echo "  3) small  (~500MB) - середня, хороша якість"
    echo "  4) medium (~1.5GB) - повільна, відмінна якість"
    read -p "Виберіть (1-4) [2]: " choice
    choice=${choice:-2}

    case $choice in
        1) MODEL="tiny" ;;
        2) MODEL="base" ;;
        3) MODEL="small" ;;
        4) MODEL="medium" ;;
        *) MODEL="base" ;;
    esac

    MODEL_URL="https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-${MODEL}.bin"
    MODEL_FILE="$WHISPER_DIR/ggml-${MODEL}.bin"

    echo "Завантаження $MODEL_URL ..."
    curl -L -o "$MODEL_FILE" "$MODEL_URL"
    echo -e "${GREEN}✓ Модель завантажено: $MODEL_FILE${NC}"
else
    echo -e "${GREEN}✓ Модель Whisper вже є${NC}"
fi

# Build the project
echo ""
echo "🔨 Збірка проєкту..."
cargo build --release

# Install
echo ""
echo "📁 Встановлення..."
mkdir -p "$HOME/.local/bin"
cp target/release/voice-dictation "$HOME/.local/bin/"

# Create desktop entry
mkdir -p "$HOME/.local/share/applications"
cat > "$HOME/.local/share/applications/voice-dictation.desktop" << EOF
[Desktop Entry]
Type=Application
Name=Voice Dictation
Name[uk]=Голосова диктовка
Comment=Offline speech-to-text using Whisper
Comment[uk]=Офлайн розпізнавання мовлення через Whisper
Exec=voice-dictation
Icon=audio-input-microphone
Terminal=false
Categories=AudioVideo;Audio;Utility;
Keywords=voice;dictation;speech;whisper;transcription;
StartupNotify=false
X-GNOME-Autostart-enabled=true
EOF

# Also create autostart entry
mkdir -p "$HOME/.config/autostart"
cp "$HOME/.local/share/applications/voice-dictation.desktop" "$HOME/.config/autostart/"

# Check for GNOME tray extension
if [ "$XDG_CURRENT_DESKTOP" = "GNOME" ]; then
    echo ""
    echo -e "${YELLOW}⚠️  Ви використовуєте GNOME.${NC}"
    echo "Для відображення іконки в треї потрібне розширення AppIndicator."

    if ! gnome-extensions list 2>/dev/null | grep -q "appindicator"; then
        read -p "Встановити gnome-shell-extension-appindicator? (y/n) [y]: " install_ext
        install_ext=${install_ext:-y}
        if [ "$install_ext" = "y" ]; then
            sudo dnf install -y gnome-shell-extension-appindicator
            echo -e "${YELLOW}Перезапустіть GNOME (Alt+F2 → r → Enter) для активації${NC}"
        fi
    else
        echo -e "${GREEN}✓ AppIndicator розширення вже встановлено${NC}"
    fi
fi

echo ""
echo -e "${GREEN}✅ Встановлення завершено!${NC}"
echo ""
echo "Запуск:"
echo "  voice-dictation"
echo ""
echo "Або знайдіть 'Voice Dictation' в меню програм."
