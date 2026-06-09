//! Wait system - smart waiting for elements, URLs, network idle, and page load
//!
//! The wait system is a core differentiator of rpage: operations automatically
//! wait for their preconditions (element visible, clickable, etc.) before
//! proceeding, reducing flakiness without explicit sleeps.

use std::time::{Duration, Instant};

use tracing::{debug, trace};

use crate::error::{Error, Result};

/// Default timeout for wait operations
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

/// Default polling interval
pub const DEFAULT_POLL_INTERVAL: Duration = Duration::from_millis(200);

/// Wait condition builder
#[derive(Debug, Clone)]
pub struct WaitOptions {
    /// Maximum time to wait
    pub timeout: Duration,
    /// Interval between condition checks
    pub poll_interval: Duration,
}

impl Default for WaitOptions {
    fn default() -> Self {
        Self {
            timeout: DEFAULT_TIMEOUT,
            poll_interval: DEFAULT_POLL_INTERVAL,
        }
    }
}

impl WaitOptions {
    /// Create with custom timeout
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Create with custom poll interval
    pub fn poll_interval(mut self, interval: Duration) -> Self {
        self.poll_interval = interval;
        self
    }
}

/// Generic async retry loop: poll `check_fn` until it returns `Ok(true)`,
/// `Err` is treated as "not yet" until timeout, then propagated.
pub async fn retry_until<F, Fut>(check_fn: F, opts: &WaitOptions, label: &str) -> Result<()>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<bool>>,
{
    let start = Instant::now();
    debug!("Waiting for: {label} (timeout: {:?})", opts.timeout);

    loop {
        match check_fn().await {
            Ok(true) => {
                debug!("Condition met: {label} ({:?} elapsed)", start.elapsed());
                return Ok(());
            }
            Ok(false) | Err(_) => {
                if start.elapsed() >= opts.timeout {
                    return Err(Error::Timeout(format!(
                        "Timed out waiting for: {label} ({:?})",
                        opts.timeout
                    )));
                }
                trace!("Not yet: {label}, polling in {:?}", opts.poll_interval);
                tokio::time::sleep(opts.poll_interval).await;
            }
        }
    }
}

/// Sleep for a specified duration (convenience wrapper)
pub async fn sleep(duration: Duration) {
    tokio::time::sleep(duration).await;
}

/// Wait for a sync predicate to become true
pub async fn wait_sync<F>(predicate: F, opts: &WaitOptions, label: &str) -> Result<()>
where
    F: Fn() -> bool,
{
    let start = Instant::now();
    loop {
        if predicate() {
            return Ok(());
        }
        if start.elapsed() >= opts.timeout {
            return Err(Error::Timeout(format!("Timed out: {label}")));
        }
        tokio::time::sleep(opts.poll_interval).await;
    }
}

/// Wait for an element to appear, retrying up to `opts.timeout`
pub async fn wait_element<F, Fut>(
    find_fn: F,
    locator: &str,
    opts: &WaitOptions,
) -> Result<crate::element::Element>
where
    F: Fn(&str) -> Fut,
    Fut: std::future::Future<Output = Result<crate::element::Element>>,
{
    let start = Instant::now();
    loop {
        match find_fn(locator).await {
            Ok(el) => return Ok(el),
            Err(_) => {
                if start.elapsed() >= opts.timeout {
                    return Err(Error::Timeout(format!(
                        "Timed out waiting for element: {locator}"
                    )));
                }
                tokio::time::sleep(opts.poll_interval).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wait_options_default() {
        let opts = WaitOptions::default();
        assert_eq!(opts.timeout, DEFAULT_TIMEOUT);
        assert_eq!(opts.poll_interval, DEFAULT_POLL_INTERVAL);
    }

    #[test]
    fn test_wait_options_builder() {
        let opts = WaitOptions::default()
            .timeout(Duration::from_secs(30))
            .poll_interval(Duration::from_millis(500));
        assert_eq!(opts.timeout, Duration::from_secs(30));
        assert_eq!(opts.poll_interval, Duration::from_millis(500));
    }

    #[tokio::test]
    async fn test_wait_sync_immediate() {
        let opts = WaitOptions::default();
        let result = wait_sync(|| true, &opts, "immediate").await;
        assert!(result.is_ok());
    }
}
