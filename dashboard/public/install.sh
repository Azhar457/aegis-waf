#!/bin/bash
set -e

echo "================================================="
echo " 🛡️ Aegis WAF Agent Installation (Linux / macOS)"
echo "================================================="

if [ -z "$CONTROLLER_IP" ]; then
  echo "Error: CONTROLLER_IP environment variable not set."
  echo "Usage: curl -sSL http://<IP>:8080/install.sh | CONTROLLER_IP=<IP>:8080 bash"
  exit 1
fi

echo "[*] Connecting to Aegis Central Controller at: $CONTROLLER_IP"
echo "[*] Detecting OS..."

OS="$(uname -s)"
ARCH="$(uname -m)"

echo "[*] Detected: $OS ($ARCH)"
echo "[*] Checking required dependencies..."
DEPS_MISSING=0

check_cmd() {
    if ! command -v "$1" >/dev/null 2>&1; then
        echo " ❌ Missing command: $1"
        DEPS_MISSING=1
    else
        echo " ✅ Found: $1"
    fi
}

check_cmd "curl"
check_cmd "sudo"

if [ "$OS" = "Linux" ]; then
    check_cmd "systemctl"
    
    # Memeriksa ketersediaan libssl untuk kebutuhan WAF
    if ! command -v openssl >/dev/null 2>&1 && ! ldconfig -p 2>/dev/null | grep -q "libssl"; then
        echo " ❌ Missing library: libssl (OpenSSL)"
        DEPS_MISSING=1
    else
        echo " ✅ Found: libssl (OpenSSL)"
    fi
fi

if [ $DEPS_MISSING -eq 1 ]; then
    echo ""
    echo "⚠️  Error: Beberapa dependensi sistem belum terinstall."
    echo "Silakan install terlebih dahulu. Contoh untuk Ubuntu/Debian:"
    echo "   sudo apt update && sudo apt install curl sudo systemd openssl -y"
    exit 1
fi
INSTALL_DIR="/opt/aegis-waf"
echo "[*] Creating installation directory at $INSTALL_DIR..."
sudo mkdir -p "$INSTALL_DIR"

echo "[*] Downloading Aegis WAF Agent binary dari Controller..."
sudo curl -sSL "http://$CONTROLLER_IP/bin/aegis-agent-$OS-$ARCH" -o "$INSTALL_DIR/aegis-agent"
sudo chmod +x "$INSTALL_DIR/aegis-agent"

echo "[*] Generating Agent Configuration (config.toml)..."
sudo bash -c "cat <<EOF > $INSTALL_DIR/config.toml
mode = \"agent\"
controller_url = \"http://$CONTROLLER_IP\"
port = 80
EOF"

if [ "$OS" = "Linux" ] && command -v systemctl >/dev/null 2>&1; then
    echo "[*] Setting up systemd background service..."
    sudo bash -c "cat <<EOF > /etc/systemd/system/aegis-agent.service
[Unit]
Description=Aegis WAF Agent
After=network.target

[Service]
Type=simple
ExecStart=$INSTALL_DIR/aegis-agent --config $INSTALL_DIR/config.toml
Restart=on-failure
User=root

[Install]
WantedBy=multi-user.target
EOF"
    sudo systemctl daemon-reload
    sudo systemctl enable aegis-agent
    echo "[*] Service registered. Run 'sudo systemctl start aegis-agent' to begin proxying traffic."
else
    echo "[*] To start the agent manually, run:"
    echo "    sudo $INSTALL_DIR/aegis-agent --config $INSTALL_DIR/config.toml"
fi

echo "================================================="
echo " ✅ Aegis Agent installation completed!"
echo "================================================="
