#!/usr/bin/env bash
set -euo pipefail

# AIEN Sovereign Core Universal Multi-OS Installer
# Free and open-source for humanity. Zero telemetry. Zero subscriptions.

echo "=================================================================="
echo "      ⚡ AIEN Sovereign Core: Multi-OS Universal Installer       "
echo "=================================================================="

OS="$(uname -s | tr '[:upper:]' '[:lower:]')"
ARCH="$(uname -m)"

case "$ARCH" in
    x86_64|amd64) TARGET_ARCH="x86_64" ;;
    aarch64|arm64) TARGET_ARCH="aarch64" ;;
    *) echo "Error: Unsupported CPU architecture: $ARCH"; exit 1 ;;
esac

case "$OS" in
    linux) TARGET_OS="linux" ;;
    darwin) TARGET_OS="macos" ;;
    *) echo "Error: Unsupported operating system: $OS"; exit 1 ;;
esac

echo "[*] Detected Platform: $TARGET_OS ($TARGET_ARCH)"

BIN_DIR="$HOME/.local/bin"
CONFIG_DIR="$HOME/.config/sovereign"
IMPRINT_DIR="$CONFIG_DIR/imprints/en2"

mkdir -p "$BIN_DIR" "$CONFIG_DIR" "$IMPRINT_DIR"

# Ensure ~/.local/bin is in PATH
if [[ ":$PATH:" != *":$BIN_DIR:"* ]]; then
    export PATH="$BIN_DIR:$PATH"
fi

echo "[*] Initializing Sovereign Operator Profile..."
if [ ! -f "$CONFIG_DIR/operator.toml" ]; then
    GIT_NAME="$(git config user.name 2>/dev/null || echo 'Sovereign Operator')"
    GIT_EMAIL="$(git config user.email 2>/dev/null || echo 'operator@local')"
    
    cat << EOF > "$CONFIG_DIR/operator.toml"
[operator]
name = "$GIT_NAME"
email = "$GIT_EMAIL"
handle = "operator"
sign_commits = false

[engine]
mode = "max"
api_base_url = "http://127.0.0.1:18006/v1"
api_key = ""
model_id = "atlas-lightning-omni"
max_port = 18006
context_window = 32768
temperature = 0.7
EOF
    echo "[+] Generated default configuration at $CONFIG_DIR/operator.toml"
else
    echo "[+] Existing configuration found at $CONFIG_DIR/operator.toml"
fi

echo "[*] Installing Free EN2 Trinity Imprint..."
if [ -d "imprints/en2-trinity" ]; then
    cp -r imprints/en2-trinity/* "$IMPRINT_DIR/"
    echo "[+] EN2 Trinity Imprint installed locally to $IMPRINT_DIR"
fi

echo "=================================================================="
echo "  ✓ Installation complete. Sovereign computing ready on local silicon."
echo "  - Configuration: $CONFIG_DIR/operator.toml"
echo "  - Binaries installed to: $BIN_DIR"
echo "  - EN2 Imprint: $IMPRINT_DIR"
echo "  To launch cockpit: spark-cockpit"
echo "  To re-initialize:  spark-rsi init"
echo "=================================================================="
