use chrono::DateTime;
use chrono::Utc;
use clap::Parser;
use influxdb::{Client, InfluxDbWriteable};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};
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

    /// InfluxDB user (required with --password and --database for InfluxDB output)
    #[arg(short, long)]
    user: Option<String>,

    /// InfluxDB password
    #[arg(short, long)]
    password: Option<String>,

    /// InfluxDB database
    #[arg(short, long)]
    database: Option<String>,

    /// InfluxDB host
    #[arg(long, default_value = "http://localhost:8086")]
    host: String,

    /// Atomically write Prometheus textfile-collector metrics here
    #[arg(long)]
    prometheus_file: Option<PathBuf>,
}

fn render_prometheus(
    status: Option<&StatusMessage>,
    summary: Option<&SummaryMessage>,
    success: bool,
    timestamp: i64,
) -> String {
    let mut lines = vec![
        "# HELP restic_backup_running Whether a restic backup is currently running.".to_string(),
        "# TYPE restic_backup_running gauge".to_string(),
        format!(
            "restic_backup_running {}",
            u8::from(status.is_some() && summary.is_none())
        ),
        "# HELP restic_backup_success Whether the latest completed backup succeeded.".to_string(),
        "# TYPE restic_backup_success gauge".to_string(),
        format!("restic_backup_success {}", u8::from(success)),
        format!("restic_backup_last_update_timestamp_seconds {timestamp}"),
    ];

    if let Some(status) = status {
        lines.extend([
            format!("restic_backup_seconds_elapsed {}", status.seconds_elapsed),
            format!(
                "restic_backup_seconds_remaining {}",
                status.seconds_remaining
            ),
            format!("restic_backup_percent_done {}", status.percent_done * 100.0),
            format!("restic_backup_files_done {}", status.files_done),
            format!("restic_backup_total_files {}", status.total_files),
            format!("restic_backup_bytes_done {}", status.bytes_done),
            format!("restic_backup_total_bytes {}", status.total_bytes),
            format!("restic_backup_error_count {}", status.error_count),
        ]);
    }

    if let Some(summary) = summary {
        let snapshot_id = summary
            .snapshot_id
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        lines.extend([
            format!("restic_backup_duration_seconds {}", summary.total_duration),
            format!("restic_backup_data_added_bytes {}", summary.data_added),
            format!(
                "restic_backup_bytes_processed {}",
                summary.total_bytes_processed
            ),
            format!(
                "restic_backup_files_processed {}",
                summary.total_files_processed
            ),
            format!("restic_backup_files_new {}", summary.files_new),
            format!("restic_backup_files_changed {}", summary.files_changed),
            format!("restic_backup_snapshot_info{{snapshot_id=\"{snapshot_id}\"}} 1"),
        ]);
    }

    lines.push(String::new());
    lines.join("\n")
}

fn write_prometheus_file(path: &Path, contents: &str) -> io::Result<()> {
    let temp = path.with_extension("prom.tmp");
    std::fs::write(&temp, contents)?;
    std::fs::rename(temp, path)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    let stdin = io::stdin();

    let client = match (cli.user, cli.password, cli.database) {
        (Some(user), Some(password), Some(database)) => {
            Some(Client::new(cli.host, database).with_auth(user, password))
        }
        (None, None, None) => None,
        _ => return Err("--user, --password and --database must be supplied together".into()),
    };

    if client.is_none() && cli.prometheus_file.is_none() && !cli.dry_run {
        return Err("configure InfluxDB output, --prometheus-file, or --dry-run".into());
    }

    // Always write the first item
    let mut last_write_time = Utc::now() - Duration::from_secs(cli.interval) * 2;
    let mut saw_summary = false;

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
                if let Some(path) = &cli.prometheus_file {
                    let rendered =
                        render_prometheus(Some(&status), None, false, Utc::now().timestamp());
                    write_prometheus_file(path, &rendered)?;
                }
                status.into_query("status_message")
            }
            "summary" => {
                saw_summary = true;
                let mut summary: SummaryMessage = match serde_json::from_str(&line) {
                    Ok(message) => message,
                    Err(e) => {
                        eprintln!("Summary parse error: {:?}", e);
                        continue;
                    }
                };

                summary.time = SystemTime::now().into();
                if let Some(path) = &cli.prometheus_file {
                    let rendered =
                        render_prometheus(None, Some(&summary), true, Utc::now().timestamp());
                    write_prometheus_file(path, &rendered)?;
                }
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
        } else if let Some(client) = &client {
            client.query(&query).await?;
        }
    }

    if let Some(path) = &cli.prometheus_file {
        if !saw_summary {
            let rendered = render_prometheus(None, None, false, Utc::now().timestamp());
            write_prometheus_file(path, &rendered)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_status_as_prometheus_metrics() {
        let status = StatusMessage {
            time: Utc::now(),
            message_type: "status".into(),
            seconds_elapsed: 12,
            seconds_remaining: 34,
            percent_done: 0.25,
            files_done: 5,
            total_files: 20,
            bytes_done: 100,
            total_bytes: 400,
            error_count: 2,
            current_files: String::new(),
        };

        let rendered = render_prometheus(Some(&status), None, false, 1_700_000_000);

        assert!(rendered.contains("restic_backup_running 1"));
        assert!(rendered.contains("restic_backup_percent_done 25"));
        assert!(rendered.contains("restic_backup_files_done 5"));
        assert!(rendered.contains("restic_backup_last_update_timestamp_seconds 1700000000"));
    }

    #[test]
    fn renders_incomplete_run_as_stopped_failure() {
        let rendered = render_prometheus(None, None, false, 1_700_000_002);

        assert!(rendered.contains("restic_backup_running 0"));
        assert!(rendered.contains("restic_backup_success 0"));
    }

    #[test]
    fn renders_completed_summary_as_prometheus_metrics() {
        let summary = SummaryMessage {
            time: Utc::now(),
            message_type: "summary".into(),
            data_added: 4096,
            data_blobs: 2,
            dirs_changed: 3,
            dirs_new: 4,
            dirs_unmodified: 5,
            files_changed: 6,
            files_new: 7,
            files_unmodified: 8,
            snapshot_id: "abc123".into(),
            total_bytes_processed: 8192,
            total_duration: 9.5,
            total_files_processed: 10,
            tree_blobs: 11,
        };

        let rendered = render_prometheus(None, Some(&summary), true, 1_700_000_001);

        assert!(rendered.contains("restic_backup_running 0"));
        assert!(rendered.contains("restic_backup_success 1"));
        assert!(rendered.contains("restic_backup_data_added_bytes 4096"));
        assert!(rendered.contains("restic_backup_snapshot_info{snapshot_id=\"abc123\"} 1"));
    }
}
