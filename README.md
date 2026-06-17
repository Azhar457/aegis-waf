# 🛡️ Aegis WAF — Next Gen Layer 7 Web Application Firewall

Aegis WAF is a pure, ultra-fast **Reverse Proxy + Rule Engine + Virtual Host Router** built in Rust for high-throughput network environments (like homelabs and enterprise portals). It features a real-time, terminal-like dashboard built in Svelte to monitor attacks, manage virtual hosts, toggle rules, and configure log limits dynamically.

---

## ✨ Features

- **Reverse Proxy Engine**: Built on Axum and Hyper, supporting WebSockets, HTTP/1.1, and proxying headers (`X-Real-IP`, `X-Forwarded-For`).
- **Dynamic Virtual Host Routing**: Route wildcard domains (e.g. `*.mydomain.id`) to different upstream ports without reloading services.
- **WAF Rule Engine (35+ Rules)**: Checks requests across 4 phases (Headers, URI, Query, Request Body) for SQL Injection, Cross-Site Scripting (XSS), Local File Inclusion (LFI), SSRF, Command Injection, and Scanners.
- **Stateless/Persistent Rate Limiting**: Token-bucket algorithm applied globally and per path, persisting across client requests.
- **SafeLine-Style Log Terminal**: High-frequency real-time SSE stream log view resembling a monospace terminal, optimized for millions of requests.
- **Log Pruning & Exporting**: Custom capacity settings (500MB, 1024MB, Custom) with automated database pruning to protect storage from overflowing during NMAP scans.

---

## 🏗️ Architecture

- **Controller Mode**: Runs the Svelte web panel, aggregates logs into SQLite, handles stats, config exports, and serves endpoints on port `8080`.
- **Agent Mode**: Runs the actual packet parser, WAF rule inspector, rate limiter, and reverse proxies incoming traffic on port `80` (HTTP). It feeds logs back to the Controller and dynamically hot-reloads configurations.

---

## 💻 Platform Support & Feature Matrix

The following matrix shows the feature availability and differences across target Operating Systems:

| Feature | Linux | Windows | macOS |
| :--- | :---: | :---: | :---: |
| **Layer 7 HTTP Proxy & WAF** | ✅ Supported | ✅ Supported | ✅ Supported |
| **Dynamic Virtual Host Routing** | ✅ Supported | ✅ Supported | ✅ Supported |
| **Stateless Rate Limiting Tiers** | ✅ Supported | ✅ Supported | ✅ Supported |
| **Geoblocking (Allow/Blocklist)** | ✅ Supported | ✅ Supported | ✅ Supported |
| **Collaborative IP Threat Intel** | ✅ Supported | ✅ Supported | ✅ Supported |
| **Privilege Dropping (Root -> Nobody)** | ✅ Yes (`nix` UID/GID drop) | ❌ N/A (Runs as Admin) | ❌ N/A (Runs as Root/Sudo) |
| **eBPF XDP Packet Offloading** | ✅ Supported (Kernel space) | ❌ Not Supported | ❌ Not Supported |
| **eBPF Uprobe SSL Sniffing** | ✅ Supported (Zero-copy L7) | ❌ Not Supported | ❌ Not Supported |
| **Native System Daemon Integration** | ✅ Yes (`systemd` Service) | ❌ No (Task Scheduler/NSSM) | ❌ No (`launchd` Plist) |

---

## 🚀 Installation & Setup Guide

### Prerequisites
1. **Rust Toolchain**: Install [Rust (cargo)](https://www.rust-lang.org/tools/install) (edition 2021).
2. **Node.js**: Install [Node.js (npm)](https://nodejs.org/) (for dashboard compilation).

### Quick Build
Compile both the frontend dashboard and the Rust WAF executable from the root directory:
```bash
# Install dashboard frontend dependencies
npm run dashboard:install

# Compile the entire project (Dashboard + Rust executable)
npm run build-all
```

---

## 💻 Operating System Guides

### 1. Windows (PowerShell / CMD)
Windows is supported for local development and homelab setups.

*   **Binding Privileged Ports**: Running the WAF Agent on port `80` requires Administrator privileges on Windows. Open PowerShell/CMD as **Administrator**.
*   **Compile & Build**:
    ```cmd
    npm run build-all
    ```
*   **Run Controller (Svelte + Logger)**:
    ```cmd
    npm run waf:controller
    ```
*   **Run WAF Agent**:
    ```cmd
    npm run waf:agent
    ```
    *Note: If you get a file locking error compiling on Windows, kill any running `aegis-waf` processes first.*

---

### 2. Linux (Debian, Ubuntu, CentOS)
Linux is the recommended production environment.

*   **Compilation**:
    ```bash
    npm run build-all
    ```
*   **Privilege Dropping (Security)**:
    Linux prevents non-root users from binding to ports `<1024` (like port `80`). Aegis WAF binds to port `80` as root and immediately drops privileges to the `nobody` group/user (`uid=65534, gid=65534`) for security.
*   **Running using Systemd**:
    To keep the processes running in the background, create systemd service files:
    
    `/etc/systemd/system/aegis-controller.service`:
    ```ini
    [Unit]
    Description=Aegis WAF Controller
    After=network.target

    [Service]
    WorkingDirectory=/home/aegis/aegis-waf
    ExecStart=/home/aegis/aegis-waf/target/release/aegis-waf controller --port 8080
    Restart=always
    User=aegis

    [Install]
    WantedBy=multi-user.target
    ```

    `/etc/systemd/system/aegis-agent.service`:
    ```ini
    [Unit]
    Description=Aegis WAF Agent
    After=network.target

    [Service]
    WorkingDirectory=/home/aegis/aegis-waf
    ExecStart=/home/aegis/aegis-waf/target/release/aegis-waf agent --controller http://localhost:8080
    Restart=always
    # Binds to port 80 as root, drops privilege itself
    User=root
    Group=root

    [Install]
    WantedBy=multi-user.target
    ```
    Enable and start services:
    ```bash
    sudo systemctl daemon-reload
    sudo systemctl enable --now aegis-controller aegis-agent
    ```

---

### 3. macOS
macOS is fully supported for testing and local proxy setups.

*   **Mac compilation**:
    ```bash
    npm run build-all
    ```
*   **Running with Sudo**:
    Similar to Linux, binding to port `80` requires root. Run the agent with `sudo`:
    ```bash
    # Run Controller
    cargo run -- controller
    
    # Run Agent (in another terminal tab)
    sudo cargo run -- agent --controller http://localhost:8080
    ```

---

## ⚙️ Configuration (`config.toml`)

The configuration file is automatically watched and dynamically hot-reloaded by the agent:

```toml
[global]
port_http = 80
port_https = 443
max_body_size = 10485760      # 10 MB for body inspection
default_rate_limit = 600      # 600 req/min default per IP
log_dir = "./logs"

[tls]
mode = "local_ca"
cert_dir = "./certs"

[[vhosts]]
name = "aegis_demo"
hosts = ["*.aegiswaf.demo"]
backend = "127.0.0.1:8080"
rules = ["SQLI-*", "XSS-*", "LFI-*", "RFI-*"]
ssl = "Auto (Local CA)"
max_body = "10MB"
rate_limit = "600 req/min"
logging = { enabled = true, db_path = "logs/aegis-waf.db" }
```

---

## 🔧 Troubleshooting

1.  **Vite / Build failures**: Ensure you ran `npm run dashboard:install` from the root directory to populate node modules.
2.  **Port 80/8080 already in use**:
    *   On Windows, check IIS or Docker container bindings: `netstat -ano | findstr 80`
    *   On Linux/Mac: `sudo lsof -i :80` or `sudo lsof -i :8080`
3.  **Windows Compilation Lock**: If `cargo build` fails with `Access Denied`, make sure you have closed any active terminal instances running `npm run waf:controller` or `npm run waf:agent` (`Ctrl + C` to close).
