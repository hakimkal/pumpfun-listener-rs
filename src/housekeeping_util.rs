use std::fs;
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use chrono::Utc;
use tracing::{info, error,Level};
use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter, Registry};
use tracing_subscriber::fmt::{time, MakeWriter};
use flexi_logger::{Criterion, Cleanup, FileSpec, Logger, Naming};
use tokio::time::sleep;
use std::time::Duration as StdDuration;
use anyhow::__private::not;
use tracing_subscriber::util::SubscriberInitExt;

// Wrapper around Arc<Mutex<File>> to implement Write
type FileHandle = Arc<Mutex<std::fs::File>>;
const LOG_FILE_DIRS: &str ="/var/logs/pumpfun_ingestion";
struct LogFileWrapper(FileHandle);

impl Write for LogFileWrapper {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut file = self.0.lock().unwrap();
        file.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut file = self.0.lock().unwrap();
        file.flush()
    }
}

// Implement MakeWriter for our wrapper
struct LogMakeWriter(FileHandle);

impl<'a> MakeWriter<'a> for LogMakeWriter {
    type Writer = LogFileWrapper;

    fn make_writer(&'a self) -> Self::Writer {
        LogFileWrapper(self.0.clone())
    }
}



pub fn init_logging() {
    let log_path = format!("{}/app.log",LOG_FILE_DIRS);

    // Ensure parent directory exists
    if let Some(parent) = std::path::Path::new(&log_path).parent() {
        fs::create_dir_all(parent)
            .expect("Failed to create log directory");
    }

    // Initialize rolling log file manually
    let file = OpenOptions::new()
        .append(true)
        .create(true)
        .open(log_path)
        .expect("Failed to create log file");

    let file_handle = Arc::new(Mutex::new(file));

    // stdout layer
    let stdout_layer = fmt::layer()
        .with_writer(std::io::stdout)
        .with_target(false)
        .with_level(true);

    // file layer using MakeWriter wrapper
    let file_layer = fmt::layer()
        .with_writer(LogMakeWriter(file_handle.clone()))
        .with_target(false)
        .with_level(true);

    // Combine layers
    Registry::default()
        .with(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with(stdout_layer)
        .with(file_layer)
        .init();

    info!("Logging initialized");
}

pub fn clean_old_logs(dir: &str, max_age_days: i64) {
    // Resolve directory relative to current working directory
    let path = PathBuf::from(dir);
    info!("Checking logs directory: {}", path.display());

    if !path.exists() {
        error!("Logs directory does not exist: {}", path.display());
        return;
    }

    let now = Utc::now();

    match fs::read_dir(&path) {
        Ok(entries) => {
            let mut any_files = false;
            for entry in entries {
                match entry {
                    Ok(entry) => {
                        let path = entry.path();
                        any_files = true;
                        info!("Found file: {}", path.display());
                        if path.is_file() {
                            match fs::metadata(&path) {
                                Ok(metadata) => match metadata.modified() {
                                    Ok(modified) => {
                                        let modified_time = chrono::DateTime::<Utc>::from(modified);
                                        let age_days = now.signed_duration_since(modified_time).num_days();
                                        info!("File {} is {} days old", path.display(), age_days);
                                        if age_days >= max_age_days {
                                            match fs::remove_file(&path) {
                                                Ok(_) => info!("Deleted old log: {}", path.display()),
                                                Err(e) => error!("Failed to delete {}: {}", path.display(), e),
                                            }
                                        }
                                    }
                                    Err(e) => error!("Failed to get modified time for {}: {}", path.display(), e),
                                },
                                Err(e) => error!("Failed to get metadata for {}: {}", path.display(), e),
                            }
                        }
                    }
                    Err(e) => error!("Failed to read entry in {}: {}", path.display(), e),
                }
            }

            if !any_files {
                info!("No files found in directory {}", path.display());
            }
        }
        Err(e) => error!("Failed to read directory {}: {}", path.display(), e),
    }
}

pub fn spawn_log_cleaner(max_age_days: i64) {
    tokio::spawn(async move {
        loop {
            info!("Running log cleaner for: {}",&LOG_FILE_DIRS);
            clean_old_logs(&LOG_FILE_DIRS,max_age_days);
            sleep(StdDuration::from_secs(60 * 60 * 24)).await;
        }
    });
}