use rusqlite::{Connection, params};
use std::path::Path;
use std::fs;
use crate::config::Config;
use tokio::sync::mpsc::Receiver;
use std::time::Duration;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct WafLogEntry {
    pub timestamp: String,
    pub client_ip: String,
    pub method: String,
    pub path: String,
    pub action: String,
    pub rule_id: String,
    pub reason: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct Stats {
    pub total_requests: i64,
    pub blocked: i64,
    pub rate_limited: i64,
}

pub fn init_db(cfg: &Config) -> Result<(), Box<dyn std::error::Error>> {
    let log_dir = Path::new(&cfg.global.log_dir);
    fs::create_dir_all(log_dir)?;
    let db_path = log_dir.join("aegis-waf.db");
    let conn = Connection::open(db_path)?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS request_log (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            timestamp TEXT NOT NULL,
            client_ip TEXT NOT NULL,
            method TEXT,
            path TEXT,
            status INTEGER,
            rule_id TEXT,
            reason TEXT
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_request_log_reputation ON request_log (timestamp, method, client_ip)",
        [],
    )?;
    Ok(())
}

pub fn get_stats(db_path: &str, hours: u32) -> Result<Stats, Box<dyn std::error::Error>> {
    let conn = Connection::open(db_path)?;
    let mut stmt = conn.prepare(
        "SELECT 
            COUNT(*) as total,
            SUM(CASE WHEN method = 'BLOCK' THEN 1 ELSE 0 END) as blocked,
            SUM(CASE WHEN method = 'RATE_LIMIT' THEN 1 ELSE 0 END) as rate_limited
         FROM request_log 
         WHERE timestamp > datetime('now', ?1 || ' hours')"
    )?;

    let stats = stmt.query_row([format!("-{}", hours)], |row| {
        let total: i64 = row.get(0)?;
        let blocked: i64 = row.get(1).unwrap_or(0);
        let rate_limited: i64 = row.get(2).unwrap_or(0);
        Ok(Stats {
            total_requests: total,
            blocked,
            rate_limited,
        })
    })?;

    Ok(stats)
}

// Background worker to batch logs and write them to local DB or POST to Controller
pub async fn log_worker(mut rx: Receiver<WafLogEntry>, cfg: Config, controller_url: Option<String>) {
    let db_path = Path::new(&cfg.global.log_dir).join("aegis-waf.db");
    let client = reqwest::Client::new();
    let batch_interval = Duration::from_secs(1);
    let max_batch_size = 100;
    
    let mut batch = Vec::new();
    let mut last_flush = tokio::time::Instant::now();

    loop {
        let timeout = batch_interval.checked_sub(last_flush.elapsed()).unwrap_or(Duration::from_millis(10));
        
        tokio::select! {
            // Membaca log dari channel
            Some(entry) = rx.recv() => {
                batch.push(entry);
                if batch.len() >= max_batch_size {
                    flush_logs(&batch, &db_path, &controller_url, &client).await;
                    batch.clear();
                    last_flush = tokio::time::Instant::now();
                }
            }
            // Flush jika melebihi batas waktu (timeout interval 1 detik)
            _ = tokio::time::sleep(timeout) => {
                if !batch.is_empty() {
                    flush_logs(&batch, &db_path, &controller_url, &client).await;
                    batch.clear();
                }
                last_flush = tokio::time::Instant::now();
            }
        }
    }
}

async fn flush_logs(batch: &[WafLogEntry], db_path: &Path, controller_url: &Option<String>, client: &reqwest::Client) {
    if batch.is_empty() {
        return;
    }

    if let Some(ctrl_url) = controller_url {
        // Mode Distributed WAF - Kirim JSON Array ke Controller
        let url = format!("{}/api/v1/logs", ctrl_url.trim_end_matches('/'));
        match client.post(&url).json(batch).send().await {
            Ok(resp) => {
                if !resp.status().is_success() {
                    tracing::error!("Failed to post logs to controller: status {}", resp.status());
                }
            }
            Err(e) => {
                tracing::error!("Error posting logs to controller: {}", e);
            }
        }
    } else {
        // Mode Standalone WAF - Tulis ke SQLite lokal menggunakan transaksi (bulk insert)
        let db_path_clone = db_path.to_path_buf();
        let batch_clone = batch.to_vec();
        let res = tokio::task::spawn_blocking(move || {
            let mut conn = Connection::open(db_path_clone)?;
            let tx = conn.transaction()?;
            {
                let mut stmt = tx.prepare(
                    "INSERT INTO request_log (timestamp, client_ip, method, path, status, rule_id, reason)
                     VALUES (?1, ?2, ?3, ?4, 403, ?5, ?6)"
                )?;
                for entry in batch_clone {
                    stmt.execute(params![
                        entry.timestamp,
                        entry.client_ip,
                        entry.action, // method diganti action (misal BLOCK / RATE_LIMIT) agar informatif di DB
                        entry.path,
                        entry.rule_id,
                        entry.reason
                    ])?;
                }
            }
            tx.commit()?;
            Ok::<(), rusqlite::Error>(())
        }).await;

        if let Err(e) = res {
            tracing::error!("Log bulk insert join error: {:?}", e);
        } else if let Ok(Err(db_err)) = res {
            tracing::error!("Log bulk insert SQLite error: {:?}", db_err);
        }
    }
}