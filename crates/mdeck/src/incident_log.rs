use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{Instant, SystemTime};

struct Inner {
    path: PathBuf,
    presentation_file: String,
    session_start: Instant,
    count: usize,
    file: Option<std::fs::File>,
}

pub struct IncidentLog {
    inner: Mutex<Inner>,
}

impl IncidentLog {
    pub fn new(presentation_file: &str) -> Self {
        let path = log_dir().join(format!(
            "incident-{}.log",
            format_timestamp(SystemTime::now())
        ));
        Self {
            inner: Mutex::new(Inner {
                path,
                presentation_file: presentation_file.to_string(),
                session_start: Instant::now(),
                count: 0,
                file: None,
            }),
        }
    }

    pub fn record(&self, category: &str, summary: &str, detail: &str) {
        let mut inner = self.inner.lock().unwrap();

        // Lazy file creation on first incident
        if inner.file.is_none() {
            if let Some(parent) = inner.path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            match std::fs::File::create(&inner.path) {
                Ok(mut f) => {
                    let _ = write_header(&mut f, &inner.presentation_file);
                    inner.file = Some(f);
                }
                Err(e) => {
                    eprintln!("Warning: could not create incident log: {e}");
                    return;
                }
            }
        }

        inner.count += 1;
        let num = inner.count;
        let elapsed = inner.session_start.elapsed().as_secs_f64();

        if let Some(ref mut f) = inner.file {
            let _ = writeln!(f, "\n--- incident #{num} ---");
            let _ = writeln!(f, "time: +{elapsed:.1}s");
            let _ = writeln!(f, "category: {category}");
            let _ = writeln!(f, "summary: {summary}");
            if !detail.is_empty() {
                let _ = writeln!(f, "detail: |");
                for line in detail.lines() {
                    let _ = writeln!(f, "  {line}");
                }
            }
            let _ = f.flush();
        }
    }

    pub fn summary(&self) -> Option<(PathBuf, usize)> {
        let inner = self.inner.lock().unwrap();
        if inner.count > 0 {
            Some((inner.path.clone(), inner.count))
        } else {
            None
        }
    }

    #[cfg(test)]
    pub fn count(&self) -> usize {
        self.inner.lock().unwrap().count
    }
}

fn write_header(f: &mut std::fs::File, presentation_file: &str) -> std::io::Result<()> {
    writeln!(
        f,
        "# mdeck incident log\n\
         # Share this file when reporting issues at https://github.com/mklab-se/mdeck/issues\n"
    )?;
    writeln!(f, "version: {}", env!("CARGO_PKG_VERSION"))?;
    writeln!(f, "file: {presentation_file}")?;
    writeln!(f, "os: {} {}", std::env::consts::OS, std::env::consts::ARCH)?;

    // Include display-related env vars when present (useful for Linux diagnostics)
    for var in [
        "DISPLAY",
        "WAYLAND_DISPLAY",
        "XDG_SESSION_TYPE",
        "XDG_CURRENT_DESKTOP",
        "DESKTOP_SESSION",
    ] {
        if let Ok(val) = std::env::var(var) {
            writeln!(f, "env.{var}: {val}")?;
        }
    }

    Ok(())
}

fn log_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("mdeck")
        .join("logs")
}

/// Format a SystemTime as `YYYY-MM-DD-HHMMSS` without external dependencies.
fn format_timestamp(time: SystemTime) -> String {
    let dur = time
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = dur.as_secs();

    let secs_in_day = total_secs % 86400;
    let hours = secs_in_day / 3600;
    let minutes = (secs_in_day % 3600) / 60;
    let seconds = secs_in_day % 60;

    let days = (total_secs / 86400) as i64;
    let (year, month, day) = days_to_ymd(days);

    format!("{year:04}-{month:02}-{day:02}-{hours:02}{minutes:02}{seconds:02}")
}

/// Convert days since Unix epoch to (year, month, day).
fn days_to_ymd(days_since_epoch: i64) -> (i64, u32, u32) {
    // Algorithm from http://howardhinnant.github.io/date_algorithms.html
    let z = days_since_epoch + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn no_incident_no_file() {
        let log = IncidentLog::new("/tmp/test.md");
        assert_eq!(log.count(), 0);
        assert!(log.summary().is_none());
        // The log file path should not exist
        let path = log.inner.lock().unwrap().path.clone();
        assert!(!path.exists());
    }

    #[test]
    fn file_created_on_first_record() {
        let dir = std::env::temp_dir().join(format!("mdeck-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);

        let log = IncidentLog::new("/tmp/test.md");
        // Override the path to use our temp dir
        log.inner.lock().unwrap().path = dir.join("test-incident.log");

        log.record("test_category", "test summary", "test detail");

        let path = log.inner.lock().unwrap().path.clone();
        assert!(path.exists());
        assert_eq!(log.count(), 1);

        let mut content = String::new();
        std::fs::File::open(&path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        assert!(content.contains("incident #1"));
        assert!(content.contains("category: test_category"));
        assert!(content.contains("summary: test summary"));

        // Cleanup
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn multiple_incidents() {
        let dir = std::env::temp_dir().join(format!("mdeck-test-multi-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);

        let log = IncidentLog::new("/tmp/test.md");
        log.inner.lock().unwrap().path = dir.join("test-incident.log");

        log.record("cat_a", "first", "");
        log.record("cat_b", "second", "some detail");
        log.record("cat_c", "third", "line1\nline2");

        assert_eq!(log.count(), 3);
        let (path, count) = log.summary().unwrap();
        assert_eq!(count, 3);

        let mut content = String::new();
        std::fs::File::open(&path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        assert!(content.contains("incident #1"));
        assert!(content.contains("incident #2"));
        assert!(content.contains("incident #3"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn header_contains_version() {
        let dir = std::env::temp_dir().join(format!("mdeck-test-hdr-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);

        let log = IncidentLog::new("/home/user/talks/demo.md");
        log.inner.lock().unwrap().path = dir.join("test-incident.log");

        log.record("test", "trigger header", "");

        let path = log.inner.lock().unwrap().path.clone();
        let mut content = String::new();
        std::fs::File::open(&path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        assert!(content.contains(&format!("version: {}", env!("CARGO_PKG_VERSION"))));
        assert!(content.contains("file: /home/user/talks/demo.md"));
        assert!(content.contains("# mdeck incident log"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn timestamp_format() {
        // 2024-01-15 12:10:45 UTC
        let dur = std::time::Duration::from_secs(1705320645);
        let time = SystemTime::UNIX_EPOCH + dur;
        let ts = format_timestamp(time);
        assert_eq!(ts, "2024-01-15-121045");
    }

    #[test]
    fn days_to_ymd_epoch() {
        assert_eq!(days_to_ymd(0), (1970, 1, 1));
    }

    #[test]
    fn days_to_ymd_known_date() {
        // 2024-01-15 is day 19737 since epoch
        assert_eq!(days_to_ymd(19737), (2024, 1, 15));
    }
}
