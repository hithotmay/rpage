//! Network - request interception, monitoring, and header management
//!
//! Provides network-level control in Chromium mode:
//! - Request/response interception
//! - Header overrides
//! - Request logging
//! - Response filtering

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use chromiumoxide::cdp::browser_protocol::network::{
    EnableParams, Headers, SetExtraHttpHeadersParams,
};
use chromiumoxide::Page;

use crate::error::{Error, Result};

/// A recorded HTTP request
#[derive(Debug, Clone)]
pub struct RequestRecord {
    pub request_id: String,
    pub url: String,
    pub method: String,
    pub headers: HashMap<String, String>,
    pub resource_type: String,
}

/// A recorded HTTP response
#[derive(Debug, Clone)]
pub struct ResponseRecord {
    pub request_id: String,
    pub url: String,
    pub status: u16,
    pub headers: HashMap<String, String>,
    pub mime_type: String,
}

/// A failed request record
#[derive(Debug, Clone)]
pub struct FailedRequest {
    pub request_id: String,
    pub url: String,
    pub error_text: String,
}

/// Network monitor that records all requests and responses
#[derive(Debug, Clone, Default)]
pub struct NetworkMonitor {
    requests: Arc<Mutex<Vec<RequestRecord>>>,
    responses: Arc<Mutex<Vec<ResponseRecord>>>,
    failures: Arc<Mutex<Vec<FailedRequest>>>,
}

impl NetworkMonitor {
    /// Create a new empty network monitor
    pub fn new() -> Self {
        Self {
            requests: Arc::new(Mutex::new(Vec::new())),
            responses: Arc::new(Mutex::new(Vec::new())),
            failures: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Record an outgoing request
    pub fn record_request(&self, req: RequestRecord) {
        if let Ok(mut list) = self.requests.lock() {
            list.push(req);
        }
    }

    /// Record a received response
    pub fn record_response(&self, resp: ResponseRecord) {
        if let Ok(mut list) = self.responses.lock() {
            list.push(resp);
        }
    }

    /// Record a failed request
    pub fn record_failure(&self, fail: FailedRequest) {
        if let Ok(mut list) = self.failures.lock() {
            list.push(fail);
        }
    }

    /// Get all recorded requests
    pub fn requests(&self) -> Vec<RequestRecord> {
        self.requests.lock().map(|l| l.clone()).unwrap_or_default()
    }

    /// Get all recorded responses
    pub fn responses(&self) -> Vec<ResponseRecord> {
        self.responses.lock().map(|l| l.clone()).unwrap_or_default()
    }

    /// Get all failed requests
    pub fn failures(&self) -> Vec<FailedRequest> {
        self.failures.lock().map(|l| l.clone()).unwrap_or_default()
    }

    /// Clear all records
    pub fn clear(&self) {
        if let Ok(mut l) = self.requests.lock() {
            l.clear();
        }
        if let Ok(mut l) = self.responses.lock() {
            l.clear();
        }
        if let Ok(mut l) = self.failures.lock() {
            l.clear();
        }
    }

    /// Find all requests matching a URL pattern
    pub fn find_requests_by_url(&self, pattern: &str) -> Vec<RequestRecord> {
        self.requests()
            .into_iter()
            .filter(|r| r.url.contains(pattern))
            .collect()
    }

    /// Find all responses matching a URL pattern
    pub fn find_responses_by_url(&self, pattern: &str) -> Vec<ResponseRecord> {
        self.responses()
            .into_iter()
            .filter(|r| r.url.contains(pattern))
            .collect()
    }
}

/// Set extra HTTP headers on a Chromium page
pub async fn set_extra_headers(page: &Page, headers: HashMap<String, String>) -> Result<()> {
    let mut map = serde_json::Map::new();
    for (k, v) in headers {
        map.insert(k, serde_json::Value::String(v));
    }
    let headers_val = Headers::new(serde_json::Value::Object(map));
    let params = SetExtraHttpHeadersParams::new(headers_val);
    page.execute(params)
        .await
        .map_err(|e| Error::Browser(format!("set extra headers: {e}")))?;
    Ok(())
}

/// Enable network monitoring on a Chromium page
pub async fn enable_network(page: &Page) -> Result<()> {
    page.execute(EnableParams::default())
        .await
        .map_err(|e| Error::Browser(format!("enable network: {e}")))?;
    Ok(())
}

/// Set a custom User-Agent via CDP
pub async fn set_user_agent(page: &Page, user_agent: &str) -> Result<()> {
    use chromiumoxide::cdp::browser_protocol::network::SetUserAgentOverrideParams;
    let params = SetUserAgentOverrideParams::new(user_agent);
    page.execute(params)
        .await
        .map_err(|e| Error::Browser(format!("set user agent: {e}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monitor_record_and_query() {
        let monitor = NetworkMonitor::new();

        monitor.record_request(RequestRecord {
            request_id: "1".into(),
            url: "https://example.com/api/data".into(),
            method: "GET".into(),
            headers: HashMap::new(),
            resource_type: "XHR".into(),
        });

        monitor.record_request(RequestRecord {
            request_id: "2".into(),
            url: "https://example.com/page".into(),
            method: "GET".into(),
            headers: HashMap::new(),
            resource_type: "Document".into(),
        });

        monitor.record_response(ResponseRecord {
            request_id: "1".into(),
            url: "https://example.com/api/data".into(),
            status: 200,
            headers: HashMap::new(),
            mime_type: "application/json".into(),
        });

        assert_eq!(monitor.requests().len(), 2);
        assert_eq!(monitor.responses().len(), 1);
        assert_eq!(monitor.find_requests_by_url("/api/").len(), 1);

        monitor.clear();
        assert!(monitor.requests().is_empty());
    }
}
