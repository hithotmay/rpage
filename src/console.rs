//! Console log and JS exception capture via CDP Runtime events.
//!
//! Provides:
//! - `ConsoleMonitor` — records console.log/warn/error/info/debug entries and JS exceptions
//! - `ConsoleLevel` — log level enum
//! - `ConsoleEntry` — a single console log entry
//! - `JsException` — a captured JS exception

use std::sync::{Arc, Mutex};

/// Console log level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleLevel {
    Log,
    Warn,
    Error,
    Info,
    Debug,
    Other,
}

impl std::fmt::Display for ConsoleLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConsoleLevel::Log => write!(f, "log"),
            ConsoleLevel::Warn => write!(f, "warn"),
            ConsoleLevel::Error => write!(f, "error"),
            ConsoleLevel::Info => write!(f, "info"),
            ConsoleLevel::Debug => write!(f, "debug"),
            ConsoleLevel::Other => write!(f, "other"),
        }
    }
}

/// A single console entry captured from `Runtime.consoleAPICalled`.
#[derive(Debug, Clone)]
pub struct ConsoleEntry {
    /// Log level.
    pub level: ConsoleLevel,
    /// Concatenated text of all arguments.
    pub text: String,
    /// Timestamp (CDP epoch milliseconds).
    pub timestamp: f64,
}

/// A captured JS exception from `Runtime.exceptionThrown`.
#[derive(Debug, Clone)]
pub struct JsException {
    /// Exception text.
    pub text: String,
    /// Source URL (if available).
    pub url: Option<String>,
    /// Line number (0-based).
    pub line: i64,
    /// Column number (0-based).
    pub column: i64,
    /// Formatted stack trace string (if available).
    pub stack_trace: Option<String>,
    /// Timestamp (CDP epoch milliseconds).
    pub timestamp: f64,
}

/// Thread-safe monitor that stores captured console entries and JS exceptions.
#[derive(Debug, Clone, Default)]
pub struct ConsoleMonitor {
    logs: Arc<Mutex<Vec<ConsoleEntry>>>,
    exceptions: Arc<Mutex<Vec<JsException>>>,
}

impl ConsoleMonitor {
    /// Create a new empty monitor.
    pub fn new() -> Self {
        Self {
            logs: Arc::new(Mutex::new(Vec::new())),
            exceptions: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Record a console log entry.
    pub fn add_log(&self, entry: ConsoleEntry) {
        if let Ok(mut list) = self.logs.lock() {
            list.push(entry);
        }
    }

    /// Record a JS exception.
    pub fn add_exception(&self, exc: JsException) {
        if let Ok(mut list) = self.exceptions.lock() {
            list.push(exc);
        }
    }

    /// Get all captured console log entries.
    pub fn logs(&self) -> Vec<ConsoleEntry> {
        self.logs.lock().map(|l| l.clone()).unwrap_or_default()
    }

    /// Get all captured JS exceptions.
    pub fn exceptions(&self) -> Vec<JsException> {
        self.exceptions
            .lock()
            .map(|l| l.clone())
            .unwrap_or_default()
    }

    /// Clear all stored console entries and exceptions.
    pub fn clear(&self) {
        if let Ok(mut l) = self.logs.lock() {
            l.clear();
        }
        if let Ok(mut l) = self.exceptions.lock() {
            l.clear();
        }
    }
}

/// Enable the Runtime domain on a page so console/exception events fire.
pub async fn enable_runtime(page: &chromiumoxide::Page) -> crate::error::Result<()> {
    use chromiumoxide::cdp::js_protocol::runtime::EnableParams;
    page.execute(EnableParams::default())
        .await
        .map_err(|e| crate::error::Error::Browser(format!("enable runtime: {e}")))?;
    Ok(())
}

/// Convert a CDP `ConsoleApiCalledType` to our `ConsoleLevel`.
pub fn cdp_type_to_level(
    ty: &chromiumoxide::cdp::js_protocol::runtime::ConsoleApiCalledType,
) -> ConsoleLevel {
    use chromiumoxide::cdp::js_protocol::runtime::ConsoleApiCalledType;
    match ty {
        ConsoleApiCalledType::Log => ConsoleLevel::Log,
        ConsoleApiCalledType::Debug => ConsoleLevel::Debug,
        ConsoleApiCalledType::Info => ConsoleLevel::Info,
        ConsoleApiCalledType::Error => ConsoleLevel::Error,
        ConsoleApiCalledType::Warning => ConsoleLevel::Warn,
        _ => ConsoleLevel::Other,
    }
}

/// Format a CDP `StackTrace` into a human-readable string.
pub fn format_stack_trace(st: &chromiumoxide::cdp::js_protocol::runtime::StackTrace) -> String {
    let mut lines = Vec::new();
    for frame in &st.call_frames {
        lines.push(format!(
            "    at {} ({}:{}:{})",
            if frame.function_name.is_empty() {
                "<anonymous>"
            } else {
                &frame.function_name
            },
            if frame.url.is_empty() {
                "<unknown>"
            } else {
                &frame.url
            },
            frame.line_number + 1,
            frame.column_number + 1,
        ));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitor_add_and_query() {
        let monitor = ConsoleMonitor::new();

        monitor.add_log(ConsoleEntry {
            level: ConsoleLevel::Log,
            text: "hello".into(),
            timestamp: 1000.0,
        });
        monitor.add_log(ConsoleEntry {
            level: ConsoleLevel::Error,
            text: "oops".into(),
            timestamp: 1001.0,
        });
        monitor.add_exception(JsException {
            text: "Uncaught TypeError".into(),
            url: Some("https://example.com/app.js".into()),
            line: 42,
            column: 5,
            stack_trace: None,
            timestamp: 1002.0,
        });

        assert_eq!(monitor.logs().len(), 2);
        assert_eq!(monitor.exceptions().len(), 1);
        assert_eq!(monitor.exceptions()[0].line, 42);

        monitor.clear();
        assert!(monitor.logs().is_empty());
        assert!(monitor.exceptions().is_empty());
    }
}
