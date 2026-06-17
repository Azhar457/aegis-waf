
# 🔧 ANALISIS PERFORMA: SSE vs WebSocket untuk Real-time Terminal

## Masalah yang Anda Hadapi

| Masalah | Penyebab | Solusi |
|---------|----------|--------|
| **Kurang real-time** | SSE (Server-Sent Events) = one-way, HTTP overhead | WebSocket = bidirectional, persistent connection |
| **Latency tinggi** | HTTP polling / SSE reconnect | WebSocket tetap terbuka, push instan |
| **xterm.js lag** | Render setiap batch, bukan per-karakter | Streaming buffer + debounce |
| **SQLite bottleneck** | Write lock, single-threaded | PostgreSQL + connection pool |
| **Dashboard freeze** | Semua data di-load sekaligus | Pagination + lazy loading |

---

## 🏗️ Arsitektur Target: Real-time Optimal

```
┌─────────────────────────────────────────────────────────────┐
│           ARSITEKTUR REAL-TIME v0.2 (Optimal)              │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  BROWSER (Svelte + xterm.js)                              │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  WebSocket Client (reconnecting-websocket)        │   │
│  │  • Binary stream untuk terminal (minimal overhead) │   │
│  │  • JSON untuk command/control                      │   │
│  │  • Auto-reconnect dengan exponential backoff       │   │
│  │                                                     │   │
│  │  xterm.js Optimasi:                                 │   │
│  │  • write() dengan buffer 16KB (bukan per char)     │   │
│  │  • requestAnimationFrame untuk render batch        │   │
│  │  • Addon: web-links, webgl (GPU accelerated)         │   │
│  │  • Debounce: 16ms (1 frame) untuk update UI        │   │
│  └─────────────────────────────────────────────────────┘   │
│                              │                              │
│                              │ WebSocket (ws:// atau wss://)│
│                              │ Binary + JSON multiplexing   │
│                              ▼                              │
│  CONTROLLER (Rust/Go) — Port 8080                          │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  WebSocket Server (tokio-tungstenite / fastwebsocket)│   │
│  │  • Accept connection dari browser                    │   │
│  │  • Broadcast log ke semua subscriber               │   │
│  │  • Channel per-topik: firewall, waf, system        │   │
│  │                                                     │   │
│  │  PostgreSQL (Docker) — Replace SQLite               │   │
│  │  • Connection pool: 10-20 connections              │   │
│  │  • Async: tokio-postgres / sqlx                     │   │
│  │  • TimescaleDB extension: time-series optimization  │   │
│  │  • Partitioning: per-day untuk log table            │   │
│  │  • Retention: auto-drop setelah 30 hari           │   │
│  │                                                     │   │
│  │  Redis (Docker) — Cache & Pub/Sub                   │   │
│  │  • Cache: recent logs (LRU, 1000 entries)          │   │
│  │  • Pub/Sub: real-time broadcast ke WebSocket       │   │
│  │  • Rate limit: sliding window counter              │   │
│  └─────────────────────────────────────────────────────┘   │
│                              │                              │
│                              │ HTTP API / gRPC              │
│                              ▼                              │
│  AGENT (Rust) — Port 80                                    │
│  ┌─────────────────────────────────────────────────────┐   │
│  │  Log Shipper (tokio::sync::mpsc)                    │   │
│  │  • Buffer: 1000 messages                             │   │
│  │  • Batch: kirim setiap 100ms atau 100 messages      │   │
│  │  • Compression: zstd untuk log besar                │   │
│  │  • Retry: exponential backoff ke Controller         │   │
│  └─────────────────────────────────────────────────────┘   │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

---

## 📊 Perbandingan: SSE vs WebSocket vs gRPC-Web

| Aspek | SSE (Current) | WebSocket | gRPC-Web |
|-------|---------------|-----------|----------|
| **Direction** | Server → Client | Bidirectional | Bidirectional |
| **Protocol** | HTTP/1.1 | HTTP/1.1 (ws) atau HTTP/2 | HTTP/2 |
| **Overhead** | ~200 bytes/header per message | ~2 bytes/frame | ~5 bytes/frame |
| **Latency** | ~100-500ms (reconnect) | ~1-10ms | ~1-10ms |
| **Throughput** | ~1000 msg/sec | ~100K msg/sec | ~50K msg/sec |
| **Binary** | ❌ Base64 only | ✅ Native | ✅ Native |
| **Reconnect** | Auto (EventSource) | Manual (library) | Auto (grpc) |
| **Browser** | ✅ Native | ✅ 99% support | ⚠️ Proxy needed |
| **Complexity** | Low | Medium | High |
| **Best for** | Notifications, simple push | Real-time games, terminal | Microservices |

**Verdict untuk Aegis:** **WebSocket** — karena butuh bidirectional (command dari dashboard ke agent) dan binary stream untuk terminal.

---

## 🎯 Solusi Spesifik untuk Masalah Anda

### 1. xterm.js Real-time Streaming

```typescript
// dashboard/src/lib/terminal.ts (Optimasi)

import { Terminal } from 'xterm';
import { WebglAddon } from 'xterm-addon-webgl';
import { FitAddon } from 'xterm-addon-fit';

export class AegisTerminal {
  private terminal: Terminal;
  private buffer: string[] = [];
  private flushTimer: number | null = null;
  private readonly FLUSH_INTERVAL = 16; // 1 frame (60fps)
  private readonly BUFFER_SIZE = 16384; // 16KB

  constructor(container: HTMLElement) {
    this.terminal = new Terminal({
      cols: 120,
      rows: 40,
      fontSize: 14,
      fontFamily: 'JetBrains Mono, monospace',
      cursorBlink: true,
      cursorStyle: 'block',
      theme: {
        background: '#1a1a2e',
        foreground: '#eaeaea',
        cursor: '#f39c12',
        selectionBackground: '#f39c12',
        black: '#1a1a2e',
        red: '#e74c3c',
        green: '#2ecc71',
        yellow: '#f1c40f',
        blue: '#3498db',
        magenta: '#9b59b6',
        cyan: '#1abc9c',
        white: '#ecf0f1',
      },
      scrollback: 100000, // 100K lines
      convertEol: true,
    });

    // GPU Acceleration (WebGL)
    const webglAddon = new WebglAddon();
    this.terminal.loadAddon(webglAddon);

    const fitAddon = new FitAddon();
    this.terminal.loadAddon(fitAddon);

    this.terminal.open(container);
    fitAddon.fit();

    // Handle resize
    window.addEventListener('resize', () => fitAddon.fit());
  }

  // Optimasi: Buffer + Debounce
  write(data: string | Uint8Array) {
    const str = typeof data === 'string' ? data : new TextDecoder().decode(data);
    this.buffer.push(str);

    // Flush jika buffer penuu atau timer habis
    const totalLength = this.buffer.reduce((sum, s) => sum + s.length, 0);
    if (totalLength >= this.BUFFER_SIZE) {
      this.flush();
    } else if (!this.flushTimer) {
      this.flushTimer = window.setTimeout(() => this.flush(), this.FLUSH_INTERVAL);
    }
  }

  private flush() {
    if (this.buffer.length === 0) return;

    const combined = this.buffer.join('');
    this.buffer = [];

    if (this.flushTimer) {
      clearTimeout(this.flushTimer);
      this.flushTimer = null;
    }

    // Write ke terminal dalam satu batch
    this.terminal.write(combined, () => {
      // Callback setelah write selesai
    });
  }

  // Untuk binary data (eBPF logs)
  writeBinary(data: Uint8Array) {
    const decoder = new TextDecoder('utf-8', { fatal: false });
    const str = decoder.decode(data);
    this.write(str);
  }

  clear() {
    this.terminal.clear();
  }

  dispose() {
    this.terminal.dispose();
  }
}
```

### 2. WebSocket Server (Rust — Controller)

```rust
// controller/src/websocket.rs

use tokio::sync::{broadcast, mpsc};
use tokio_tungstenite::{accept_async, tungstenite::Message};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::sync::Arc;
use dashmap::DashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
enum WsMessage {
    #[serde(rename = "log")]
    Log { 
        source: String,      // "firewall", "waf", "system"
        level: String,       // "info", "warn", "error", "block"
        message: String,
        timestamp: String,
        metadata: Option<serde_json::Value>,
    },
    #[serde(rename = "command")]
    Command {
        action: String,      // "reload", "block_ip", "unblock_ip"
        params: serde_json::Value,
    },
    #[serde(rename = "stats")]
    Stats {
        requests_per_second: f64,
        blocked_per_second: f64,
        active_connections: u32,
    },
    #[serde(rename = "ping")]
    Ping,
    #[serde(rename = "pong")]
    Pong,
}

pub struct WebSocketManager {
    // Broadcast channel untuk log (1 sender, N receivers)
    log_tx: broadcast::Sender<WsMessage>,
    // Active connections
    connections: Arc<DashMap<SocketAddr, mpsc::UnboundedSender<WsMessage>>>,
}

impl WebSocketManager {
    pub fn new() -> Self {
        let (log_tx, _) = broadcast::channel(10000); // Buffer 10K messages
        Self {
            log_tx,
            connections: Arc::new(DashMap::new()),
        }
    }

    pub async fn handle_connection(
        &self,
        stream: tokio::net::TcpStream,
        addr: SocketAddr,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let ws_stream = accept_async(stream).await?;
        let (mut ws_tx, mut ws_rx) = ws_stream.split();

        // Subscribe ke broadcast channel
        let mut log_rx = self.log_tx.subscribe();

        // Channel untuk kirim message ke WebSocket
        let (tx, mut rx) = mpsc::unbounded_channel::<WsMessage>();
        self.connections.insert(addr, tx.clone());

        // Task 1: Kirim message dari channel ke WebSocket
        let connections = self.connections.clone();
        let send_task = tokio::spawn(async move {
            while let Some(msg) = rx.recv().await {
                let json = serde_json::to_string(&msg).unwrap_or_default();
                if ws_tx.send(Message::Text(json)).await.is_err() {
                    break;
                }
            }
            connections.remove(&addr);
        });

        // Task 2: Terima message dari WebSocket (commands dari browser)
        let log_tx = self.log_tx.clone();
        let receive_task = tokio::spawn(async move {
            while let Some(Ok(msg)) = ws_rx.next().await {
                match msg {
                    Message::Text(text) => {
                        if let Ok(cmd) = serde_json::from_str::<WsMessage>(&text) {
                            match cmd {
                                WsMessage::Command { action, params } => {
                                    // Handle command dari dashboard
                                    handle_command(action, params, &log_tx).await;
                                }
                                WsMessage::Ping => {
                                    let _ = tx.send(WsMessage::Pong);
                                }
                                _ => {}
                            }
                        }
                    }
                    Message::Close(_) => break,
                    _ => {}
                }
            }
        });

        // Task 3: Forward broadcast log ke connection ini
        let tx_clone = tx.clone();
        let broadcast_task = tokio::spawn(async move {
            while let Ok(msg) = log_rx.recv().await {
                if tx_clone.send(msg).is_err() {
                    break;
                }
            }
        });

        // Wait for any task to finish
        tokio::select! {
            _ = send_task => {},
            _ = receive_task => {},
            _ = broadcast_task => {},
        }

        self.connections.remove(&addr);
        Ok(())
    }

    pub fn broadcast_log(&self, msg: WsMessage) {
        let _ = self.log_tx.send(msg);
    }
}

async fn handle_command(
    action: String,
    params: serde_json::Value,
    log_tx: &broadcast::Sender<WsMessage>,
) {
    match action.as_str() {
        "reload" => {
            // Hot reload config
            log_tx.send(WsMessage::Log {
                source: "system".to_string(),
                level: "info".to_string(),
                message: "Config reload triggered from dashboard".to_string(),
                timestamp: chrono::Utc::now().to_rfc3339(),
                metadata: None,
            }).ok();
        }
        "block_ip" => {
            if let Some(ip) = params.get("ip").and_then(|v| v.as_str()) {
                // Add to blocklist
                log_tx.send(WsMessage::Log {
                    source: "firewall".to_string(),
                    level: "warn".to_string(),
                    message: format!("IP {} manually blocked from dashboard", ip),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    metadata: Some(params),
                }).ok();
            }
        }
        _ => {}
    }
}
```

### 3. PostgreSQL + TimescaleDB (Docker)

```yaml
# docker-compose.yml (Database Stack)

version: '3.8'

services:
  postgres:
    image: timescale/timescaledb:latest-pg16
    container_name: aegis-postgres
    restart: unless-stopped
    environment:
      POSTGRES_USER: aegis
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD:-aegis_secure_password}
      POSTGRES_DB: aegis
    volumes:
      - postgres_data:/var/lib/postgresql/data
      - ./init.sql:/docker-entrypoint-initdb.d/init.sql
    ports:
      - "5432:5432"
    command: >
      postgres
      -c shared_preload_libraries=timescaledb
      -c max_connections=200
      -c shared_buffers=256MB
      -c effective_cache_size=768MB
      -c maintenance_work_mem=64MB
      -c wal_buffers=16MB
      -c default_statistics_target=100
      -c random_page_cost=1.1
      -c effective_io_concurrency=200
      -c work_mem=1310kB
      -c min_wal_size=1GB
      -c max_wal_size=4GB
      -c max_worker_processes=4
      -c max_parallel_workers_per_gather=2
      -c max_parallel_workers=4
      -c max_parallel_maintenance_workers=2

  redis:
    image: redis:7-alpine
    container_name: aegis-redis
    restart: unless-stopped
    command: redis-server --maxmemory 256mb --maxmemory-policy allkeys-lru
    volumes:
      - redis_data:/data
    ports:
      - "6379:6379"

  # Optional: PGAdmin untuk development
  pgadmin:
    image: dpage/pgadmin4:latest
    container_name: aegis-pgadmin
    restart: unless-stopped
    environment:
      PGADMIN_DEFAULT_EMAIL: admin@aegis.local
      PGADMIN_DEFAULT_PASSWORD: ${PGADMIN_PASSWORD:-admin}
    ports:
      - "5050:80"
    depends_on:
      - postgres

volumes:
  postgres_data:
  redis_data:
```

```sql
-- init.sql (Database Schema dengan TimescaleDB)

-- Enable TimescaleDB extension
CREATE EXTENSION IF NOT EXISTS timescaledb;

-- Create tables
CREATE TABLE IF NOT EXISTS requests (
    time TIMESTAMPTZ NOT NULL,
    agent_id TEXT NOT NULL,
    client_ip INET NOT NULL,
    method TEXT NOT NULL,
    path TEXT NOT NULL,
    host TEXT,
    user_agent TEXT,
    action TEXT NOT NULL,  -- 'PASS', 'BLOCK', 'RATE_LIMIT', 'ERROR'
    rule_id TEXT,
    rule_name TEXT,
    severity TEXT,
    reason TEXT,
    response_status INTEGER,
    response_time_ms INTEGER,
    body_size INTEGER,
    country_code TEXT(2),
    tags TEXT[]
);

-- Convert to hypertable (TimescaleDB time-series optimization)
SELECT create_hypertable('requests', 'time', if_not_exists => TRUE, chunk_time_interval => INTERVAL '1 day');

-- Indexes
CREATE INDEX idx_requests_agent_time ON requests(agent_id, time DESC);
CREATE INDEX idx_requests_client_ip ON requests(client_ip, time DESC);
CREATE INDEX idx_requests_action ON requests(action, time DESC);
CREATE INDEX idx_requests_rule_id ON requests(rule_id, time DESC);
CREATE INDEX idx_requests_host ON requests(host, time DESC);

-- Compression policy (save 90%+ space)
ALTER TABLE requests SET (
    timescaledb.compress,
    timescaledb.compress_segmentby = 'agent_id, action'
);

SELECT add_compression_policy('requests', INTERVAL '7 days');

-- Retention policy (auto-drop after 30 days)
SELECT add_retention_policy('requests', INTERVAL '30 days');

-- Continuous aggregates (pre-computed stats)
CREATE MATERIALIZED VIEW requests_1min
WITH (timescaledb.continuous) AS
SELECT
    time_bucket('1 minute', time) AS bucket,
    agent_id,
    host,
    COUNT(*) as total_requests,
    SUM(CASE WHEN action = 'BLOCK' THEN 1 ELSE 0 END) as blocked,
    SUM(CASE WHEN action = 'RATE_LIMIT' THEN 1 ELSE 0 END) as rate_limited,
    AVG(response_time_ms) as avg_response_time,
    MAX(response_time_ms) as max_response_time
FROM requests
GROUP BY bucket, agent_id, host;

-- Stats table (latest snapshot)
CREATE TABLE IF NOT EXISTS stats (
    agent_id TEXT PRIMARY KEY,
    last_updated TIMESTAMPTZ DEFAULT NOW(),
    total_requests BIGINT DEFAULT 0,
    total_blocked BIGINT DEFAULT 0,
    total_rate_limited BIGINT DEFAULT 0,
    requests_per_second DOUBLE PRECISION DEFAULT 0,
    active_connections INTEGER DEFAULT 0,
    top_blocked_ips JSONB DEFAULT '[]',
    top_rules_triggered JSONB DEFAULT '[]'
);

-- Blocked IPs table (with expiration)
CREATE TABLE IF NOT EXISTS blocked_ips (
    id SERIAL PRIMARY KEY,
    ip INET NOT NULL,
    first_seen TIMESTAMPTZ DEFAULT NOW(),
    last_seen TIMESTAMPTZ DEFAULT NOW(),
    expires_at TIMESTAMPTZ,
    reason TEXT,
    source TEXT,  -- 'WAF', 'FIREWALL', 'MANUAL'
    agent_id TEXT,
    UNIQUE(ip, agent_id)
);

CREATE INDEX idx_blocked_ips_expires ON blocked_ips(expires_at);
CREATE INDEX idx_blocked_ips_ip ON blocked_ips(ip);
```

### 4. Rust PostgreSQL Client (Async)

```rust
// controller/src/db.rs (PostgreSQL dengan sqlx)

use sqlx::{postgres::PgPoolOptions, PgPool, Row};
use chrono::{DateTime, Utc};
use std::time::Duration;

pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn connect(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(20)
            .min_connections(5)
            .acquire_timeout(Duration::from_secs(3))
            .idle_timeout(Duration::from_secs(600))
            .connect(database_url)
            .await?;

        Ok(Self { pool })
    }

    pub async fn insert_request(&self, entry: LogEntry) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO requests (
                time, agent_id, client_ip, method, path, host, user_agent,
                action, rule_id, rule_name, severity, reason,
                response_status, response_time_ms, body_size, country_code
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)"
        )
        .bind(entry.time)
        .bind(entry.agent_id)
        .bind(entry.client_ip)
        .bind(entry.method)
        .bind(entry.path)
        .bind(entry.host)
        .bind(entry.user_agent)
        .bind(entry.action)
        .bind(entry.rule_id)
        .bind(entry.rule_name)
        .bind(entry.severity)
        .bind(entry.reason)
        .bind(entry.response_status)
        .bind(entry.response_time_ms)
        .bind(entry.body_size)
        .bind(entry.country_code)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn get_recent_logs(
        &self,
        agent_id: &str,
        limit: i64,
    ) -> Result<Vec<LogEntry>, sqlx::Error> {
        let rows = sqlx::query(
            "SELECT * FROM requests 
             WHERE agent_id = $1 
             ORDER BY time DESC 
             LIMIT $2"
        )
        .bind(agent_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(|row| {
            Ok(LogEntry {
                time: row.try_get("time")?,
                agent_id: row.try_get("agent_id")?,
                client_ip: row.try_get("client_ip")?,
                method: row.try_get("method")?,
                path: row.try_get("path")?,
                host: row.try_get("host")?,
                user_agent: row.try_get("user_agent")?,
                action: row.try_get("action")?,
                rule_id: row.try_get("rule_id")?,
                rule_name: row.try_get("rule_name")?,
                severity: row.try_get("severity")?,
                reason: row.try_get("reason")?,
                response_status: row.try_get("response_status")?,
                response_time_ms: row.try_get("response_time_ms")?,
                body_size: row.try_get("body_size")?,
                country_code: row.try_get("country_code")?,
            })
        }).collect()
    }

    pub async fn get_stats_1min(
        &self,
        agent_id: &str,
    ) -> Result<Stats1Min, sqlx::Error> {
        let row = sqlx::query(
            "SELECT * FROM requests_1min 
             WHERE agent_id = $1 
             ORDER BY bucket DESC 
             LIMIT 1"
        )
        .bind(agent_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(Stats1Min {
            bucket: row.try_get("bucket")?,
            total_requests: row.try_get("total_requests")?,
            blocked: row.try_get("blocked")?,
            rate_limited: row.try_get("rate_limited")?,
            avg_response_time: row.try_get("avg_response_time")?,
        })
    }
}

#[derive(Debug)]
pub struct LogEntry {
    pub time: DateTime<Utc>,
    pub agent_id: String,
    pub client_ip: String,
    pub method: String,
    pub path: String,
    pub host: Option<String>,
    pub user_agent: Option<String>,
    pub action: String,
    pub rule_id: Option<String>,
    pub rule_name: Option<String>,
    pub severity: Option<String>,
    pub reason: Option<String>,
    pub response_status: Option<i32>,
    pub response_time_ms: Option<i32>,
    pub body_size: Option<i32>,
    pub country_code: Option<String>,
}

#[derive(Debug)]
pub struct Stats1Min {
    pub bucket: DateTime<Utc>,
    pub total_requests: i64,
    pub blocked: i64,
    pub rate_limited: i64,
    pub avg_response_time: Option<f64>,
}
```

### 5. Svelte WebSocket Client (Real-time Dashboard)

```typescript
// dashboard/src/lib/websocket.ts

import { writable, derived } from 'svelte/store';
import { browser } from '$app/environment';

export interface LogMessage {
    type: 'log';
    source: 'firewall' | 'waf' | 'system' | 'agent';
    level: 'info' | 'warn' | 'error' | 'block' | 'rate_limit';
    message: string;
    timestamp: string;
    metadata?: Record<string, unknown>;
}

export interface StatsMessage {
    type: 'stats';
    requests_per_second: number;
    blocked_per_second: number;
    active_connections: number;
}

export type WsMessage = LogMessage | StatsMessage | { type: 'ping' | 'pong' };

class WebSocketClient {
    private ws: WebSocket | null = null;
    private reconnectAttempts = 0;
    private maxReconnectAttempts = 10;
    private reconnectDelay = 1000; // Start with 1s
    private maxReconnectDelay = 30000; // Max 30s
    private pingInterval: ReturnType<typeof setInterval> | null = null;

    public connected = writable(false);
    public logs = writable<LogMessage[]>([]);
    public stats = writable<StatsMessage | null>(null);
    public connectionStatus = writable<'connecting' | 'connected' | 'disconnected'>('disconnected');

    constructor(private url: string) {
        if (browser) {
            this.connect();
        }
    }

    private connect() {
        this.connectionStatus.set('connecting');

        try {
            this.ws = new WebSocket(this.url);

            this.ws.onopen = () => {
                console.log('WebSocket connected');
                this.connected.set(true);
                this.connectionStatus.set('connected');
                this.reconnectAttempts = 0;
                this.reconnectDelay = 1000;

                // Start ping
                this.startPing();
            };

            this.ws.onmessage = (event) => {
                try {
                    const msg: WsMessage = JSON.parse(event.data);
                    this.handleMessage(msg);
                } catch (e) {
                    console.error('Failed to parse WebSocket message:', e);
                }
            };

            this.ws.onclose = () => {
                console.log('WebSocket closed');
                this.connected.set(false);
                this.connectionStatus.set('disconnected');
                this.stopPing();
                this.reconnect();
            };

            this.ws.onerror = (error) => {
                console.error('WebSocket error:', error);
                this.ws?.close();
            };
        } catch (e) {
            console.error('Failed to create WebSocket:', e);
            this.reconnect();
        }
    }

    private handleMessage(msg: WsMessage) {
        switch (msg.type) {
            case 'log':
                this.logs.update(logs => {
                    const newLogs = [msg as LogMessage, ...logs];
                    // Keep only last 10000 logs in memory
                    return newLogs.slice(0, 10000);
                });
                break;
            case 'stats':
                this.stats.set(msg as StatsMessage);
                break;
            case 'pong':
                // Pong received, connection alive
                break;
        }
    }

    private startPing() {
        this.pingInterval = setInterval(() => {
            this.send({ type: 'ping' });
        }, 30000); // Ping every 30s
    }

    private stopPing() {
        if (this.pingInterval) {
            clearInterval(this.pingInterval);
            this.pingInterval = null;
        }
    }

    private reconnect() {
        if (this.reconnectAttempts >= this.maxReconnectAttempts) {
            console.error('Max reconnection attempts reached');
            return;
        }

        this.reconnectAttempts++;
        const delay = Math.min(this.reconnectDelay * Math.pow(2, this.reconnectAttempts - 1), this.maxReconnectDelay);

        console.log(`Reconnecting in ${delay}ms (attempt ${this.reconnectAttempts})`);

        setTimeout(() => {
            this.connect();
        }, delay);
    }

    public send(msg: object) {
        if (this.ws?.readyState === WebSocket.OPEN) {
            this.ws.send(JSON.stringify(msg));
        }
    }

    public disconnect() {
        this.stopPing();
        this.ws?.close();
    }
}

// Singleton instance
let wsClient: WebSocketClient | null = null;

export function getWebSocketClient(url?: string): WebSocketClient {
    if (!wsClient && url) {
        wsClient = new WebSocketClient(url);
    }
    return wsClient!;
}

// Derived stores for UI
export const logCount = derived(getWebSocketClient().logs, $logs => $logs.length);
export const recentBlocks = derived(
    getWebSocketClient().logs, 
    $logs => $logs.filter(l => l.level === 'block').slice(0, 100)
);
export const isConnected = derived(getWebSocketClient().connected, $c => $c);
```

```svelte
<!-- dashboard/src/routes/terminal/+page.svelte (Optimasi) -->

<script lang="ts">
    import { onMount, onDestroy } from 'svelte';
    import { AegisTerminal } from '$lib/terminal';
    import { getWebSocketClient, isConnected } from '$lib/websocket';
    import { browser } from '$app/environment';

    let terminalContainer: HTMLElement;
    let terminal: AegisTerminal;
    let wsClient = getWebSocketClient('ws://localhost:8080/ws');

    onMount(() => {
        if (!browser) return;

        // Init terminal dengan optimasi
        terminal = new AegisTerminal(terminalContainer);

        // Subscribe ke logs
        const unsubscribe = wsClient.logs.subscribe(logs => {
            // Batch write: collect semua log baru, write sekali
            const newLogs = logs.slice(0, 50); // Ambil 50 terbaru
            const output = newLogs.map(log => {
                const color = getColorForLevel(log.level);
                return `[${formatTime(log.timestamp)}] ${color}[${log.source}]${reset} ${log.message}\n`;
            }).join('');

            terminal.write(output);
        });

        return () => {
            unsubscribe();
            terminal.dispose();
        };
    });

    function getColorForLevel(level: string): string {
        switch (level) {
            case 'block': return '\x1b[31m'; // Red
            case 'rate_limit': return '\x1b[33m'; // Yellow
            case 'error': return '\x1b[31m'; // Red
            case 'warn': return '\x1b[33m'; // Yellow
            case 'info': return '\x1b[32m'; // Green
            default: return '\x1b[0m'; // Reset
        }
    }

    const reset = '\x1b[0m';

    function formatTime(timestamp: string): string {
        return new Date(timestamp).toLocaleTimeString('en-US', { hour12: false });
    }
</script>

<div class="terminal-wrapper">
    <div class="terminal-header">
        <span class="status" class:connected={$isConnected}>
            {$isConnected ? '● Connected' : '○ Disconnected'}
        </span>
        <button on:click={() => terminal.clear()}>Clear</button>
    </div>
    <div bind:this={terminalContainer} class="terminal-container"></div>
</div>

<style>
    .terminal-wrapper {
        display: flex;
        flex-direction: column;
        height: 100vh;
        background: #1a1a2e;
    }
    .terminal-header {
        display: flex;
        justify-content: space-between;
        align-items: center;
        padding: 8px 16px;
        background: #16213e;
        border-bottom: 1px solid #0f3460;
    }
    .status {
        font-family: monospace;
        font-size: 12px;
    }
    .status.connected {
        color: #2ecc71;
    }
    .status:not(.connected) {
        color: #e74c3c;
    }
    .terminal-container {
        flex: 1;
        overflow: hidden;
    }
    :global(.xterm) {
        padding: 8px;
    }
</style>
```

---

## 🐳 Docker Compose Full Stack

```yaml
# docker-compose.yml (Production Stack)

version: '3.8'

services:
  # PostgreSQL dengan TimescaleDB
  postgres:
    image: timescale/timescaledb:latest-pg16
    container_name: aegis-postgres
    restart: unless-stopped
    environment:
      POSTGRES_USER: aegis
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD:-change_me_in_env}
      POSTGRES_DB: aegis
    volumes:
      - postgres_data:/var/lib/postgresql/data
      - ./init.sql:/docker-entrypoint-initdb.d/init.sql
    ports:
      - "127.0.0.1:5432:5432"  # Hanya localhost untuk security
    networks:
      - aegis-internal
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U aegis"]
      interval: 10s
      timeout: 5s
      retries: 5

  # Redis untuk cache & pub/sub
  redis:
    image: redis:7-alpine
    container_name: aegis-redis
    restart: unless-stopped
    command: >
      redis-server
      --maxmemory 256mb
      --maxmemory-policy allkeys-lru
      --appendonly yes
      --appendfsync everysec
    volumes:
      - redis_data:/data
    ports:
      - "127.0.0.1:6379:6379"
    networks:
      - aegis-internal
    healthcheck:
      test: ["CMD", "redis-cli", "ping"]
      interval: 10s
      timeout: 3s
      retries: 5

  # Aegis Controller
  controller:
    build:
      context: ./controller
      dockerfile: Dockerfile
    container_name: aegis-controller
    restart: unless-stopped
    environment:
      DATABASE_URL: postgres://aegis:${POSTGRES_PASSWORD}@postgres:5432/aegis
      REDIS_URL: redis://redis:6379
      RUST_LOG: info
      BIND_ADDR: 0.0.0.0:8080
    ports:
      - "8080:8080"
    volumes:
      - ./config:/etc/aegis:ro
      - controller_logs:/var/log/aegis
    networks:
      - aegis-internal
      - aegis-external
    depends_on:
      postgres:
        condition: service_healthy
      redis:
        condition: service_healthy
    healthcheck:
      test: ["CMD", "wget", "-q", "--spider", "http://localhost:8080/health"]
      interval: 30s
      timeout: 10s
      retries: 3

  # Aegis Agent (WAF)
  agent:
    build:
      context: ./agent
      dockerfile: Dockerfile
    container_name: aegis-agent
    restart: unless-stopped
    environment:
      CONTROLLER_URL: http://controller:8080
      AGENT_ID: ${AGENT_ID:-default}
      RUST_LOG: info
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./config:/etc/aegis:ro
      - agent_logs:/var/log/aegis
      - agent_certs:/etc/aegis/certs
    networks:
      - aegis-internal
      - aegis-external
    depends_on:
      - controller
    cap_add:
      - NET_BIND_SERVICE  # Untuk bind port 80/443
    # Tidak perlu privileged untuk userspace WAF
    # eBPF butuh --privileged atau CAP_BPF (v0.3+)

  # Dashboard (Svelte static files served by controller)
  # Atau build terpisah:
  dashboard:
    build:
      context: ./dashboard
      dockerfile: Dockerfile
    container_name: aegis-dashboard
    restart: unless-stopped
    ports:
      - "3000:3000"
    environment:
      API_URL: http://controller:8080
      WS_URL: ws://controller:8080/ws
    networks:
      - aegis-external
    depends_on:
      - controller

volumes:
  postgres_data:
  redis_data:
  controller_logs:
  agent_logs:
  agent_certs:

networks:
  aegis-internal:
    internal: true  # Tidak bisa diakses dari luar
  aegis-external:
    driver: bridge
```

---

## 📊 Benchmark Target (Setelah Optimasi)

| Metrik | Sebelum (SSE + SQLite) | Sesudah (WebSocket + PostgreSQL) | Improvement |
|--------|------------------------|-----------------------------------|-------------|
| **Latency (terminal)** | 200-500ms | 5-20ms | **25-100x** |
| **Throughput (logs)** | 100 msg/sec | 10,000 msg/sec | **100x** |
| **Concurrent connections** | 100 | 10,000 | **100x** |
| **Database write** | 50 writes/sec | 5,000 writes/sec | **100x** |
| **Memory (dashboard)** | 200MB+ | 50MB | **4x lebih ringan** |
| **Startup time** | 5s | 1s | **5x** |
| **Reconnect time** | 3-5s | <100ms | **30-50x** |

---

## 🎯 Action Plan: Migrasi ke v0.2

### Phase 1: Database (Minggu 1)
- [ ] Setup Docker Compose (PostgreSQL + Redis)
- [ ] Migrasi schema dari SQLite ke PostgreSQL
- [ ] Test connection pool (sqlx)
- [ ] Benchmark insert 1000 logs

### Phase 2: WebSocket (Minggu 2)
- [ ] Implement WebSocket server (tokio-tungstenite)
- [ ] Implement WebSocket client (Svelte)
- [ ] Test bidirectional communication
- [ ] Implement ping/pong + reconnect

### Phase 3: xterm.js Optimasi (Minggu 2)
- [ ] Buffer + debounce (16ms)
- [ ] WebGL addon
- [ ] Binary stream support
- [ ] Test dengan 1000 lines/sec

### Phase 4: Integrasi (Minggu 3)
- [ ] Connect semua komponen
- [ ] End-to-end testing
- [ ] Load testing (wrk + siege)
- [ ] Docker Compose production test

### Phase 5: Release v0.2 (Minggu 4)
- [ ] Update README
- [ ] GitHub Release dengan binary
- [ ] Blog post: "How we made Aegis 100x faster"
- [ ] Share ke Reddit r/homelab

---

*Document Version: 1.0 — Real-time Optimasi*
*Last Updated: 2026-06-17*
*Status: Ready untuk implementasi*
