//! WebSocket frame monitoring via CDP.
//!
//! Captures WebSocket frames sent/received by the browser using
//! `Network.webSocketFrame*` CDP events. Requires `Network.enable`
//! (already called during `ChromiumPage::connect()`).

use std::sync::{Arc, Mutex};

/// A captured WebSocket frame (sent or received).
#[derive(Debug, Clone)]
pub struct WsFrame {
    /// CDP request identifier for the WebSocket connection.
    pub request_id: String,
    /// Monotonic timestamp from CDP.
    pub timestamp: f64,
    /// WebSocket opcode as a string (e.g. "1" = text, "2" = binary).
    pub opcode: String,
    /// Frame payload. For text frames (opcode 1) this is UTF-8 text;
    /// for binary frames it is base64-encoded.
    pub payload: String,
    /// `true` if the frame was sent by the client, `false` if received.
    pub is_sent: bool,
}

/// A WebSocket lifecycle event (created / closed / error).
#[derive(Debug, Clone)]
pub enum WsEvent {
    /// A new WebSocket connection was established.
    Created { request_id: String, url: String },
    /// A WebSocket connection was closed.
    Closed { request_id: String, timestamp: f64 },
    /// A WebSocket frame error occurred.
    Error {
        request_id: String,
        timestamp: f64,
        error_message: String,
    },
}

/// Thread-safe monitor that accumulates WebSocket frames and events.
#[derive(Debug, Default)]
pub struct WebSocketMonitor {
    frames: Arc<Mutex<Vec<WsFrame>>>,
    events: Arc<Mutex<Vec<WsEvent>>>,
}

impl WebSocketMonitor {
    /// Create a new empty monitor.
    pub fn new() -> Self {
        Self {
            frames: Arc::new(Mutex::new(Vec::new())),
            events: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Record a captured frame.
    pub fn add_frame(&self, frame: WsFrame) {
        if let Ok(mut list) = self.frames.lock() {
            list.push(frame);
        }
    }

    /// Record a lifecycle event.
    pub fn add_event(&self, event: WsEvent) {
        if let Ok(mut list) = self.events.lock() {
            list.push(event);
        }
    }

    /// Return all captured frames (cloned).
    pub fn frames(&self) -> Vec<WsFrame> {
        self.frames.lock().map(|l| l.clone()).unwrap_or_default()
    }

    /// Return all lifecycle events (cloned).
    pub fn events(&self) -> Vec<WsEvent> {
        self.events.lock().map(|l| l.clone()).unwrap_or_default()
    }

    /// Clear all captured frames and events.
    pub fn clear(&self) {
        if let Ok(mut l) = self.frames.lock() {
            l.clear();
        }
        if let Ok(mut l) = self.events.lock() {
            l.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitor_record_and_query() {
        let monitor = WebSocketMonitor::new();

        monitor.add_frame(WsFrame {
            request_id: "ws-1".into(),
            timestamp: 1.0,
            opcode: "1".into(),
            payload: "hello".into(),
            is_sent: true,
        });
        monitor.add_frame(WsFrame {
            request_id: "ws-1".into(),
            timestamp: 2.0,
            opcode: "1".into(),
            payload: "world".into(),
            is_sent: false,
        });

        monitor.add_event(WsEvent::Created {
            request_id: "ws-1".into(),
            url: "wss://example.com".into(),
        });
        monitor.add_event(WsEvent::Closed {
            request_id: "ws-1".into(),
            timestamp: 3.0,
        });

        assert_eq!(monitor.frames().len(), 2);
        assert_eq!(monitor.events().len(), 2);

        assert!(monitor.frames().iter().any(|f| f.is_sent));
        assert!(monitor.frames().iter().any(|f| !f.is_sent));

        monitor.clear();
        assert!(monitor.frames().is_empty());
        assert!(monitor.events().is_empty());
    }
}
