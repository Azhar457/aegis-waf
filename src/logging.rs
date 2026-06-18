use std::time::Duration;
use serde::{Serialize, Deserialize};
use tokio::sync::mpsc::Receiver;
use crate::config::Config;

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

// Inisialisasi ClickHouse Table
pub async fn init_db(clickhouse_url: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();
    let ddl = "
        CREATE TABLE IF NOT EXISTS request_log (
            timestamp DateTime64(3, 'UTC'),
            client_ip String,
            method String,
            path String,
            action String,
            rule_id String,
            reason String
        ) ENGINE = MergeTree()
        ORDER BY (timestamp, client_ip)
        TTL toDateTime(timestamp) + INTERVAL 30 DAY DELETE
    ";
    
    let url = format!("{}/?query={}", clickhouse_url.trim_end_matches('/'), urlencoding::encode(ddl));
    let res = client.post(&url).send().await?;
    if !res.status().is_success() {
        let err = res.text().await?;
        return Err(format!("ClickHouse init error: {}", err).into());
    }
    tracing::info!("ClickHouse request_log table initialized successfully");
    Ok(())
}

// Mendapatkan statistik realtime dari ClickHouse
pub async fn get_stats(clickhouse_url: &str, hours: u32) -> Result<Stats, Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();
    let query = format!(
        "SELECT count(), countIf(action = 'BLOCK'), countIf(action = 'RATE_LIMIT') FROM request_log WHERE timestamp > now() - INTERVAL {} HOUR FORMAT TSV",
        hours
    );
    let url = format!("{}/?query={}", clickhouse_url.trim_end_matches('/'), urlencoding::encode(&query));
    
    let res = client.get(&url).send().await?;
    if !res.status().is_success() {
        return Err("ClickHouse stats query failed".into());
    }
    
    let text = res.text().await?;
    let parts: Vec<&str> = text.trim().split('\t').collect();
    if parts.len() == 3 {
        Ok(Stats {
            total_requests: parts[0].parse().unwrap_or(0),
            blocked: parts[1].parse().unwrap_or(0),
            rate_limited: parts[2].parse().unwrap_or(0),
        })
    } else {
        Ok(Stats { total_requests: 0, blocked: 0, rate_limited: 0 })
    }
}

// Mendapatkan jumlah disk usage dari ClickHouse Table
pub async fn get_db_size(clickhouse_url: &str) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();
    let query = "SELECT total_bytes FROM system.tables WHERE name = 'request_log' FORMAT TSV";
    let url = format!("{}/?query={}", clickhouse_url.trim_end_matches('/'), urlencoding::encode(query));
    
    let res = client.get(&url).send().await?;
    if res.status().is_success() {
        let text = res.text().await?;
        Ok(text.trim().parse().unwrap_or(0))
    } else {
        Ok(0)
    }
}

// Worker untuk membaca channel log dan mengirimkannya secara batch ke ClickHouse
pub async fn log_worker(mut rx: Receiver<WafLogEntry>, clickhouse_url: String, controller_url: Option<String>) {
    let client = reqwest::Client::new();
    let batch_interval = Duration::from_secs(1);
    let max_batch_size = 5000;
    
    let mut batch = Vec::new();
    let mut last_flush = tokio::time::Instant::now();

    loop {
        let timeout = batch_interval.checked_sub(last_flush.elapsed()).unwrap_or(Duration::from_millis(10));
        
        tokio::select! {
            Some(entry) = rx.recv() => {
                batch.push(entry);
                if batch.len() >= max_batch_size {
                    flush_logs(&batch, &clickhouse_url, &controller_url, &client).await;
                    batch.clear();
                    last_flush = tokio::time::Instant::now();
                }
            }
            _ = tokio::time::sleep(timeout) => {
                if !batch.is_empty() {
                    flush_logs(&batch, &clickhouse_url, &controller_url, &client).await;
                    batch.clear();
                }
                last_flush = tokio::time::Instant::now();
            }
        }
    }
}

async fn flush_logs(batch: &[WafLogEntry], clickhouse_url: &str, controller_url: &Option<String>, client: &reqwest::Client) {
    if batch.is_empty() { return; }

    if let Some(ctrl_url) = controller_url {
        // Mode Agent: Kirim JSON Array ke Controller
        let url = format!("{}/api/v1/logs", ctrl_url.trim_end_matches('/'));
        if let Err(e) = client.post(&url).json(batch).send().await {
            tracing::error!("Error posting logs to controller: {}", e);
        }
    } else {
        // Mode Controller: Bulk Insert ke ClickHouse menggunakan JSONEachRow
        let mut body = String::new();
        for entry in batch {
            if let Ok(json) = serde_json::to_string(entry) {
                body.push_str(&json);
                body.push('\n');
            }
        }
        
        let url = format!("{}/?query=INSERT INTO request_log FORMAT JSONEachRow", clickhouse_url.trim_end_matches('/'));
        let res = client.post(&url).body(body).send().await;
        if let Err(e) = res {
            tracing::error!("Failed to insert logs to ClickHouse: {}", e);
        } else if let Ok(resp) = res {
            if !resp.status().is_success() {
                let err_text = resp.text().await.unwrap_or_default();
                tracing::error!("ClickHouse insert error: {}", err_text);
            }
        }
    }
}