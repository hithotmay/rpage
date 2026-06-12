//! WebPage - unified page combining Chromium and Session modes.
//!
//! The core abstraction: seamlessly switch between browser mode
//! and HTTP request mode with automatic cookie synchronization.

use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::Arc;

use tracing::info;

use crate::chromium_page::ChromiumPage;
use crate::chromium_page::FrameContext;
use crate::chromium_page::{ActionChain, InterceptGuard};
use crate::config::{ChromiumOptions, SessionOptions, WebPageOptions};
use crate::cookie_hub::CookieHub;
use crate::download::DownloadManager;
use crate::element::Element;
use crate::error::{Error, Result};
use crate::network::NetworkMonitor;
use crate::session_page::SessionPage;

/// Current page mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageMode {
    Chromium,
    Session,
}

impl std::fmt::Display for PageMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PageMode::Chromium => write!(f, "Chromium"),
            PageMode::Session => write!(f, "Session"),
        }
    }
}

/// WebPage — switch between browser and HTTP mode with `to_session()` / `to_chromium()`.
///
/// Cookies are automatically synchronized when switching modes.
/// All methods take `&self` (uses interior mutability for Session mode).
pub struct WebPage {
    mode: PageMode,
    chromium: Option<ChromiumPage>,
    session: RefCell<SessionPage>,
    cookie_hub: Arc<CookieHub>,
    opts: WebPageOptions,
}

impl WebPage {
    /// **启动浏览器** — 一个函数搞定，零自动化标记，永不触发验证码。
    /// Uses a random port to avoid multi-instance conflicts.
    pub async fn new() -> Result<Self> {
        let cookie_hub = Arc::new(CookieHub::new());
        let session = SessionPage::with_cookie_hub(cookie_hub.clone(), SessionOptions::default())?;
        let chromium = ChromiumPage::new().await?;
        Ok(Self {
            mode: PageMode::Chromium,
            chromium: Some(chromium),
            session: RefCell::new(session),
            cookie_hub,
            opts: WebPageOptions::default(),
        })
    }

    /// Create with custom options.
    pub async fn with_options(opts: WebPageOptions) -> Result<Self> {
        let cookie_hub = Arc::new(CookieHub::new());
        let session = SessionPage::with_cookie_hub(cookie_hub.clone(), opts.session.clone())?;
        let chromium = if opts.initial_mode == PageMode::Chromium {
            Some(ChromiumPage::with_options(opts.chromium.clone()).await?)
        } else {
            None
        };
        Ok(Self {
            mode: opts.initial_mode,
            chromium,
            session: RefCell::new(session),
            cookie_hub,
            opts,
        })
    }

    /// Create in Session-only mode (no browser).
    pub fn session_only(opts: Option<SessionOptions>) -> Result<Self> {
        let s_opts = opts.unwrap_or_default();
        let cookie_hub = Arc::new(CookieHub::new());
        let session = SessionPage::with_cookie_hub(cookie_hub.clone(), s_opts.clone())?;
        Ok(Self {
            mode: PageMode::Session,
            chromium: None,
            session: RefCell::new(session),
            cookie_hub,
            opts: WebPageOptions {
                chromium: ChromiumOptions::default(),
                session: s_opts,
                initial_mode: PageMode::Session,
            },
        })
    }

    /// **接管已打开的浏览器** — 零自动化标记，永不触发验证码。
    pub async fn connect(debug_url: &str) -> Result<Self> {
        let cookie_hub = Arc::new(CookieHub::new());
        let session = SessionPage::with_cookie_hub(cookie_hub.clone(), SessionOptions::default())?;
        let chromium = ChromiumPage::connect(debug_url).await?;
        Ok(Self {
            mode: PageMode::Chromium,
            chromium: Some(chromium),
            session: RefCell::new(session),
            cookie_hub,
            opts: WebPageOptions::default(),
        })
    }

    // ── Mode ─────────────────────────────────────────────────

    pub fn mode(&self) -> PageMode {
        self.mode
    }

    /// Switch to Session mode. Syncs cookies from browser → store.
    pub async fn to_session(&mut self) -> Result<()> {
        if self.mode == PageMode::Session {
            return Ok(());
        }
        if let Some(ref c) = self.chromium {
            let cookies = c.cookies().await?;
            self.cookie_hub.sync_from_chromium(cookies)?;
        }
        self.mode = PageMode::Session;
        info!("Switched to Session mode");
        Ok(())
    }

    /// Switch to Chromium mode. Launches browser if needed.
    pub async fn to_chromium(&mut self) -> Result<()> {
        if self.mode == PageMode::Chromium {
            return Ok(());
        }
        if self.chromium.is_none() {
            self.chromium = Some(ChromiumPage::with_options(self.opts.chromium.clone()).await?);
        }
        // Sync cookies from session store → browser
        let url_opt = self.session.borrow().url().map(String::from);
        if let (Some(ref c), Some(url)) = (&self.chromium, url_opt) {
            let cookies = self.cookie_hub.get_cookies(&url)?;
            for ck in cookies {
                let info = crate::chromium_page::CookieInfo {
                    name: ck.name().to_string(),
                    value: ck.value().to_string(),
                    domain: Some(match &ck.domain {
                        cookie_store::CookieDomain::Suffix(d) => d.to_string(),
                        cookie_store::CookieDomain::HostOnly(d) => d.to_string(),
                        _ => String::new(),
                    }),
                    path: Some(ck.path.to_string()),
                    secure: ck.secure().unwrap_or(false),
                    http_only: ck.http_only().unwrap_or(false),
                };
                c.set_cookie(info).await.ok();
            }
        }
        self.mode = PageMode::Chromium;
        info!("Switched to Chromium mode");
        Ok(())
    }

    // ── Navigation (all &self) ───────────────────────────────

    /// Navigate to URL. Auto-waits for page load in Chromium mode.
    #[allow(clippy::await_holding_refcell_ref)]
    pub async fn get(&self, url: &str) -> Result<()> {
        match self.mode {
            PageMode::Chromium => {
                self.chromium
                    .as_ref()
                    .ok_or_else(|| Error::Browser("no chromium".into()))?
                    .get(url)
                    .await?;
            }
            PageMode::Session => {
                self.session.borrow_mut().get(url).await?;
            }
        }
        Ok(())
    }

    /// POST request.
    #[allow(clippy::await_holding_refcell_ref)]
    pub async fn post(&self, url: &str, body: &str) -> Result<String> {
        match self.mode {
            PageMode::Chromium => {
                let c = self
                    .chromium
                    .as_ref()
                    .ok_or_else(|| Error::Browser("no chromium".into()))?;
                let js = format!(
                    "fetch('{}', {{method:'POST', body:{}}}).then(r=>r.text())",
                    url,
                    serde_json::to_string(body).unwrap_or_else(|_| "\"\"".to_string())
                );
                let val = c.execute(&js).await?;
                Ok(val.as_str().unwrap_or("").to_string())
            }
            PageMode::Session => self.session.borrow_mut().post(url, body.to_string()).await,
        }
    }

    // ── Elements ─────────────────────────────────────────────

    /// Find first element. Auto-retries up to 5s in Chromium mode.
    pub async fn ele(&self, locator_str: &str) -> Result<Element> {
        match self.mode {
            PageMode::Chromium => {
                self.chromium
                    .as_ref()
                    .ok_or_else(|| Error::Browser("no chromium".into()))?
                    .ele(locator_str)
                    .await
            }
            PageMode::Session => self.session.borrow().ele(locator_str),
        }
    }

    /// Find all elements.
    pub async fn eles(&self, locator_str: &str) -> Result<Vec<Element>> {
        match self.mode {
            PageMode::Chromium => {
                self.chromium
                    .as_ref()
                    .ok_or_else(|| Error::Browser("no chromium".into()))?
                    .eles(locator_str)
                    .await
            }
            PageMode::Session => self.session.borrow().eles(locator_str),
        }
    }

    /// Find an element inside a Shadow DOM host (Chromium mode only).
    ///
    /// Usage: `page.shadow_ele("#host >>> .inner")`
    pub async fn shadow_ele(&self, locator_str: &str) -> Result<Element> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("shadow_ele requires Chromium mode".into()))?
            .shadow_ele(locator_str)
            .await
    }

    /// Find all elements inside a Shadow DOM host (Chromium mode only).
    ///
    /// Usage: `page.shadow_eles("#host >>> .inner")`
    pub async fn shadow_eles(&self, locator_str: &str) -> Result<Vec<Element>> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("shadow_eles requires Chromium mode".into()))?
            .shadow_eles(locator_str)
            .await
    }

    // ── Page info ────────────────────────────────────────────

    /// Page HTML.
    pub async fn html(&self) -> Result<String> {
        match self.mode {
            PageMode::Chromium => {
                self.chromium
                    .as_ref()
                    .ok_or_else(|| Error::Browser("no chromium".into()))?
                    .html()
                    .await
            }
            PageMode::Session => Ok(self.session.borrow().html().to_string()),
        }
    }

    /// Page title.
    pub async fn title(&self) -> Result<String> {
        match self.mode {
            PageMode::Chromium => {
                self.chromium
                    .as_ref()
                    .ok_or_else(|| Error::Browser("no chromium".into()))?
                    .title()
                    .await
            }
            PageMode::Session => self
                .session
                .borrow()
                .title()
                .ok_or_else(|| Error::Browser("no title".into())),
        }
    }

    /// Current URL.
    pub async fn url(&self) -> Result<String> {
        match self.mode {
            PageMode::Chromium => {
                self.chromium
                    .as_ref()
                    .ok_or_else(|| Error::Browser("no chromium".into()))?
                    .url()
                    .await
            }
            PageMode::Session => self
                .session
                .borrow()
                .url()
                .map(String::from)
                .ok_or_else(|| Error::Browser("no URL".into())),
        }
    }

    // ── JS / Screenshot (Chromium only) ──────────────────────

    /// Execute JavaScript.
    pub async fn execute(&self, js: &str) -> Result<serde_json::Value> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("execute requires Chromium mode".into()))?
            .execute(js)
            .await
    }

    /// Screenshot → file.
    pub async fn screenshot(&self, path: &str) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("screenshot requires Chromium mode".into()))?
            .screenshot(path)
            .await
    }

    /// Screenshot → bytes.
    pub async fn screenshot_bytes(&self) -> Result<Vec<u8>> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("screenshot requires Chromium mode".into()))?
            .screenshot_bytes()
            .await
    }

    // ── Navigation helpers ───────────────────────────────────

    pub async fn refresh(&self) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("refresh requires Chromium mode".into()))?
            .refresh()
            .await
    }

    pub async fn back(&self) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("back requires Chromium mode".into()))?
            .back()
            .await
    }

    pub async fn forward(&self) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("forward requires Chromium mode".into()))?
            .forward()
            .await
    }

    /// Sleep for the specified duration.
    pub async fn sleep(&self, duration: std::time::Duration) {
        tokio::time::sleep(duration).await;
    }

    /// Close the browser.
    pub async fn close(&self) -> Result<()> {
        if let Some(ref c) = self.chromium {
            c.close().await?;
        }
        Ok(())
    }

    /// Manually sync cookies from browser → session store.
    pub async fn sync_cookies(&self) -> Result<()> {
        if let Some(ref c) = self.chromium {
            let cookies = c.cookies().await?;
            self.cookie_hub.sync_from_chromium(cookies)?;
        }
        Ok(())
    }

    // ── Browser lifecycle ────────────────────────────────────

    /// Quit the browser entirely.
    pub async fn quit(&self) -> Result<()> {
        if let Some(ref c) = self.chromium {
            c.quit().await?;
        }
        Ok(())
    }

    // ── Connection status / reconnection (f30) ─────────────

    /// Check if the browser connection is still alive (Chromium mode only).
    ///
    /// Returns `false` if there is no chromium instance or the browser is
    /// no longer reachable via its debug URL.
    pub fn is_connected(&self) -> bool {
        self.chromium
            .as_ref()
            .map(|c| c.is_connected())
            .unwrap_or(false)
    }

    /// Reconnect to the browser using the saved debug URL (Chromium mode only).
    ///
    /// Drops the current CDP connection and creates a fresh one to the same
    /// debug endpoint. The browser must still be running for this to succeed.
    pub async fn reconnect(&mut self) -> Result<()> {
        if let Some(ref mut c) = self.chromium {
            c.reconnect().await?;
        } else {
            return Err(Error::Browser("no chromium instance to reconnect".into()));
        }
        Ok(())
    }

    /// Return the saved debug URL (e.g. `http://localhost:9222`) if in Chromium mode.
    pub fn debug_url(&self) -> Option<&str> {
        self.chromium.as_ref().map(|c| c.debug_url())
    }

    // ── Scroll ────────────────────────────────────────────────

    /// Scroll page to absolute position.
    pub async fn scroll_to(&self, x: u32, y: u32) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("scroll requires Chromium mode".into()))?
            .scroll_to(x, y)
            .await
    }

    /// Scroll to page top.
    pub async fn scroll_to_top(&self) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("scroll requires Chromium mode".into()))?
            .scroll_to_top()
            .await
    }

    /// Scroll to page bottom.
    pub async fn scroll_to_bottom(&self) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("scroll requires Chromium mode".into()))?
            .scroll_to_bottom()
            .await
    }

    /// Scroll down by pixels.
    pub async fn scroll_down(&self, pixels: u32) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("scroll requires Chromium mode".into()))?
            .scroll_down(pixels)
            .await
    }

    /// Scroll up by pixels.
    pub async fn scroll_up(&self, pixels: u32) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("scroll requires Chromium mode".into()))?
            .scroll_up(pixels)
            .await
    }

    // ── Dialog / Alert ────────────────────────────────────────

    /// Handle a JavaScript dialog (alert/confirm/prompt).
    pub async fn handle_alert(&self, accept: bool, text: Option<&str>) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("handle_alert requires Chromium mode".into()))?
            .handle_alert(accept, text)
            .await
    }

    // ── Frames ────────────────────────────────────────────────

    /// Read an iframe's HTML content.
    pub async fn frame_html(&self, selector: &str) -> Result<String> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("frame_html requires Chromium mode".into()))?
            .frame_html(selector)
            .await
    }

    /// Execute JS in an iframe context.
    pub async fn frame_execute(&self, selector: &str, js: &str) -> Result<serde_json::Value> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("frame_execute requires Chromium mode".into()))?
            .frame_execute(selector, js)
            .await
    }

    // ── Cookie management ─────────────────────────────────────

    /// Delete a cookie by name.
    pub async fn delete_cookie(&self, name: &str) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("delete_cookie requires Chromium mode".into()))?
            .delete_cookie(name)
            .await
    }

    /// Clear all cookies.
    pub async fn clear_cookies(&self) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("clear_cookies requires Chromium mode".into()))?
            .clear_cookies()
            .await
    }

    // ── Multi-window management (f26) ──────────────────────────

    /// Return the `user_data_dir` configured for this instance (if any).
    pub fn user_data_dir(&self) -> Option<&PathBuf> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("user_data_dir requires Chromium mode".into()))
            .ok()
            .and_then(|c| c.user_data_dir())
    }

    /// Read all cookies from `self` and write them into `other`.
    pub async fn share_cookies_to(&self, other: &WebPage) -> Result<()> {
        let src = self.chromium.as_ref().ok_or_else(|| {
            Error::Browser("share_cookies_to source requires Chromium mode".into())
        })?;
        let dst = other.chromium.as_ref().ok_or_else(|| {
            Error::Browser("share_cookies_to target requires Chromium mode".into())
        })?;
        src.share_cookies_to(dst).await
    }

    /// Clone this session: launch a new browser sharing the same user_data_dir.
    pub async fn clone_session(&self) -> Result<WebPage> {
        let src = self
            .chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("clone_session requires Chromium mode".into()))?;
        let cloned = src.clone_session().await?;
        let mut wp = WebPage::with_options(self.opts.clone()).await?;
        wp.chromium = Some(cloned);
        Ok(wp)
    }

    // ── Cookies (read/set already exist, add tabs) ────────────

    /// Get all open tabs.
    pub async fn tabs(&self) -> Result<Vec<chromiumoxide::Page>> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("tabs requires Chromium mode".into()))?
            .tabs()
            .await
    }

    /// Open a new tab.
    pub async fn new_tab(&self) -> Result<chromiumoxide::Page> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("new_tab requires Chromium mode".into()))?
            .new_tab()
            .await
    }

    /// Get all tab titles.
    pub async fn tab_titles(&self) -> Result<Vec<String>> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("tab_titles requires Chromium mode".into()))?
            .tab_titles()
            .await
    }

    /// Get all tab URLs.
    pub async fn tab_urls(&self) -> Result<Vec<String>> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("tab_urls requires Chromium mode".into()))?
            .tab_urls()
            .await
    }

    /// Switch to a tab by index (0-based).
    pub async fn switch_to_tab(&self, index: usize) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("switch_to_tab requires Chromium mode".into()))?
            .switch_to_tab(index)
            .await
    }

    /// Close a tab by index.
    pub async fn close_tab(&self, index: usize) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("close_tab requires Chromium mode".into()))?
            .close_tab(index)
            .await
    }

    /// Set a cookie.
    pub async fn set_cookie(&self, cookie: crate::chromium_page::CookieInfo) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("set_cookie requires Chromium mode".into()))?
            .set_cookie(cookie)
            .await
    }

    /// Get all cookies.
    pub async fn cookies(&self) -> Result<Vec<crate::chromium_page::CookieInfo>> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("cookies requires Chromium mode".into()))?
            .cookies()
            .await
    }

    /// Evaluate JS on every new document.
    pub async fn evaluate_on_new_document(&self, js: &str) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("requires Chromium mode".into()))?
            .evaluate_on_new_document(js)
            .await
    }

    /// Register a named init script that runs on every new document.
    pub async fn add_init_script(&self, name: &str, js: &str) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("add_init_script requires Chromium mode".into()))?
            .add_init_script(name, js)
            .await
    }

    /// Remove a previously registered named init script.
    pub async fn remove_init_script(&self, name: &str) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("remove_init_script requires Chromium mode".into()))?
            .remove_init_script(name)
            .await
    }

    /// List all registered init script names.
    pub fn list_init_scripts(&self) -> Vec<String> {
        self.chromium
            .as_ref()
            .map(|c| c.list_init_scripts())
            .unwrap_or_default()
    }

    // ── Press / PDF / Viewport (Chromium only) ──────────────

    /// Press a key at page level.
    pub async fn press(&self, key: &str) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("press requires Chromium mode".into()))?
            .press(key)
            .await
    }

    /// Export page to PDF with default options (backward compatible).
    pub async fn pdf(&self, path: &str) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("pdf requires Chromium mode".into()))?
            .pdf(path)
            .await
    }

    /// Export page to PDF with custom options and save to `path`.
    pub async fn pdf_to_file(
        &self,
        path: &str,
        opts: crate::chromium_page::PdfOptions,
    ) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("pdf_to_file requires Chromium mode".into()))?
            .pdf_to_file(path, opts)
            .await
    }

    /// Export page to PDF with custom options and return raw bytes.
    pub async fn pdf_bytes(&self, opts: crate::chromium_page::PdfOptions) -> Result<Vec<u8>> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("pdf_bytes requires Chromium mode".into()))?
            .pdf_bytes(opts)
            .await
    }

    /// Set viewport size at runtime.
    pub async fn set_viewport(&self, width: u32, height: u32) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("set_viewport requires Chromium mode".into()))?
            .set_viewport(width, height)
            .await
    }

    // ── Conditional wait (Chromium only) ──────────────────

    /// Wait for an element matching the locator to appear.
    pub async fn wait_ele(&self, locator_str: &str, timeout_secs: u64) -> Result<Element> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("wait_ele requires Chromium mode".into()))?
            .wait_ele(locator_str, timeout_secs)
            .await
    }

    /// Wait for page title to contain the given text.
    pub async fn wait_title_contains(&self, text: &str, timeout_secs: u64) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("wait_title_contains requires Chromium mode".into()))?
            .wait_title_contains(text, timeout_secs)
            .await
    }

    /// Wait for URL to contain the given text.
    pub async fn wait_url_contains(&self, text: &str, timeout_secs: u64) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("wait_url_contains requires Chromium mode".into()))?
            .wait_url_contains(text, timeout_secs)
            .await
    }

    /// Wait for an element matching the locator to become hidden or be removed.
    pub async fn wait_ele_hidden(&self, locator_str: &str, timeout_secs: u64) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("wait_ele_hidden requires Chromium mode".into()))?
            .wait_ele_hidden(locator_str, timeout_secs)
            .await
    }

    /// Wait for an element matching the locator to be removed from the DOM entirely.
    pub async fn wait_ele_deleted(&self, locator_str: &str, timeout_secs: u64) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("wait_ele_deleted requires Chromium mode".into()))?
            .wait_ele_deleted(locator_str, timeout_secs)
            .await
    }

    // ── Runtime configuration (Chromium only) ─────────────

    /// Set extra HTTP headers for all subsequent requests.
    pub async fn set_extra_headers(
        &self,
        headers: std::collections::HashMap<String, String>,
    ) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("set_extra_headers requires Chromium mode".into()))?
            .set_extra_headers(headers)
            .await
    }

    /// Override user agent at runtime.
    pub async fn set_user_agent(&self, user_agent: &str) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("set_user_agent requires Chromium mode".into()))?
            .set_user_agent(user_agent)
            .await
    }

    /// Set proxy authentication (Chromium only).
    ///
    /// Configures `Proxy-Authorization: Basic <base64(user:pass)>` via
    /// `Network.setExtraHTTPHeaders`. The browser must have been launched
    /// with `--proxy-server` (e.g. via `ChromiumOptions::proxy`) for this
    /// to take effect.
    pub async fn set_proxy_auth(&self, user: &str, pass: &str) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("set_proxy_auth requires Chromium mode".into()))?
            .set_proxy_auth(user, pass)
            .await
    }

    // ── Multipart POST (Session only) ───────────────────────

    /// Send a multipart/form-data POST request with file upload (Session mode).
    #[allow(clippy::await_holding_refcell_ref)]
    pub async fn post_multipart(
        &self,
        url: &str,
        fields: std::collections::HashMap<String, String>,
        file_field: &str,
        file_path: &str,
    ) -> Result<String> {
        self.session
            .borrow_mut()
            .post_multipart(url, fields, file_field, file_path)
            .await
    }

    // ── Session convenience proxies ──────────────────────────

    /// Session GET request (Session mode).
    #[allow(clippy::await_holding_refcell_ref)]
    pub async fn session_get(&self, url: &str) -> Result<String> {
        self.session.borrow_mut().get(url).await
    }

    /// Session POST request with plain text body (Session mode).
    #[allow(clippy::await_holding_refcell_ref)]
    pub async fn session_post(&self, url: &str, body: &str) -> Result<String> {
        self.session.borrow_mut().post(url, body.to_string()).await
    }

    /// Session PUT request with plain text body (Session mode).
    #[allow(clippy::await_holding_refcell_ref)]
    pub async fn session_put(&self, url: &str, body: &str) -> Result<String> {
        self.session.borrow_mut().put(url, body.to_string()).await
    }

    /// Session DELETE request (Session mode).
    #[allow(clippy::await_holding_refcell_ref)]
    pub async fn session_delete(&self, url: &str) -> Result<String> {
        self.session.borrow_mut().delete(url).await
    }

    /// Session HEAD request (Session mode). Returns the HTTP status code.
    #[allow(clippy::await_holding_refcell_ref)]
    pub async fn session_head(&self, url: &str) -> Result<reqwest::StatusCode> {
        self.session.borrow_mut().head(url).await
    }

    /// Session POST JSON request (Session mode).
    #[allow(clippy::await_holding_refcell_ref)]
    pub async fn session_post_json(
        &self,
        url: &str,
        json: &serde_json::Value,
    ) -> Result<reqwest::Response> {
        self.session.borrow_mut().post_json(url, json).await
    }

    /// Session PATCH request with plain text body (Session mode).
    #[allow(clippy::await_holding_refcell_ref)]
    pub async fn session_patch(&self, url: &str, body: &str) -> Result<String> {
        self.session.borrow_mut().patch(url, body.to_string()).await
    }

    // ── Cookie save/load ────────────────────────────────────

    /// Save all cookies to a JSON file.
    pub fn save_cookies_to_file(&self, path: &str) -> Result<()> {
        self.cookie_hub.save_to_file(path)
    }

    /// Load cookies from a JSON file.
    pub fn load_cookies_from_file(&self, path: &str) -> Result<()> {
        self.cookie_hub.load_from_file(path)
    }

    // ── Accessors ────────────────────────────────────────────

    pub fn chromium(&self) -> Option<&ChromiumPage> {
        self.chromium.as_ref()
    }
    pub fn session(&self) -> std::cell::Ref<'_, SessionPage> {
        self.session.borrow()
    }
    pub fn session_mut(&self) -> std::cell::RefMut<'_, SessionPage> {
        self.session.borrow_mut()
    }
    pub fn cookie_hub(&self) -> &Arc<CookieHub> {
        &self.cookie_hub
    }

    // ── Low-level access ─────────────────────────────────────

    /// Access the underlying CDP page for advanced operations (cheap Arc clone).
    pub fn inner_page(&self) -> Option<chromiumoxide::Page> {
        self.chromium.as_ref().map(|c| c.inner_page())
    }

    /// Access the browser instance.
    pub fn browser(&self) -> Option<&chromiumoxide::browser::Browser> {
        self.chromium.as_ref().map(|c| c.browser())
    }

    /// Get current ChromiumOptions.
    pub fn options(&self) -> Option<&ChromiumOptions> {
        self.chromium.as_ref().map(|c| c.options())
    }

    /// Get download manager.
    pub fn download_manager(&self) -> Option<&std::sync::Arc<DownloadManager>> {
        self.chromium.as_ref().map(|c| c.download_manager())
    }

    /// Check if in Chromium mode.
    pub fn is_chromium(&self) -> bool {
        matches!(self.mode, PageMode::Chromium)
    }

    /// Check if in Session mode.
    pub fn is_session(&self) -> bool {
        matches!(self.mode, PageMode::Session)
    }

    // ── Advanced features ─────────────────────────────────

    /// Enter an iframe by CSS selector (Chromium mode only).
    pub async fn enter_frame(&self, selector: &str) -> Result<FrameContext> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("Not in Chromium mode".into()))?
            .enter_frame(selector)
            .await
    }

    /// Get the network monitor (Chromium mode only).
    pub fn network_monitor(&self) -> Option<&Arc<NetworkMonitor>> {
        self.chromium.as_ref().map(|c| c.network_monitor())
    }

    /// Get all captured console log entries (Chromium mode only).
    pub fn console_log(&self) -> Vec<crate::console::ConsoleEntry> {
        self.chromium
            .as_ref()
            .map(|c| c.console_log())
            .unwrap_or_default()
    }

    /// Get all captured JS exceptions (Chromium mode only).
    pub fn console_exceptions(&self) -> Vec<crate::console::JsException> {
        self.chromium
            .as_ref()
            .map(|c| c.console_exceptions())
            .unwrap_or_default()
    }

    /// Clear all captured console entries and exceptions (Chromium mode only).
    pub fn clear_console(&self) {
        if let Some(ref c) = self.chromium {
            c.clear_console();
        }
    }

    /// Get all captured WebSocket frames (Chromium mode only).
    pub fn ws_frames(&self) -> Vec<crate::websocket::WsFrame> {
        self.chromium
            .as_ref()
            .map(|c| c.ws_frames())
            .unwrap_or_default()
    }

    /// Get all captured WebSocket lifecycle events (Chromium mode only).
    pub fn ws_events(&self) -> Vec<crate::websocket::WsEvent> {
        self.chromium
            .as_ref()
            .map(|c| c.ws_events())
            .unwrap_or_default()
    }

    /// Clear all captured WebSocket frames and events (Chromium mode only).
    pub fn clear_ws_frames(&self) {
        if let Some(ref c) = self.chromium {
            c.clear_ws_frames();
        }
    }

    /// Create an ActionChain for complex multi-step input sequences.
    ///
    /// ```ignore
    /// let chain = page.actions();
    /// chain.move_to(100.0, 200.0)
    ///     .click_at(100.0, 200.0)
    ///     .key_down("Control")
    ///     .press("a")
    ///     .key_up("Control")
    ///     .perform()
    ///     .await?;
    /// ```
    pub fn actions(&self) -> Option<ActionChain> {
        self.chromium.as_ref().map(|c| c.actions())
    }

    /// Enable network request interception. Returns an `InterceptGuard`
    /// that holds paused requests. Call `continue_request()` or `fail_request()`
    /// to resume them. Interception is disabled when the guard is dropped.
    ///
    /// ```ignore
    /// let guard = page.enable_intercept("*/api/*").await?;
    /// tokio::time::sleep(Duration::from_secs(5)).await;
    /// for req in guard.paused_requests() {
    ///     guard.continue_request(&req.request_id, None).await?;
    /// }
    /// guard.disable().await?;
    /// ```
    pub async fn enable_intercept(&self, url_pattern: &str) -> Result<InterceptGuard> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("Not in Chromium mode".into()))?
            .enable_intercept(url_pattern)
            .await
    }

    /// Get all tracked downloads.
    pub fn downloads(&self) -> Vec<crate::download::DownloadInfo> {
        self.chromium
            .as_ref()
            .map(|c| c.downloads())
            .unwrap_or_default()
    }

    /// Wait for the most recent download to finish.
    pub async fn wait_download(&self, timeout_secs: u64) -> Result<crate::download::DownloadInfo> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("Not in Chromium mode".into()))?
            .wait_download(timeout_secs)
            .await
    }

    /// Wait until a JavaScript expression evaluates to a truthy value.
    pub async fn wait_js(&self, expression: &str, timeout_secs: u64) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("Not in Chromium mode".into()))?
            .wait_js(expression, timeout_secs)
            .await
    }

    /// Execute an async JavaScript expression and wait for the Promise to resolve.
    ///
    /// Uses CDP `Runtime.evaluate` with `awaitPromise = true` so that `fetch()`,
    /// `new Promise()`, and other async patterns complete before returning.
    ///
    /// ```ignore
    /// let json = page.run_async_js("fetch('/api/data').then(r => r.json())").await?;
    /// ```
    pub async fn run_async_js(&self, expression: &str) -> Result<serde_json::Value> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("run_async_js requires Chromium mode".into()))?
            .run_async_js(expression)
            .await
    }

    /// Execute a JavaScript function with arguments passed as a JSON value.
    ///
    /// The `expression` should be a function declaration. The `args` value is
    /// serialised and passed as the first argument.
    ///
    /// ```ignore
    /// let args = serde_json::json!({"selector": "#content"});
    /// let text = page.run_js_with_args(
    ///     "(a) => { let el = document.querySelector(a.selector); return el ? el.innerText : ''; }",
    ///     args,
    /// ).await?;
    /// ```
    pub async fn run_js_with_args(
        &self,
        expression: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("run_js_with_args requires Chromium mode".into()))?
            .run_js_with_args(expression, args)
            .await
    }

    /// Wait for a download whose URL contains `url_pattern` to complete.
    ///
    /// Polls the download manager and returns the
    /// [`DownloadInfo`](crate::download::DownloadInfo) once a matching
    /// download reaches a terminal state, or times out after `timeout_secs`.
    ///
    /// ```ignore
    /// let dl = page.wait_for_download("/files/report.pdf", 30).await?;
    /// ```
    pub async fn wait_for_download(
        &self,
        url_pattern: &str,
        timeout_secs: u64,
    ) -> Result<crate::download::DownloadInfo> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("wait_for_download requires Chromium mode".into()))?
            .wait_for_download(url_pattern, timeout_secs)
            .await
    }

    /// Get the `Content-Type` of the current page's main document.
    ///
    /// ```ignore
    /// let ct = page.get_content_type().await?;
    /// ```
    pub async fn get_content_type(&self) -> Result<String> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("get_content_type requires Chromium mode".into()))?
            .get_content_type()
            .await
    }

    /// Select all text on the page (Ctrl+A).
    pub async fn select_all_text(&self) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("select_all_text requires Chromium mode".into()))?
            .select_all_text()
            .await
    }

    /// Copy the currently selected text to the clipboard (Ctrl+C).
    pub async fn copy_text(&self) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("copy_text requires Chromium mode".into()))?
            .copy_text()
            .await
    }

    /// Paste text from the clipboard (Ctrl+V).
    pub async fn paste_text(&self) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("paste_text requires Chromium mode".into()))?
            .paste_text()
            .await
    }

    /// Search for `text` on the current page.
    ///
    /// Returns `true` if a match was found, `false` otherwise.
    ///
    /// ```ignore
    /// if page.find_text("Welcome").await? {
    ///     println!("Found!");
    /// }
    /// ```
    pub async fn find_text(&self, text: &str) -> Result<bool> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("find_text requires Chromium mode".into()))?
            .find_text(text)
            .await
    }

    // ── Performance metrics (Chromium only) ──────────────────

    /// Grant browser permissions for the given origin (Chromium only).
    ///
    /// ```ignore
    /// page.grant_permissions("https://example.com", vec![
    ///     "geolocation".into(),
    ///     "notifications".into(),
    /// ]).await?;
    /// ```
    pub async fn grant_permissions(&self, origin: &str, permissions: Vec<String>) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("grant_permissions requires Chromium mode".into()))?
            .grant_permissions(origin, permissions)
            .await
    }

    /// Reset all browser permission overrides (Chromium only).
    ///
    /// ```ignore
    /// page.reset_permissions().await?;
    /// ```
    pub async fn reset_permissions(&self) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("reset_permissions requires Chromium mode".into()))?
            .reset_permissions()
            .await
    }

    /// Retrieve current CDP performance metrics.
    ///
    /// Returns a list of `(name, value)` pairs from the browser's
    /// Performance domain (Timestamp, Documents, Frames, …).
    pub async fn performance_metrics(&self) -> Result<Vec<(String, f64)>> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("performance_metrics requires Chromium mode".into()))?
            .performance_metrics()
            .await
    }

    /// Extract page-load timing via `performance.timing`.
    ///
    /// Returns a `HashMap` with keys `dns`, `tcp`, `request`, `response`,
    /// `dom`, `load`, `domInteractive`, `domContentLoaded` (values in ms).
    pub async fn page_timing(&self) -> Result<std::collections::HashMap<String, f64>> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("page_timing requires Chromium mode".into()))?
            .page_timing()
            .await
    }

    // ── Device emulation (f22, Chromium only) ─────────────────

    /// Override the browser's geolocation (Chromium only).
    pub async fn set_geolocation(&self, lat: f64, lng: f64) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("set_geolocation requires Chromium mode".into()))?
            .set_geolocation(lat, lng)
            .await
    }

    /// Override the browser's timezone (Chromium only).
    pub async fn set_timezone(&self, tz: &str) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("set_timezone requires Chromium mode".into()))?
            .set_timezone(tz)
            .await
    }

    /// Emulate a device by setting viewport, scale factor, touch mode, and user agent (Chromium only).
    pub async fn emulate_device(
        &self,
        width: u32,
        height: u32,
        ua: &str,
        scale: f64,
        touch: bool,
    ) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("emulate_device requires Chromium mode".into()))?
            .emulate_device(width, height, ua, scale, touch)
            .await
    }

    // ── File chooser (f25) ────────────────────────────────────

    /// Enable or disable interception of file chooser dialogs (Chromium only).
    pub async fn set_file_chooser(&self, enabled: bool) {
        if let Some(ref c) = self.chromium {
            c.set_file_chooser(enabled).await;
        }
    }

    /// Wait for a file chooser dialog event within the given timeout (Chromium only).
    pub async fn wait_file_chooser(
        &self,
        timeout_secs: u64,
    ) -> Result<crate::chromium_page::FileChooserInfo> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("wait_file_chooser requires Chromium mode".into()))?
            .wait_file_chooser(timeout_secs)
            .await
    }

    // ── Audio control (f34) ──────────────────────────────────

    /// Mute all audio on the page (Chromium only).
    pub async fn mute(&self) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("mute requires Chromium mode".into()))?
            .mute()
            .await
    }

    /// Unmute audio on the page (Chromium only).
    pub async fn unmute(&self) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("unmute requires Chromium mode".into()))?
            .unmute()
            .await
    }

    // ── DOM Snapshot (f31) ────────────────────────────────────

    /// Capture a full DOM snapshot of the current page as a JSON tree (Chromium only).
    ///
    /// Each node is represented as `{ type, name, attrs?, children?, value? }`.
    pub async fn dom_snapshot(&self) -> Result<serde_json::Value> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("dom_snapshot requires Chromium mode".into()))?
            .dom_snapshot()
            .await
    }

    // ── Clipboard (f32) ──────────────────────────────────────

    /// Read text from the clipboard (Chromium only).
    ///
    /// The page must be focused and have clipboard-read permission.
    /// Use `grant_permissions` if needed.
    pub async fn clipboard_read(&self) -> Result<String> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("clipboard_read requires Chromium mode".into()))?
            .clipboard_read()
            .await
    }

    /// Write text to the clipboard (Chromium only).
    ///
    /// The page must be focused and have clipboard-write permission.
    /// Use `grant_permissions` if needed.
    pub async fn clipboard_write(&self, text: &str) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("clipboard_write requires Chromium mode".into()))?
            .clipboard_write(text)
            .await
    }

    // ── CSS override (f35) ──────────────────────────────────────

    /// Inject a `<style>` tag into the page and return its generated ID (Chromium only).
    ///
    /// The returned ID can later be passed to `remove_css` to delete the tag.
    pub async fn inject_css(&self, css: &str) -> Result<String> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("inject_css requires Chromium mode".into()))?
            .inject_css(css)
            .await
    }

    /// Remove a previously injected `<style>` tag by its ID (Chromium only).
    pub async fn remove_css(&self, id: &str) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("remove_css requires Chromium mode".into()))?
            .remove_css(id)
            .await
    }

    // ── DrissionPage-style convenience API (Chromium only) ──

    /// Navigate to a URL and return `&self` for chaining (Chromium mode only).
    ///
    /// ```ignore
    /// page.goto("https://example.com").await?.click_ele("#btn").await?;
    /// ```
    pub async fn goto(&self, url: &str) -> Result<&WebPage> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("goto requires Chromium mode".into()))?
            .goto(url)
            .await?;
        Ok(self)
    }

    /// Type text into the first element matching `selector` — wait + fill in one step (Chromium only).
    ///
    /// ```ignore
    /// page.type_text("#search", "hello").await?.click_ele("#go").await?;
    /// ```
    pub async fn type_text(&self, selector: &str, text: &str) -> Result<&WebPage> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("type_text requires Chromium mode".into()))?
            .type_text(selector, text)
            .await?;
        Ok(self)
    }

    /// Click the first element matching `selector` — wait + click in one step (Chromium only).
    ///
    /// ```ignore
    /// page.click_ele("#submit").await?;
    /// ```
    pub async fn click_ele(&self, selector: &str) -> Result<&WebPage> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("click_ele requires Chromium mode".into()))?
            .click_ele(selector)
            .await?;
        Ok(self)
    }

    /// Get the visible text of the first element matching `selector` (Chromium only).
    ///
    /// ```ignore
    /// let label = page.get_text("#result").await?;
    /// ```
    pub async fn get_text(&self, selector: &str) -> Result<String> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("get_text requires Chromium mode".into()))?
            .get_text(selector)
            .await
    }

    /// Get an attribute value from the first element matching `selector` (Chromium only).
    ///
    /// ```ignore
    /// let href = page.get_attr("#link", "href").await?;
    /// ```
    pub async fn get_attr(&self, selector: &str, attr: &str) -> Result<Option<String>> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("get_attr requires Chromium mode".into()))?
            .get_attr(selector, attr)
            .await
    }

    /// Wait until the current URL contains `expected_url` (Chromium only).
    ///
    /// ```ignore
    /// page.click_ele("#login").await?;
    /// page.wait_for_navigation("/dashboard", Duration::from_secs(10)).await?;
    /// ```
    pub async fn wait_for_navigation(
        &self,
        expected_url: &str,
        timeout: std::time::Duration,
    ) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("wait_for_navigation requires Chromium mode".into()))?
            .wait_for_navigation(expected_url, timeout)
            .await
    }

    /// Scroll the page by a relative offset in pixels (Chromium only).
    ///
    /// ```ignore
    /// page.scroll_by(0, 500).await?; // scroll down 500px
    /// ```
    pub async fn scroll_by(&self, x: i64, y: i64) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("scroll_by requires Chromium mode".into()))?
            .scroll_by(x, y)
            .await
    }

    /// Type text character-by-character to simulate realistic keyboard input (Chromium only).
    ///
    /// ```ignore
    /// page.keys("hello").await?;
    /// ```
    pub async fn keys(&self, text: &str) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("keys requires Chromium mode".into()))?
            .keys(text)
            .await
    }

    // ── DrissionPage-style convenience aliases & helpers ───

    /// Alias for [`url()`](Self::url) — DrissionPage uses `current_url`.
    pub async fn current_url(&self) -> Result<String> {
        self.url().await
    }

    /// Alias for [`title()`](Self::title) — DrissionPage uses `current_title`.
    pub async fn current_title(&self) -> Result<String> {
        self.title().await
    }

    /// Alias for [`html()`](Self::html) — DrissionPage uses `page_source`.
    pub async fn page_source(&self) -> Result<String> {
        self.html().await
    }

    /// Wait for the page URL to **exactly match** `expected` (Chromium only).
    pub async fn wait_url_is(&self, expected: &str, timeout_secs: u64) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("wait_url_is requires Chromium mode".into()))?
            .wait_url_is(expected, timeout_secs)
            .await
    }

    /// Wait for the page title to **exactly match** `expected` (Chromium only).
    pub async fn wait_title_is(&self, expected: &str, timeout_secs: u64) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("wait_title_is requires Chromium mode".into()))?
            .wait_title_is(expected, timeout_secs)
            .await
    }

    /// Re-locate an element in the live DOM using its original locator (Chromium only).
    pub async fn refresh_ele(&self, el: &Element) -> Result<Element> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("refresh_ele requires Chromium mode".into()))?
            .refresh_ele(el)
            .await
    }

    /// Type text into the first element matching `selector` in **append** mode (Chromium only).
    ///
    /// Unlike [`type_text`](Self::type_text) which clears the field first, this appends.
    /// Returns `&self` for chaining.
    pub async fn input_text(&self, selector: &str, text: &str) -> Result<&WebPage> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("input_text requires Chromium mode".into()))?
            .input_text(selector, text)
            .await?;
        Ok(self)
    }

    /// Hover over the first element matching `selector` — wait + hover (Chromium only).
    pub async fn hover_ele(&self, selector: &str) -> Result<&WebPage> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("hover_ele requires Chromium mode".into()))?
            .hover_ele(selector)
            .await?;
        Ok(self)
    }

    /// Scroll the first element matching `selector` into view — wait + scroll (Chromium only).
    pub async fn scroll_to_ele(&self, selector: &str) -> Result<&WebPage> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("scroll_to_ele requires Chromium mode".into()))?
            .scroll_to_ele(selector)
            .await?;
        Ok(self)
    }

    /// Quick check: does at least one element matching `selector` exist? (Chromium only)
    ///
    /// Returns `false` in Session mode. Never throws.
    pub async fn exists(&self, selector: &str) -> bool {
        match self.chromium.as_ref() {
            Some(c) => c.exists(selector).await,
            None => false,
        }
    }

    /// Count how many elements currently match `selector` (Chromium only).
    ///
    /// Returns `0` in Session mode or when the selector is invalid.
    pub async fn count(&self, selector: &str) -> usize {
        match self.chromium.as_ref() {
            Some(c) => c.count(selector).await,
            None => 0,
        }
    }

    /// Find the first element matching `selector`, or `None` if absent (Chromium only).
    ///
    /// Returns `None` in Session mode.
    pub async fn ele_or_none(&self, selector: &str) -> Option<Element> {
        match self.chromium.as_ref() {
            Some(c) => c.ele_or_none(selector).await,
            None => None,
        }
    }

    // ── Load strategy ────────────────────────────────────────

    /// Set the page load strategy (Chromium only).
    ///
    /// Controls how `get()` waits after navigation:
    /// - `"normal"` — wait for the full `load` event (default)
    /// - `"eager"` — wait for `DOMContentLoaded` only
    /// - `"none"` — return immediately after navigation
    pub fn set_load_strategy(&mut self, strategy: &str) {
        if let Some(c) = self.chromium.as_mut() {
            c.set_load_strategy(strategy);
        }
    }

    /// Get the current load strategy (Chromium only).
    ///
    /// Returns `"normal"`, `"eager"`, or `"none"`.
    pub fn load_strategy(&self) -> Option<&str> {
        self.chromium.as_ref().map(|c| c.load_strategy())
    }

    // ── Window management ─────────────────────────────────────

    /// Get the current browser window's bounds as `(left, top, width, height)` (Chromium only).
    pub async fn get_window_bounds(&self) -> Result<(i32, i32, u32, u32)> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("get_window_bounds requires Chromium mode".into()))?
            .get_window_bounds()
            .await
    }

    /// Set the window position (top-left corner) (Chromium only).
    pub async fn set_window_position(&self, left: i32, top: i32) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("set_window_position requires Chromium mode".into()))?
            .set_window_position(left, top)
            .await
    }

    /// Set the window size (width × height) (Chromium only).
    pub async fn set_window_size(&self, width: u32, height: u32) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("set_window_size requires Chromium mode".into()))?
            .set_window_size(width, height)
            .await
    }

    /// Minimize the browser window (Chromium only).
    pub async fn minimize(&self) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("minimize requires Chromium mode".into()))?
            .minimize()
            .await
    }

    /// Maximize the browser window (Chromium only).
    pub async fn maximize(&self) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("maximize requires Chromium mode".into()))?
            .maximize()
            .await
    }

    /// Set the browser window to fullscreen (Chromium only).
    pub async fn fullscreen(&self) -> Result<()> {
        self.chromium
            .as_ref()
            .ok_or_else(|| Error::Browser("fullscreen requires Chromium mode".into()))?
            .fullscreen()
            .await
    }
}
