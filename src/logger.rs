use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::{Mutex, OnceLock};

static LOG_FILE: OnceLock<Mutex<Option<File>>> = OnceLock::new();

fn file() -> &'static Mutex<Option<File>> {
    LOG_FILE.get_or_init(|| {
        let path =
            crate::config::data_dir().join(format!("openxr_simulator.{}.log", std::process::id()));
        let file = if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
            OpenOptions::new().create(true).append(true).open(path).ok()
        } else {
            None
        };
        Mutex::new(file)
    })
}

pub fn write(prefix: &str, message: std::fmt::Arguments<'_>) {
    let line = format!(
        "[{:?}] [{prefix}] {message}\n",
        std::time::SystemTime::now()
    );
    if let Ok(mut guard) = file().lock()
        && let Some(file) = guard.as_mut()
    {
        let _ = file.write_all(line.as_bytes());
        let _ = file.flush();
    }
}

pub fn info(message: std::fmt::Arguments<'_>) {
    write("simxr", message);
}

pub fn error(message: std::fmt::Arguments<'_>) {
    write("simxr:error", message);
}
