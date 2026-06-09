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
