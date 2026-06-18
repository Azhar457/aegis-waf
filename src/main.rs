mod config;
mod logging;
mod proxy;
mod rules;
mod vhost;
mod tls;

use axum::{
    body::Body,
    extract::{Host, State, ws::{WebSocketUpgrade, WebSocket}},
    http::{Request, StatusCode},
    response::{Response, IntoResponse},
    routing::{any, get, post},
    Router,
    Json,
};
use std::net::SocketAddr;
use tracing::info;
use clap::{Parser, Subcommand};
use tower_http::cors::{Any, CorsLayer};
use tokio::sync::broadcast;
use tokio_stream::StreamExt;
use std::convert::Infallible;
use axum::response::sse::{Event, Sse};
use tokio_stream::wrappers::BroadcastStream;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

#[derive(Parser, Debug)]
#[command(name = "aegis-waf")]
#[command(about = "Aegis WAF - Next Gen Layer 7 Web Application Firewall", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Path to config file (default: config.toml)
    #[arg(short, long, default_value = "config.toml")]
    config: String,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run WAF in Agent mode (default)
    Agent {
        /// URL of the central Controller
        #[arg(short, long)]
        controller: Option<String>,

        /// Registration token for the Controller
        #[arg(short, long)]
        token: Option<String>,
    },
    /// Run WAF in Controller mode (central logging and dashboard)
    Controller {
        /// Port to bind the Controller server
        #[arg(short, long, default_value_t = 8080)]
        port: u16,
    },
}

// Untuk privilege dropping (bind port <1024 lalu drop ke nobody)
#[cfg(unix)]
fn drop_privileges() {
    if let Err(e) = nix::unistd::setgid(nix::unistd::Gid::from_raw(65534)) {
        tracing::warn!("Failed to setgid: {}", e);
    }
    if let Err(e) = nix::unistd::setuid(nix::unistd::Uid::from_raw(65534)) {
        tracing::warn!("Failed to setuid: {}", e);
    }
}

#[tokio::main]
async fn main() {
    // Init tracing
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .init();

    let cli = Cli::parse();

    match cli.command.unwrap_or(Commands::Agent { controller: None, token: None }) {
        Commands::Agent { controller, token } => {
            run_agent(&cli.config, controller, token).await;
        }
        Commands::Controller { port } => {
            run_controller(port, cli.config).await;
        }
    }
}

async fn run_agent(config_path: &str, controller: Option<String>, token: Option<String>) {
    // Load config
    let cfg = config::load_config(config_path).expect("Failed to load config");
    let config_arc = Arc::new(std::sync::RwLock::new(cfg.clone()));

    // Setup logging
    let clickhouse_url = std::env::var("CLICKHOUSE_URL").unwrap_or_else(|_| "http://localhost:8123".to_string());
    logging::init_db(&clickhouse_url).await.expect("Failed to init ClickHouse DB");

    // Initialize MPSC Channel for logs
    let (log_tx, log_rx) = tokio::sync::mpsc::channel::<logging::WafLogEntry>(10000);

    // Spawn Background Log Worker
    let controller_url = controller.clone();
    let ch_url_clone = clickhouse_url.clone();
    tokio::spawn(async move {
        logging::log_worker(log_rx, ch_url_clone, controller_url).await;
    });

    // Spawn background config reloader
    let config_path_clone = config_path.to_string();
    let config_arc_clone = config_arc.clone();
    tokio::spawn(async move {
        let mut last_modified = std::fs::metadata(&config_path_clone)
            .and_then(|m| m.modified())
            .unwrap_or_else(|_| std::time::SystemTime::now());
        
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            if let Ok(metadata) = std::fs::metadata(&config_path_clone) {
                if let Ok(modified) = metadata.modified() {
                    if modified > last_modified {
                        last_modified = modified;
                        match config::load_config(&config_path_clone) {
                            Ok(new_cfg) => {
                                if let Ok(mut lock) = config_arc_clone.write() {
                                    *lock = new_cfg;
                                    info!("Configuration reloaded successfully from {}", config_path_clone);
                                }
                            }
                            Err(e) => {
                                tracing::error!("Failed to reload config from {}: {:?}", config_path_clone, e);
                            }
                        }
                    }
                }
            }
        }
    });

    if let Some(ctrl) = &controller {
        info!("Running in distributed Agent mode. Connecting to Controller at {}...", ctrl);
        if token.is_some() {
            info!("Using registration token: [REDACTED]");
        }
    } else {
        info!("Running in Standalone Agent mode. Using local configuration.");
    }

    // Build application state
    let blocklist = Arc::new(std::sync::RwLock::new(std::collections::HashSet::new()));
    let state = AppState {
        config: config_arc,
        log_tx,
        blocklist: blocklist.clone(),
    };

    // Spawn background threat intelligence / reputation blocklist sync task
    let blocklist_clone = blocklist.clone();
    let controller_url_clone = controller.clone();
    
    tokio::spawn(async move {
        let client = reqwest::Client::new();
        loop {
            if let Some(ctrl_url) = &controller_url_clone {
                // Agent Mode: Fetch blocklist from Controller
                let url = format!("{}/api/v1/reputation/blocklist", ctrl_url.trim_end_matches('/'));
                match client.get(&url).send().await {
                    Ok(resp) => {
                        if resp.status().is_success() {
                            if let Ok(ips) = resp.json::<Vec<String>>().await {
                                let mut new_blocklist = std::collections::HashSet::new();
                                for ip_str in ips {
                                    if let Ok(ip) = ip_str.parse::<std::net::IpAddr>() {
                                        new_blocklist.insert(ip);
                                    }
                                }
                                if let Ok(mut lock) = blocklist_clone.write() {
                                    *lock = new_blocklist;
                                    tracing::debug!("Reputation blocklist synced. Active blocked IPs: {}", lock.len());
                                }
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!("Error syncing reputation blocklist from controller: {}", e);
                    }
                }
            } else {
                // Standalone Mode: Query ClickHouse
                let clickhouse_url_local = std::env::var("CLICKHOUSE_URL").unwrap_or_else(|_| "http://localhost:8123".to_string());
                let blocklist_standalone = blocklist_clone.clone();
                let client_clone = client.clone();
                
                let query = "SELECT client_ip FROM request_log WHERE action = 'BLOCK' AND timestamp > now() - INTERVAL 5 MINUTE GROUP BY client_ip HAVING count() >= 5 FORMAT TSV";
                let url = format!("{}/?query={}", clickhouse_url_local.trim_end_matches('/'), urlencoding::encode(query));
                if let Ok(resp) = client_clone.get(&url).send().await {
                    if let Ok(text) = resp.text().await {
                        let mut ips = std::collections::HashSet::new();
                        for line in text.lines() {
                            if let Ok(ip) = line.trim().parse::<std::net::IpAddr>() {
                                ips.insert(ip);
                            }
                        }
                        if let Ok(mut lock) = blocklist_standalone.write() {
                            *lock = ips;
                        }
                    }
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
        }
    });

    // Build Axum router
    let app = Router::new()
        .route("/", any(handler))
        .route("/*path", any(handler))
        .with_state(state);

    // Bind HTTP
    let http_addr = SocketAddr::from(([0, 0, 0, 0], cfg.global.port_http));
    let http_listener = tokio::net::TcpListener::bind(http_addr)
        .await
        .expect("Cannot bind HTTP port");

    info!("Aegis Agent WAF listening on http://{}", http_addr);
    info!("Backend default: {}", cfg.vhosts[0].backend);

    // Drop root privileges setelah bind
    #[cfg(unix)]
    if std::process::id() == 0 {
        drop_privileges();
    }

    axum::serve(http_listener, app.into_make_service_with_connect_info::<SocketAddr>()).await.unwrap();
}

#[derive(Clone)]
struct ControllerState {
    tx: broadcast::Sender<logging::WafLogEntry>,
    clickhouse_url: String,
    logging_enabled: Arc<AtomicBool>,
    log_size_limit_mb: Arc<AtomicU64>,
    config_path: String,
}

async fn run_controller(port: u16, config_path: String) {
    info!("Starting Aegis WAF Controller on port {}...", port);

    let clickhouse_url = std::env::var("CLICKHOUSE_URL").unwrap_or_else(|_| "http://localhost:8123".to_string());
    logging::init_db(&clickhouse_url).await.expect("Failed to initialize ClickHouse DB");

    // Initialize broadcast channel for live logs
    let (tx, _rx) = broadcast::channel(10000);

    // App state
    let state = ControllerState {
        tx,
        clickhouse_url: clickhouse_url.clone(),
        logging_enabled: Arc::new(AtomicBool::new(true)),
        log_size_limit_mb: Arc::new(AtomicU64::new(500)), // default 500MB
        config_path,
    };

    // CORS Configuration for local Svelte dashboard
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_headers(Any)
        .allow_methods(Any);

    // Build Controller router
    let app = Router::new()
        .route("/api/v1/agents/register", post(register_agent_handler))
        .route("/api/v1/logs", post(receive_logs_handler))
        .route("/api/v1/logs/stream", get(sse_handler))
        .route("/api/v1/logs", get(get_logs_handler))
        .route("/api/v1/logs/db_size", get(get_db_size_handler))
        .route("/api/v1/logs/export", get(export_logs_handler))
        .route("/api/v1/config", get(get_config_handler).post(post_config_handler))
        .route("/api/v1/vhosts", get(get_vhosts_handler).post(post_vhosts_handler))
        .route("/api/v1/stats", get(get_stats_handler))
        .route("/api/v1/reputation/blocklist", get(get_blocklist_handler))
        .route("/ws/dashboard", get(ws_dashboard_handler))
        .route("/ws/agent", get(ws_agent_handler))
        .fallback_service(tower_http::services::ServeDir::new("dashboard/dist"))
        .layer(cors)
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Cannot bind Controller port");

    info!("Aegis Controller API & Dashboard available at http://{}", addr);

    axum::serve(listener, app).await.unwrap();
}

#[derive(serde::Deserialize)]
struct AgentRegisterRequest {
    hostname: String,
    ip: String,
    port: u16,
    os: String,
}

#[derive(serde::Serialize, serde::Deserialize, Clone)]
struct ConfigPayload {
    logging_enabled: bool,
    log_limit_mb: u64,
}

#[derive(serde::Serialize)]
struct DbSizeResponse {
    size_bytes: u64,
    formatted: String,
}

// Controller API & WS Handlers
async fn register_agent_handler(Json(payload): Json<AgentRegisterRequest>) -> impl IntoResponse {
    info!("Registered agent: {} at {}:{} running {}", payload.hostname, payload.ip, payload.port, payload.os);
    StatusCode::CREATED
}

async fn get_config_handler(State(state): State<ControllerState>) -> impl IntoResponse {
    let payload = ConfigPayload {
        logging_enabled: state.logging_enabled.load(Ordering::Relaxed),
        log_limit_mb: state.log_size_limit_mb.load(Ordering::Relaxed),
    };
    (StatusCode::OK, Json(payload))
}

async fn post_config_handler(
    State(state): State<ControllerState>,
    Json(payload): Json<ConfigPayload>,
) -> impl IntoResponse {
    state.logging_enabled.store(payload.logging_enabled, Ordering::Relaxed);
    state.log_size_limit_mb.store(payload.log_limit_mb, Ordering::Relaxed);
    StatusCode::OK
}

async fn get_vhosts_handler(State(state): State<ControllerState>) -> impl IntoResponse {
    let mut cfg = match config::load_config(&state.config_path) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to load config from {}: {:?}", state.config_path, e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load config").into_response();
        }
    };

    if cfg.vhosts.is_empty() {
        // Create dummy host
        let dummy = config::VHost {
            name: "aegis-demo".to_string(),
            hosts: vec!["*.aegiswaf.demo".to_string()],
            backend: "127.0.0.1:8080".to_string(),
            rate_limit_tiers: vec![],
            logging: Some(config::LoggingConfig { enabled: true, db_path: "logs/aegis-waf.db".to_string() }),
            rules: vec!["SQLI-*".to_string(), "XSS-*".to_string(), "LFI-*".to_string(), "RFI-*".to_string()],
            blocked_countries: vec![],
            geoblock_type: "Blocklist".to_string(),
            custom_rules: vec![],
            ssl: "Auto (Local CA)".to_string(),
            max_body: "10MB".to_string(),
            rate_limit: "600 req/min".to_string(),
        };
        cfg.vhosts.push(dummy);
        // Save it back to config file so it is persisted!
        if let Ok(toml_str) = toml::to_string(&cfg) {
            let _ = std::fs::write(&state.config_path, toml_str);
        }
    }

    (StatusCode::OK, Json(cfg.vhosts)).into_response()
}

async fn post_vhosts_handler(
    State(state): State<ControllerState>,
    Json(vhosts): Json<Vec<config::VHost>>,
) -> impl IntoResponse {
    let mut cfg = match config::load_config(&state.config_path) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("Failed to load config from {}: {:?}", state.config_path, e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to load config").into_response();
        }
    };

    cfg.vhosts = vhosts;

    // Serialize back to TOML and save
    let toml_str = match toml::to_string(&cfg) {
        Ok(t) => t,
        Err(e) => {
            tracing::error!("Failed to serialize updated config to TOML: {:?}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to serialize config").into_response();
        }
    };

    match std::fs::write(&state.config_path, toml_str) {
        Ok(_) => {
            info!("Virtual hosts configuration updated successfully in {}", state.config_path);
            StatusCode::OK.into_response()
        }
        Err(e) => {
            tracing::error!("Failed to write updated config to disk: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to write config file").into_response()
        }
    }
}

async fn get_db_size_handler(State(state): State<ControllerState>) -> impl IntoResponse {
    let size_bytes = logging::get_db_size(&state.clickhouse_url).await.unwrap_or(0);
    
    let formatted = if size_bytes < 1024 {
        format!("{} B", size_bytes)
    } else if size_bytes < 1024 * 1024 {
        format!("{:.1} KB", size_bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", size_bytes as f64 / (1024.0 * 1024.0))
    };

    (StatusCode::OK, Json(DbSizeResponse { size_bytes, formatted }))
}

async fn export_logs_handler(State(state): State<ControllerState>) -> impl IntoResponse {
    let client = reqwest::Client::new();
    let query = "SELECT * FROM request_log FORMAT TSV";
    let url = format!("{}/?query={}", state.clickhouse_url.trim_end_matches('/'), urlencoding::encode(query));
    match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            if let Ok(content) = resp.text().await {
                Response::builder()
                    .header("Content-Type", "text/plain; charset=utf-8")
                    .header("Content-Disposition", "attachment; filename=\"aegis-access.log\"")
                    .body(Body::from(content))
                    .unwrap()
            } else {
                (StatusCode::INTERNAL_SERVER_ERROR, "Failed to read body").into_response()
            }
        }
        _ => (StatusCode::INTERNAL_SERVER_ERROR, "Failed to export logs from ClickHouse").into_response(),
    }
}

async fn receive_logs_handler(
    State(state): State<ControllerState>,
    Json(logs): Json<Vec<logging::WafLogEntry>>,
) -> impl IntoResponse {
    // Check if logging is enabled
    if !state.logging_enabled.load(Ordering::Relaxed) {
        return StatusCode::OK;
    }

    let client = reqwest::Client::new();
    let mut body = String::new();
    let logs_clone = logs.clone();
    for entry in &logs_clone {
        if let Ok(json) = serde_json::to_string(entry) {
            body.push_str(&json);
            body.push('\n');
        }
    }
    let url = format!("{}/?query=INSERT INTO request_log FORMAT JSONEachRow", state.clickhouse_url.trim_end_matches('/'));
    let _ = client.post(&url).body(body).send().await;

    // Broadcast logs to connected dashboards
    for log in logs {
        let _ = state.tx.send(log);
    }
    StatusCode::OK
}

async fn sse_handler(
    State(state): State<ControllerState>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let rx = state.tx.subscribe();
    let stream = BroadcastStream::new(rx).map(|res| {
        match res {
            Ok(log) => {
                let json = serde_json::to_string(&log).unwrap();
                Ok(Event::default().data(json))
            }
            Err(_) => {
                Ok(Event::default().comment("lost message"))
            }
        }
    });
    Sse::new(stream).keep_alive(axum::response::sse::KeepAlive::default())
}

async fn get_blocklist_handler(
    State(state): State<ControllerState>,
) -> impl IntoResponse {
    let client = reqwest::Client::new();
    let query = "SELECT client_ip FROM request_log WHERE action = 'BLOCK' AND timestamp > now() - INTERVAL 5 MINUTE GROUP BY client_ip HAVING count() >= 5 FORMAT TSV";
    let url = format!("{}/?query={}", state.clickhouse_url.trim_end_matches('/'), urlencoding::encode(query));
    if let Ok(resp) = client.get(&url).send().await {
        if let Ok(text) = resp.text().await {
            let mut ips = Vec::new();
            for line in text.lines() {
                let ip = line.trim().to_string();
                if !ip.is_empty() {
                    ips.push(ip);
                }
            }
            return (StatusCode::OK, Json(ips)).into_response();
        }
    }
    (StatusCode::INTERNAL_SERVER_ERROR, Json(Vec::<String>::new())).into_response()
}

async fn get_logs_handler(
    State(state): State<ControllerState>,
) -> impl IntoResponse {
    let client = reqwest::Client::new();
    let query = "SELECT timestamp, client_ip, method, path, action, rule_id, reason FROM request_log ORDER BY timestamp DESC LIMIT 100 FORMAT JSONEachRow";
    let url = format!("{}/?query={}", state.clickhouse_url.trim_end_matches('/'), urlencoding::encode(query));
    if let Ok(resp) = client.get(&url).send().await {
        if let Ok(text) = resp.text().await {
            let mut logs = Vec::new();
            for line in text.lines() {
                if let Ok(log) = serde_json::from_str::<logging::WafLogEntry>(line) {
                    logs.push(log);
                }
            }
            return (StatusCode::OK, Json(logs)).into_response();
        }
    }
    (StatusCode::INTERNAL_SERVER_ERROR, Json(Vec::<logging::WafLogEntry>::new())).into_response()
}

async fn get_stats_handler(
    State(state): State<ControllerState>,
) -> impl IntoResponse {
    match logging::get_stats(&state.clickhouse_url, 24).await {
        Ok(stats) => (StatusCode::OK, Json(stats)).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, format!("DB error: {}", e)).into_response()
    }
}

async fn ws_dashboard_handler(
    ws: WebSocketUpgrade,
    State(state): State<ControllerState>,
) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_dashboard_socket(socket, state))
}

async fn ws_agent_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(|socket| handle_agent_socket(socket))
}

async fn handle_dashboard_socket(mut socket: WebSocket, state: ControllerState) {
    info!("Dashboard client connected via WebSocket");
    let mut rx = state.tx.subscribe();
    let clickhouse_url = state.clickhouse_url.clone();
    let mut stats_interval = tokio::time::interval(std::time::Duration::from_secs(5));

    loop {
        tokio::select! {
            Ok(log) = rx.recv() => {
                let json = serde_json::json!({
                    "type": "log",
                    "timestamp": log.timestamp,
                    "client_ip": log.client_ip,
                    "method": log.method,
                    "path": log.path,
                    "action": log.action,
                    "rule_id": log.rule_id,
                    "reason": log.reason
                });
                if socket.send(axum::extract::ws::Message::Text(json.to_string())).await.is_err() {
                    break;
                }
            }
            _ = stats_interval.tick() => {
                if let Ok(stats) = logging::get_stats(&clickhouse_url, 24).await {
                    let json = serde_json::json!({
                        "type": "stats",
                        "total_requests": stats.total_requests,
                        "blocked": stats.blocked,
                        "rate_limited": stats.rate_limited
                    });
                    if socket.send(axum::extract::ws::Message::Text(json.to_string())).await.is_err() {
                        break;
                    }
                }
            }
            Some(msg) = socket.recv() => {
                if msg.is_err() {
                    break;
                }
            }
        }
    }
    info!("Dashboard client disconnected");
}

async fn handle_agent_socket(_socket: WebSocket) {
    info!("Agent client connected via WebSocket");
}

// Shared application state for Agent
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<std::sync::RwLock<config::Config>>,
    pub log_tx: tokio::sync::mpsc::Sender<logging::WafLogEntry>,
    pub blocklist: Arc<std::sync::RwLock<std::collections::HashSet<std::net::IpAddr>>>,
}

// Main request handler for Agent
async fn handler(
    state: State<AppState>,
    axum::extract::ConnectInfo(addr): axum::extract::ConnectInfo<SocketAddr>,
    host: Option<Host>,
    req: Request<Body>,
) -> Response<Body> {
    proxy::forward_request(state, addr, host, req).await
}