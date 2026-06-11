//! CookieHub - shared cookie store for syncing between Chromium and Session modes

use std::sync::Arc;

use cookie_store::CookieStore;
use tracing::debug;

use crate::error::{Error, Result};

/// Thread-safe shared cookie store.
///
/// Allows cookies to flow bidirectionally between ChromiumPage (browser)
/// and SessionPage (HTTP client).
pub struct CookieHub {
    store: Arc<reqwest_cookie_store::CookieStoreMutex>,
}

impl CookieHub {
    /// Create an empty cookie hub
    pub fn new() -> Self {
        Self {
            store: Arc::new(reqwest_cookie_store::CookieStoreMutex::new(
                CookieStore::default(),
            )),
        }
    }

    /// Get a reference to the underlying cookie store mutex
    pub fn store(&self) -> Arc<reqwest_cookie_store::CookieStoreMutex> {
        self.store.clone()
    }

    /// Get all cookies for a given URL
    pub fn get_cookies(&self, url: &str) -> Result<Vec<cookie_store::Cookie<'static>>> {
        let url = url::Url::parse(url)?;
        let store = self
            .store
            .lock()
            .map_err(|e| Error::CookieSync(format!("cookie store lock poisoned: {e}")))?;
        Ok(store.matches(&url).into_iter().cloned().collect())
    }

    /// Set a cookie from parsed cookie + url
    pub fn set_cookie(&self, cookie: cookie_store::Cookie<'static>, url: &url::Url) -> Result<()> {
        let mut store = self
            .store
            .lock()
            .map_err(|e| Error::CookieSync(format!("cookie store lock poisoned: {e}")))?;
        store
            .insert(cookie, url)
            .map_err(|e| Error::CookieSync(format!("insert cookie: {e}")))?;
        Ok(())
    }

    /// Set a cookie from raw header value
    pub fn set_cookie_raw(&self, cookie_str: &str, url: &str) -> Result<()> {
        let url = url::Url::parse(url)?;
        let mut store = self
            .store
            .lock()
            .map_err(|e| Error::CookieSync(format!("cookie store lock poisoned: {e}")))?;
        let cookie = cookie_store::Cookie::parse(cookie_str.to_string(), &url)
            .map_err(|e| Error::CookieSync(format!("parse cookie: {e}")))?;
        store
            .insert(cookie, &url)
            .map_err(|e| Error::CookieSync(format!("insert cookie: {e}")))?;
        Ok(())
    }

    /// Sync cookies from a ChromiumPage (extract from browser → inject into store)
    pub fn sync_from_chromium(&self, cookies: Vec<crate::chromium_page::CookieInfo>) -> Result<()> {
        let mut store = self
            .store
            .lock()
            .map_err(|e| Error::CookieSync(format!("cookie store lock poisoned: {e}")))?;

        for c in cookies {
            let domain = c.domain.clone().unwrap_or_default();
            let url_str = if let Some(stripped) = domain.strip_prefix('.') {
                format!("https://{stripped}")
            } else {
                format!("https://{domain}")
            };
            if let Ok(url) = url::Url::parse(&url_str) {
                let cookie_str = format!(
                    "{}={}; Domain={}; Path={}",
                    c.name,
                    c.value,
                    domain,
                    c.path.clone().unwrap_or_else(|| "/".to_string())
                );
                if let Ok(parsed) = cookie_store::Cookie::parse(cookie_str, &url) {
                    store.insert(parsed, &url).ok();
                }
            }
        }

        debug!("Synced cookies from Chromium to store");
        Ok(())
    }

    /// Get cookie header string for a URL (for Session mode)
    pub fn cookie_header(&self, url: &str) -> Result<String> {
        let url = url::Url::parse(url)?;
        let store = self
            .store
            .lock()
            .map_err(|e| Error::CookieSync(format!("cookie store lock poisoned: {e}")))?;
        let cookies: Vec<String> = store
            .matches(&url)
            .into_iter()
            .map(|c| format!("{}={}", c.name(), c.value()))
            .collect();
        Ok(cookies.join("; "))
    }

    /// Clear all cookies
    pub fn clear(&self) -> Result<()> {
        let mut store = self
            .store
            .lock()
            .map_err(|e| Error::CookieSync(format!("cookie store lock poisoned: {e}")))?;
        store.clear();
        debug!("Cleared all cookies");
        Ok(())
    }

    /// Save all cookies to a JSON file.
    pub fn save_to_file(&self, path: &str) -> Result<()> {
        let store = self
            .store
            .lock()
            .map_err(|e| Error::CookieSync(format!("cookie store lock poisoned: {e}")))?;
        let cookies: Vec<serde_json::Value> = store
            .iter_unexpired()
            .map(|c| {
                let domain_str = match &c.domain {
                    cookie_store::CookieDomain::Suffix(d) => d.as_str(),
                    cookie_store::CookieDomain::HostOnly(d) => d.as_str(),
                    _ => "",
                };
                serde_json::json!({
                    "name": c.name(),
                    "value": c.value(),
                    "domain": domain_str,
                    "path": c.path.to_string(),
                })
            })
            .collect();
        let json = serde_json::to_string_pretty(&cookies)?;
        std::fs::write(path, json)?;
        debug!("Saved cookies to {path}");
        Ok(())
    }

    /// Load cookies from a JSON file.
    pub fn load_from_file(&self, path: &str) -> Result<()> {
        let json = std::fs::read_to_string(path)?;
        let cookies: Vec<serde_json::Value> = serde_json::from_str(&json)?;
        let mut store = self
            .store
            .lock()
            .map_err(|e| Error::CookieSync(format!("cookie store lock poisoned: {e}")))?;
        for c in cookies {
            let name = c["name"].as_str().unwrap_or("");
            let value = c["value"].as_str().unwrap_or("");
            let domain = c["domain"].as_str().unwrap_or("");
            let cpath = c["path"].as_str().unwrap_or("/");
            let cookie_str = format!("{name}={value}; Domain={domain}; Path={cpath}");
            let url = url::Url::parse(&format!("https://{domain}"))
                .unwrap_or_else(|_| url::Url::parse("https://example.com").unwrap());
            if let Ok(parsed) = cookie_store::Cookie::parse(cookie_str, &url) {
                let _ = store.insert(parsed, &url);
            }
        }
        debug!("Loaded cookies from {path}");
        Ok(())
    }
}

impl std::fmt::Debug for CookieHub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CookieHub").finish()
    }
}

impl Default for CookieHub {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cookie_hub_new() {
        let hub = CookieHub::new();
        // CookieHub should be constructable without error
        let _ = &hub;
    }

    #[test]
    fn test_cookie_hub_default() {
        let hub = CookieHub::default();
        let _ = &hub;
    }

    #[test]
    fn test_cookie_hub_clear_empty() {
        let hub = CookieHub::new();
        let result = hub.clear();
        assert!(result.is_ok());
    }

    #[test]
    fn test_cookie_hub_cookie_header_empty() {
        let hub = CookieHub::new();
        let result = hub.cookie_header("https://example.com");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "");
    }

    #[test]
    fn test_cookie_hub_get_cookies_empty() {
        let hub = CookieHub::new();
        let result = hub.get_cookies("https://example.com");
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_cookie_hub_set_cookie_raw() {
        let hub = CookieHub::new();
        let result = hub.set_cookie_raw("session=abc123", "https://example.com");
        assert!(result.is_ok());

        let cookies = hub.get_cookies("https://example.com").unwrap();
        assert_eq!(cookies.len(), 1);
        assert_eq!(cookies[0].name(), "session");
        assert_eq!(cookies[0].value(), "abc123");
    }

    #[test]
    fn test_cookie_hub_cookie_header_after_set() {
        let hub = CookieHub::new();
        hub.set_cookie_raw("token=xyz", "https://example.com").unwrap();
        let header = hub.cookie_header("https://example.com").unwrap();
        assert_eq!(header, "token=xyz");
    }

    #[test]
    fn test_cookie_hub_clear_removes_cookies() {
        let hub = CookieHub::new();
        hub.set_cookie_raw("foo=bar", "https://example.com").unwrap();
        assert!(!hub.get_cookies("https://example.com").unwrap().is_empty());

        hub.clear().unwrap();
        assert!(hub.get_cookies("https://example.com").unwrap().is_empty());
    }

    #[test]
    fn test_cookie_hub_debug() {
        let hub = CookieHub::new();
        let debug_str = format!("{:?}", hub);
        assert!(debug_str.contains("CookieHub"));
    }

    #[test]
    fn test_cookie_hub_invalid_url() {
        let hub = CookieHub::new();
        let result = hub.get_cookies("not-a-valid-url");
        assert!(result.is_err());
    }

    #[test]
    fn test_cookie_hub_store_is_clonable() {
        let hub = CookieHub::new();
        let store1 = hub.store();
        let store2 = hub.store();
        // Both are Arc clones pointing to the same store
        let _ = (store1, store2);
    }
}
