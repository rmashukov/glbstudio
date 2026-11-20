//! Logging is installed before macroquad creates a window or graphics context.
use std::{
    backtrace::Backtrace,
    fs::{self, File, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{Mutex, OnceLock},
    time::{Instant, SystemTime, UNIX_EPOCH},
};

struct FileLogger {
    file: Mutex<File>,
    started: Instant,
}

static LOGGER: OnceLock<FileLogger> = OnceLock::new();
static LOG_PATH: OnceLock<Option<PathBuf>> = OnceLock::new();

impl log::Log for FileLogger {
    fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
        metadata.level() <= log::Level::Info
    }

    fn log(&self, record: &log::Record<'_>) {
        if self.enabled(record.metadata())
            && let Ok(mut file) = self.file.lock()
        {
            let _ = writeln!(
                file,
                "[+{:>9.3}s] {:5} [{}] {}",
                self.started.elapsed().as_secs_f64(),
                record.level(),
                record.target(),
                record.args()
            );
            // Preserve the last completed startup/frame stage on a native crash.
            let _ = file.flush();
        }
    }

    fn flush(&self) {
        if let Ok(mut file) = self.file.lock() {
            let _ = file.flush();
        }
    }
}

fn log_directory() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("GLB_STUDIO_LOG_DIR") {
        return Some(PathBuf::from(path));
    }
    if cfg!(target_os = "windows") {
        std::env::var_os("LOCALAPPDATA").map(|path| PathBuf::from(path).join("GLB Studio/Logs"))
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|path| PathBuf::from(path).join("Library/Logs/GLB Studio"))
    } else {
        std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".local/state")))
            .map(|path| path.join("glb-studio"))
    }
}

fn open_log(directory: PathBuf) -> std::io::Result<(PathBuf, File)> {
    fs::create_dir_all(&directory)?;
    let path = directory.join("glb-studio.log");
    if fs::metadata(&path).is_ok_and(|meta| meta.len() > 2 * 1024 * 1024) {
        let previous = directory.join("glb-studio.previous.log");
        // Preserve recent sessions without accumulating unbounded history.
        fs::copy(&path, previous)?;
        File::create(&path)?;
    }
    let file = OpenOptions::new().create(true).append(true).open(&path)?;
    Ok((path, file))
}

pub fn path() -> Option<&'static std::path::Path> {
    LOG_PATH.get().and_then(|path| path.as_deref())
}

pub fn init() {
    LOG_PATH.get_or_init(|| {
        let fallback = std::env::temp_dir().join("GLB Studio");
        let log = open_log(log_directory().unwrap_or_else(|| fallback.clone()))
            .or_else(|_| open_log(fallback));
        let (path, mut file) = match log {
            Ok(log) => log,
            Err(error) => {
                eprintln!("Could not create GLB Studio log: {error}");
                return None;
            }
        };
        let unix_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let _ = writeln!(
            file,
            "\n=== GLB Studio session: unix={unix_time} pid={} ===",
            std::process::id()
        );

        // Capture native graphics/Objective-C diagnostics too, including errors
        // before the async application main function gets its first frame.
        #[cfg(unix)]
        {
            use std::os::fd::AsRawFd;
            // SAFETY: dup2 duplicates an open file descriptor into standard streams.
            // The logger keeps the original File alive for the entire process.
            unsafe {
                if libc::dup2(file.as_raw_fd(), libc::STDOUT_FILENO) == -1
                    || libc::dup2(file.as_raw_fd(), libc::STDERR_FILENO) == -1
                {
                    let _ = writeln!(
                        file,
                        "Native output redirection failed: {}",
                        std::io::Error::last_os_error()
                    );
                }
            }
        }
        let logger = LOGGER.get_or_init(|| FileLogger {
            file: Mutex::new(file),
            started: Instant::now(),
        });
        if log::set_logger(logger).is_ok() {
            log::set_max_level(log::LevelFilter::Info);
        }
        Some(path)
    });

    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        log::error!(target: "panic", "{info}\n{}", Backtrace::force_capture());
        log::logger().flush();
        if LOGGER.get().is_none() {
            previous_hook(info);
        }
    }));
    log::info!(
        "GLB Studio {} build={} os={} arch={} log={:?}",
        env!("CARGO_PKG_VERSION"),
        option_env!("GLB_STUDIO_BUILD_ID").unwrap_or("local"),
        std::env::consts::OS,
        std::env::consts::ARCH,
        path()
    );
    #[cfg(target_os = "macos")]
    if let Ok(version) = std::process::Command::new("/usr/bin/sw_vers")
        .arg("-productVersion")
        .output()
    {
        log::info!("macOS {}", String::from_utf8_lossy(&version.stdout).trim());
    }
}
