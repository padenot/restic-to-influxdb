use chrono::DateTime;
use chrono::Utc;
use clap::Parser;
use influxdb::{Client, InfluxDbWriteable};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::io::{self, BufRead};
use std::time::SystemTime;
use std::time::Duration;

// {"message_type":"status","seconds_elapsed":34,"percent_done":1.0000004500042092,"total_files":118816,"files_done":118816,"total_bytes":125958821830,"bytes_done":125958878512}

#[derive(InfluxDbWriteable, Debug)]
struct StatusMessageOut {
    time: DateTime<Utc>,
    message_type: String,
    seconds_elapsed: u64,
    seconds_remaining: u64,
    percent_done: f64,
    total_files: u64,
    files_done: u64,
    total_bytes: u64,
    bytes_done: u64,
    error_count: u64,
    // current_files: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct StatusMessage {
    message_type: String,
    seconds_elapsed: u64,
    seconds_remaining: Option<u64>,
    percent_done: f64,
    total_files: u64,
    files_done: u64,
    total_bytes: u64,
    bytes_done: u64,
    error_count: Option<u64>,
    current_files: Option<Vec<String>>,
}

// {"message_type":"summary","files_new":4,"files_changed":10,"files_unmodified":2331042,"dirs_new":0,"dirs_changed":15,"dirs_unmodified":888795,"data_blobs":28,"tree_blobs":14,"data_added":27699291,"total_files_processed":2331056,"total_bytes_processed":2306593302132,"total_duration":555.877498816,"snapshot_id":"59717ddd45f793c52138cff2edef7b250e6f978a73883b5acd3dd36851da5f7a"}

#[derive(Debug, Deserialize, Serialize)]
struct SummaryMessage {
  message_type: String,
  files_new: u64,
  files_changed: u64,
  files_unmodified: u64,
  dirs_new: u64,
  dirs_changed: u64,
  dirs_unmodified: u64,
  data_blobs: u64,
  tree_blobs: u64,
  data_added: u64,
  total_files_processed: u64,
  total_bytes_processed: u64,
  total_duration: f64,
  snapshot_id: String
}

#[derive(InfluxDbWriteable, Debug)]
struct SummaryMessageOut {
  time: DateTime<Utc>,
  message_type: String,
  files_new: u64,
  files_changed: u64,
  files_unmodified: u64,
  dirs_new: u64,
  dirs_changed: u64,
  dirs_unmodified: u64,
  data_blobs: u64,
  tree_blobs: u64,
  data_added: u64,
  total_files_processed: u64,
  total_bytes_processed: u64,
  total_duration: f64,
  snapshot_id: String
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
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

    let mut print_time = Utc::now() - Duration::from_secs(cli.interval);

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

        if type_ == "status" {
            // very spammy, limit database writes
            if Utc::now() < print_time + Duration::from_secs(cli.interval) {
                continue;
            }
            print_time = Utc::now();
            let status: StatusMessage = match serde_json::from_str(&line) {
                Ok(message) => message,
                Err(_) => {
                    panic!("Parse error");
                }
            };
            let data = StatusMessageOut {
                time: SystemTime::now().into(),
                message_type: status.message_type,
                seconds_elapsed: status.seconds_elapsed,
                seconds_remaining: status.seconds_remaining.unwrap_or(0),
                percent_done: status.percent_done,
                total_files: status.total_files,
                files_done: status.files_done,
                total_bytes: status.total_bytes,
                bytes_done: status.bytes_done,
                error_count: status.error_count.unwrap_or(0),
            };

            println!("-> {:?}", data);

            let query = data.into_query("status_message");
            client.query(&query).await?;
        } else if type_ == "summary" {
            let summary: SummaryMessage = match serde_json::from_str(&line) {
                Ok(message) => message,
                Err(_) => {
                    panic!("Parse error");
                }
            };

            println!("-> {:?}", summary);

            let out = SummaryMessageOut {
              time: SystemTime::now().into(),
              message_type: summary.message_type,
              files_new: summary.files_new,
              files_changed: summary.files_changed,
              files_unmodified: summary.files_unmodified,
              dirs_new: summary.dirs_new,
              dirs_changed: summary.dirs_changed,
              dirs_unmodified: summary.dirs_unmodified,
              data_blobs: summary.data_blobs,
              tree_blobs: summary.tree_blobs,
              data_added: summary.data_added,
              total_files_processed: summary.total_files_processed,
              total_bytes_processed: summary.total_bytes_processed,
              total_duration: summary.total_duration,
              snapshot_id: summary.snapshot_id
            };

            let query = out.into_query("summary_message");
            client.query(&query).await?;
        } else if type_ == "error" {
            // ... handle error message
        }
    }

    Ok(())
}
