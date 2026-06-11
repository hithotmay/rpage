//! Configuration types for rpage

use std::path::PathBuf;
use std::time::Duration;

/// Default timeout in seconds
const DEFAULT_TIMEOUT_SECS: u64 = 10;

/// Viewport dimensions
#[derive(Debug, Clone)]
pub struct Viewport {
    pub width: u32,
    pub height: u32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            width: 1920,
            height: 1080,
        }
    }
}

/// Options for ChromiumPage (browser mode)
#[derive(Debug, Clone)]
pub struct ChromiumOptions {
    /// Global timeout for operations
    pub timeout: Duration,
    /// User-Agent string (empty = use browser default)
    pub user_agent: String,
    /// Viewport size
    pub viewport: Viewport,
    /// Run in headless mode
    pub headless: bool,
    /// Proxy URL, e.g. "http://127.0.0.1:7890"
    pub proxy: Option<String>,
    /// Proxy authentication credentials: (username, password)
    pub proxy_auth: Option<(String, String)>,
    /// Path to Chrome/Chromium binary
    pub browser_path: Option<PathBuf>,
    /// User data directory for persistent profiles
    pub user_data_dir: Option<PathBuf>,
    /// Extension directories to load
    pub extension_dirs: Vec<PathBuf>,
    /// Disable GPU
    pub disable_gpu: bool,
    /// Disable sandbox (needed in some CI)
    pub no_sandbox: bool,
    /// Additional Chrome arguments
    pub extra_args: Vec<String>,
    /// Debug port for CDP connection (default: 9222)
    pub debug_port: u16,
}

impl Default for ChromiumOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            user_agent: String::new(),
            viewport: Viewport::default(),
            headless: true,
            proxy: None,
            proxy_auth: None,
            browser_path: None,
            user_data_dir: None,
            extension_dirs: Vec::new(),
            disable_gpu: true,
            no_sandbox: false,
            extra_args: Vec::new(),
            debug_port: 9222,
        }
    }
}

impl ChromiumOptions {
    pub fn builder() -> ChromiumOptionsBuilder {
        ChromiumOptionsBuilder::default()
    }
}

/// Builder for ChromiumOptions
#[derive(Default)]
pub struct ChromiumOptionsBuilder {
    opts: ChromiumOptions,
}

impl ChromiumOptionsBuilder {
    pub fn timeout(mut self, d: Duration) -> Self {
        self.opts.timeout = d;
        self
    }

    pub fn user_agent(mut self, ua: impl Into<String>) -> Self {
        self.opts.user_agent = ua.into();
        self
    }

    pub fn viewport(mut self, w: u32, h: u32) -> Self {
        self.opts.viewport = Viewport {
            width: w,
            height: h,
        };
        self
    }

    pub fn headless(mut self, v: bool) -> Self {
        self.opts.headless = v;
        self
    }

    pub fn proxy(mut self, p: impl Into<String>) -> Self {
        self.opts.proxy = Some(p.into());
        self
    }

    pub fn proxy_auth(mut self, user: impl Into<String>, pass: impl Into<String>) -> Self {
        self.opts.proxy_auth = Some((user.into(), pass.into()));
        self
    }

    pub fn browser_path(mut self, p: impl Into<PathBuf>) -> Self {
        self.opts.browser_path = Some(p.into());
        self
    }

    pub fn user_data_dir(mut self, p: impl Into<PathBuf>) -> Self {
        self.opts.user_data_dir = Some(p.into());
        self
    }

    pub fn extension_dir(mut self, p: impl Into<PathBuf>) -> Self {
        self.opts.extension_dirs.push(p.into());
        self
    }

    pub fn no_sandbox(mut self, v: bool) -> Self {
        self.opts.no_sandbox = v;
        self
    }

    pub fn disable_gpu(mut self, v: bool) -> Self {
        self.opts.disable_gpu = v;
        self
    }

    pub fn arg(mut self, a: impl Into<String>) -> Self {
        self.opts.extra_args.push(a.into());
        self
    }

    pub fn build(self) -> ChromiumOptions {
        self.opts
    }
}

/// Options for SessionPage (HTTP mode)
#[derive(Debug, Clone)]
pub struct SessionOptions {
    /// Global timeout for HTTP requests
    pub timeout: Duration,
    /// User-Agent string
    pub user_agent: String,
    /// Proxy URL
    pub proxy: Option<String>,
    /// Accept invalid TLS certificates
    pub accept_invalid_certs: bool,
    /// Follow redirects
    pub follow_redirects: bool,
    /// Max redirects
    pub max_redirects: usize,
}

impl Default for SessionOptions {
    fn default() -> Self {
        Self {
            timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            user_agent: String::from(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 \
                 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
            ),
            proxy: None,
            accept_invalid_certs: false,
            follow_redirects: true,
            max_redirects: 10,
        }
    }
}

impl SessionOptions {
    pub fn builder() -> SessionOptionsBuilder {
        SessionOptionsBuilder::default()
    }
}

/// Builder for SessionOptions
#[derive(Default)]
pub struct SessionOptionsBuilder {
    opts: SessionOptions,
}

impl SessionOptionsBuilder {
    pub fn timeout(mut self, d: Duration) -> Self {
        self.opts.timeout = d;
        self
    }

    pub fn user_agent(mut self, ua: impl Into<String>) -> Self {
        self.opts.user_agent = ua.into();
        self
    }

    pub fn proxy(mut self, p: impl Into<String>) -> Self {
        self.opts.proxy = Some(p.into());
        self
    }

    pub fn accept_invalid_certs(mut self, v: bool) -> Self {
        self.opts.accept_invalid_certs = v;
        self
    }

    pub fn build(self) -> SessionOptions {
        self.opts
    }
}

/// Unified options for WebPage (wraps both Chromium and Session options)
#[derive(Debug, Clone)]
pub struct WebPageOptions {
    /// Chromium-specific options
    pub chromium: ChromiumOptions,
    /// Session-specific options
    pub session: SessionOptions,
    /// Initial mode (defaults to Chromium)
    pub initial_mode: crate::web_page::PageMode,
}

impl Default for WebPageOptions {
    fn default() -> Self {
        Self {
            chromium: ChromiumOptions::default(),
            session: SessionOptions::default(),
            initial_mode: crate::web_page::PageMode::Chromium,
        }
    }
}

impl WebPageOptions {
    pub fn builder() -> WebPageOptionsBuilder {
        WebPageOptionsBuilder::default()
    }
}

/// Builder for WebPageOptions
#[derive(Default)]
pub struct WebPageOptionsBuilder {
    opts: WebPageOptions,
}

impl WebPageOptionsBuilder {
    pub fn chromium(mut self, c: ChromiumOptions) -> Self {
        self.opts.chromium = c;
        self
    }

    pub fn session(mut self, s: SessionOptions) -> Self {
        self.opts.session = s;
        self
    }

    pub fn initial_mode(mut self, m: super::web_page::PageMode) -> Self {
        self.opts.initial_mode = m;
        self
    }

    pub fn build(self) -> WebPageOptions {
        self.opts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chromium_options_default_values() {
        let opts = ChromiumOptions::default();
        assert_eq!(opts.timeout, Duration::from_secs(10));
        assert!(opts.user_agent.is_empty());
        assert!(opts.headless);
        assert!(opts.proxy.is_none());
        assert!(opts.proxy_auth.is_none());
        assert!(opts.browser_path.is_none());
        assert!(opts.user_data_dir.is_none());
        assert!(opts.extension_dirs.is_empty());
        assert!(opts.disable_gpu);
        assert!(!opts.no_sandbox);
        assert!(opts.extra_args.is_empty());
        assert_eq!(opts.debug_port, 9222);
    }

    #[test]
    fn test_chromium_options_default_viewport() {
        let opts = ChromiumOptions::default();
        assert_eq!(opts.viewport.width, 1920);
        assert_eq!(opts.viewport.height, 1080);
    }

    #[test]
    fn test_viewport_default() {
        let vp = Viewport::default();
        assert_eq!(vp.width, 1920);
        assert_eq!(vp.height, 1080);
    }

    #[test]
    fn test_session_options_default_values() {
        let opts = SessionOptions::default();
        assert_eq!(opts.timeout, Duration::from_secs(10));
        assert!(opts.user_agent.contains("Chrome"));
        assert!(opts.proxy.is_none());
        assert!(!opts.accept_invalid_certs);
        assert!(opts.follow_redirects);
        assert_eq!(opts.max_redirects, 10);
    }

    #[test]
    fn test_chromium_options_builder_custom() {
        let opts = ChromiumOptions::builder()
            .timeout(Duration::from_secs(30))
            .headless(false)
            .no_sandbox(true)
            .disable_gpu(false)
            .proxy("http://127.0.0.1:7890")
            .proxy_auth("user", "pass")
            .browser_path("/usr/bin/chromium")
            .user_data_dir("/tmp/profile")
            .extension_dir("/path/to/ext")
            .arg("--disable-extensions")
            .build();
        assert_eq!(opts.timeout, Duration::from_secs(30));
        assert!(!opts.headless);
        assert!(opts.no_sandbox);
        assert!(!opts.disable_gpu);
        assert_eq!(opts.proxy.as_deref(), Some("http://127.0.0.1:7890"));
        assert_eq!(
            opts.proxy_auth.as_ref(),
            Some(&(String::from("user"), String::from("pass")))
        );
        assert_eq!(
            opts.browser_path.as_deref(),
            Some(std::path::Path::new("/usr/bin/chromium"))
        );
        assert_eq!(opts.extension_dirs.len(), 1);
        assert_eq!(opts.extra_args.len(), 1);
        assert_eq!(opts.extra_args[0], "--disable-extensions");
    }

    #[test]
    fn test_session_options_builder_custom() {
        let opts = SessionOptions::builder()
            .timeout(Duration::from_secs(60))
            .user_agent("CustomAgent/1.0")
            .proxy("socks5://localhost:9050")
            .accept_invalid_certs(true)
            .build();
        assert_eq!(opts.timeout, Duration::from_secs(60));
        assert_eq!(opts.user_agent, "CustomAgent/1.0");
        assert_eq!(opts.proxy.as_deref(), Some("socks5://localhost:9050"));
        assert!(opts.accept_invalid_certs);
    }

    #[test]
    fn test_web_page_options_default() {
        let opts = WebPageOptions::default();
        assert_eq!(opts.initial_mode, crate::web_page::PageMode::Chromium);
    }

    #[test]
    fn test_web_page_options_builder_session_mode() {
        let opts = WebPageOptions::builder()
            .initial_mode(crate::web_page::PageMode::Session)
            .build();
        assert_eq!(opts.initial_mode, crate::web_page::PageMode::Session);
    }
}
