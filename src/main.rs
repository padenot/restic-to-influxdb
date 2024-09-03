use chrono::DateTime;
use chrono::Utc;
use clap::Parser;
use influxdb::{Client, InfluxDbWriteable};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{self, BufRead};
use std::time::Duration;
use std::time::SystemTime;

fn deserialize_current_files<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s: Option<Vec<String>> = Option::deserialize(deserializer)?;
    match s {
        Some(v) => Ok(v.join(",")),
        None => Ok(String::new()),
    }
}

// Sometimes some fields aren't present.
// time is added after deserialization and is mandatoring for InfluxDbWriteable
// file list are deserialized into a comma-separated list
#[derive(InfluxDbWriteable, Debug, Deserialize)]
struct StatusMessage {
    #[serde(default)]
    time: DateTime<Utc>,
    message_type: String,
    seconds_elapsed: u64,
    #[serde(default)]
    seconds_remaining: u64,
    percent_done: f64,
    #[serde(default)]
    files_done: u64,
    total_files: u64,
    #[serde(default)]
    bytes_done: u64,
    total_bytes: u64,
    #[serde(default)]
    error_count: u64,
    #[serde(deserialize_with = "deserialize_current_files", default)]
    current_files: String,
}

#[derive(InfluxDbWriteable, Debug, Deserialize)]
struct ErrorMessage {
    #[serde(default)]
    time: DateTime<Utc>,
    message_type: String,
    during: String,
    item: String,
}

#[derive(Debug, Deserialize, Serialize, InfluxDbWriteable)]
struct SummaryMessage {
    #[serde(default)]
    time: DateTime<Utc>,
    message_type: String,
    data_added: u64,
    data_blobs: u64,
    dirs_changed: u64,
    dirs_new: u64,
    dirs_unmodified: u64,
    files_changed: u64,
    files_new: u64,
    files_unmodified: u64,
    snapshot_id: String,
    total_bytes_processed: u64,
    total_duration: f64,
    total_files_processed: u64,
    tree_blobs: u64,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// Enable dry-run mode: don't write to influxdb
    #[arg(long, default_value_t = false)]
    dry_run: bool,

    /// Enable verbose mode
    #[arg(short, long, default_value_t = false)]
    verbose: bool,

    /// Status interval
    #[arg(short, long, default_value_t = 10)]
    interval: u64,

    /// InfluxDB user
    #[arg(short, long)]
    user: String,

    /// InfluxDB password
    #[arg(short, long)]
    password: String,

    /// InfluxDB database
    #[arg(short, long)]
    database: String,

    /// InfluxDB host
    #[arg(long, default_value = "http://localhost:8086")]
    host: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let stdin = io::stdin();

    let client = Client::new(cli.host, cli.database).with_auth(cli.user, cli.password);

    // Always write the first item
    let mut last_write_time = Utc::now() - Duration::from_secs(cli.interval) * 2;

    for line in stdin.lock().lines() {
        let line = line.unwrap();
        let message: Value = match serde_json::from_str(&line) {
            Ok(message) => message,
            Err(_) => continue,
        };
        let mut message = message.as_object().unwrap().clone();
        let type_ = message
            .remove("message_type")
            .unwrap()
            .as_str()
            .unwrap()
            .to_string();

        let query = match type_.as_str() {
            "status" => {
                // very spammy, limit database writes
                if Utc::now() < last_write_time + Duration::from_secs(cli.interval) {
                    continue;
                }
                last_write_time = Utc::now();
                let mut status: StatusMessage = match serde_json::from_str(&line) {
                    Ok(message) => message,
                    Err(e) => {
                        eprintln!("Status parse error: {:?}", e);
                        continue;
                    }
                };

                status.time = SystemTime::now().into();
                status.into_query("status_message")
            }
            "summary" => {
                let mut summary: SummaryMessage = match serde_json::from_str(&line) {
                    Ok(message) => message,
                    Err(e) => {
                        eprintln!("Summary parse error: {:?}", e);
                        continue;
                    }
                };

                summary.time = SystemTime::now().into();
                summary.into_query("summary_message")
            }
            "error" => {
                let mut error: ErrorMessage = match serde_json::from_str(&line) {
                    Ok(message) => message,
                    Err(e) => {
                        eprintln!("Error parse error: {:?}", e);
                        continue;
                    }
                };
                error.time = SystemTime::now().into();
                error.into_query("error_message")
            }
            _ => {
                continue;
            }
        };

        if cli.dry_run {
            println!("-> {:?}", query);
        } else {
            client.query(&query).await?;
        }
    }

    Ok(())
}
