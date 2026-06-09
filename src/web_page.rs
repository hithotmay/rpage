//! WebPage - unified page combining Chromium and Session modes.
//!
//! The core abstraction: seamlessly switch between browser mode
//! and HTTP request mode with automatic cookie synchronization.

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
pub struct WebPage {
    mode: PageMode,
    chromium: Option<ChromiumPage>,
    session: SessionPage,
    cookie_hub: Arc<CookieHub>,
    opts: WebPageOptions,
}

impl WebPage {
    /// Create in Chromium mode with defaults.
    pub async fn new() -> Result<Self> {
        Self::with_options(WebPageOptions::default()).await
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
            session,
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
            session,
            cookie_hub,
            opts: WebPageOptions {
                chromium: ChromiumOptions::default(),
                session: s_opts,
                initial_mode: PageMode::Session,
            },
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
        if let (Some(ref c), Some(url)) = (&self.chromium, self.session.url()) {
            let cookies = self.cookie_hub.get_cookies(url)?;
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

    // ── Navigation ───────────────────────────────────────────

    /// Navigate to URL (both modes).
    pub async fn get(&mut self, url: &str) -> Result<()> {
        match self.mode {
            PageMode::Chromium => {
                self.chromium
                    .as_ref()
                    .ok_or_else(|| Error::Browser("no chromium".into()))?
                    .get(url)
                    .await?;
            }
            PageMode::Session => {
                self.session.get(url).await?;
            }
        }
        Ok(())
    }

    /// POST request.
    pub async fn post(&mut self, url: &str, body: &str) -> Result<String> {
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
            PageMode::Session => self.session.post(url, body.to_string()).await,
        }
    }

    // ── Elements ─────────────────────────────────────────────

    /// Find first element (CDP element carries page ref → supports click/input).
    pub async fn ele(&self, locator_str: &str) -> Result<Element> {
        match self.mode {
            PageMode::Chromium => {
                self.chromium
                    .as_ref()
                    .ok_or_else(|| Error::Browser("no chromium".into()))?
                    .ele(locator_str)
                    .await
            }
            PageMode::Session => self.session.ele(locator_str),
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
            PageMode::Session => self.session.eles(locator_str),
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
            PageMode::Session => Ok(self.session.html().to_string()),
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

    /// Manually sync cookies from browser → session store.
    pub async fn sync_cookies(&mut self) -> Result<()> {
        if let Some(ref c) = self.chromium {
            let cookies = c.cookies().await?;
            self.cookie_hub.sync_from_chromium(cookies)?;
        }
        Ok(())
    }

    // ── Accessors ────────────────────────────────────────────

    pub fn chromium(&self) -> Option<&ChromiumPage> {
        self.chromium.as_ref()
    }
    pub fn session(&self) -> &SessionPage {
        &self.session
    }
    pub fn session_mut(&mut self) -> &mut SessionPage {
        &mut self.session
    }
    pub fn cookie_hub(&self) -> &Arc<CookieHub> {
        &self.cookie_hub
    }
}
