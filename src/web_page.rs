//! WebPage - unified page combining Chromium and Session modes.
//!
//! The core abstraction: seamlessly switch between browser mode
//! and HTTP request mode with automatic cookie synchronization.

use std::cell::RefCell;
use std::sync::Arc;

use tracing::info;

use crate::chromium_page::ChromiumPage;
use crate::config::{ChromiumOptions, SessionOptions, WebPageOptions};
use crate::cookie_hub::CookieHub;
use crate::element::Element;
use crate::error::{Error, Result};
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
}
