use std::path::PathBuf;
use std::sync::OnceLock;
use directories::ProjectDirs;
use chrono::Local;

static LOGGER: OnceLock<std::sync::Mutex<()>> = OnceLock::new();

pub fn get_log_path() -> PathBuf {
    ProjectDirs::from("com", "xylem", "xylem")
        .map(|d| d.data_dir().join("xylem.log"))
        .unwrap_or_else(|| PathBuf::from("xylem.log"))
}

pub fn log(level: &str, msg: &str) {
    let _mutex = LOGGER.get_or_init(|| std::sync::Mutex::new(())).lock().ok();

    let path = get_log_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    let entry = format!("[{}] {} {}\n", timestamp, level.to_uppercase(), msg);

    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .and_then(|mut f| std::io::Write::write_all(&mut f, entry.as_bytes()));
}

pub fn info(msg: &str) { log("info", msg); }
pub fn error(msg: &str) { log("error", msg); }
pub fn warn(msg: &str) { log("warn", msg); }