//! WebPage - unified page that combines Chromium and Session modes
//!
//! The core abstraction of rpage: seamlessly switch between browser mode
//! and HTTP request mode with automatic cookie synchronization.

use std::sync::Arc;

use tracing::info;

use crate::chromium_page::ChromiumPage;
use crate::config::{ChromiumOptions, SessionOptions, WebPageOptions};
use crate::cookie_hub::CookieHub;
use crate::element::Element;
use crate::error::{Error, Result};
use crate::session_page::SessionPage;

/// Current page mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageMode {
    /// Browser mode (CDP)
    Chromium,
    /// HTTP request mode (reqwest)
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

/// WebPage combines ChromiumPage and SessionPage into a unified interface.
///
/// Switch between browser and HTTP mode with `to_session()` / `to_chromium()`.
/// Cookies are automatically synchronized when switching modes.
pub struct WebPage {
    mode: PageMode,
    chromium: Option<ChromiumPage>,
    session: SessionPage,
    cookie_hub: Arc<CookieHub>,
    opts: WebPageOptions,
}

impl WebPage {
    /// Create a new WebPage in Chromium mode with default options
    pub async fn new() -> Result<Self> {
        Self::with_options(WebPageOptions::default()).await
    }

    /// Create a new WebPage with custom options
    pub async fn with_options(opts: WebPageOptions) -> Result<Self> {
        let cookie_hub = Arc::new(CookieHub::new());
        let session = SessionPage::with_cookie_hub(cookie_hub.clone(), opts.session.clone())?;

        let chromium = if opts.initial_mode == PageMode::Chromium {
            Some(ChromiumPage::with_options(opts.chromium.clone()).await?)
        } else {
            None
        };

        let mode = opts.initial_mode;

        Ok(Self {
            mode,
            chromium,
            session,
            cookie_hub,
            opts,
        })
    }

    /// Create a WebPage in Session-only mode (no browser)
    pub fn session_only(opts: Option<SessionOptions>) -> Result<Self> {
        let session_opts = opts.unwrap_or_default();
        let cookie_hub = Arc::new(CookieHub::new());
        let session = SessionPage::with_cookie_hub(cookie_hub.clone(), session_opts.clone())?;

        Ok(Self {
            mode: PageMode::Session,
            chromium: None,
            session,
            cookie_hub,
            opts: WebPageOptions {
                chromium: ChromiumOptions::default(),
                session: session_opts,
                initial_mode: PageMode::Session,
            },
        })
    }

    /// Get the current page mode
    pub fn mode(&self) -> PageMode {
        self.mode
    }

    /// Navigate to a URL (works in both modes)
    pub async fn get(&mut self, url: &str) -> Result<()> {
        match self.mode {
            PageMode::Chromium => {
                if let Some(ref chromium) = self.chromium {
                    chromium.get(url).await?;
                }
            }
            PageMode::Session => {
                self.session.get(url).await?;
            }
        }
        Ok(())
    }

    /// Send a POST request (Session mode) or use JS fetch (Chromium mode)
    pub async fn post(&mut self, url: &str, body: &str) -> Result<String> {
        match self.mode {
            PageMode::Chromium => {
                if let Some(ref chromium) = self.chromium {
                    let js = format!(
                        "fetch('{}', {{method:'POST', body:{}}}).then(r=>r.text())",
                        url,
                        serde_json::to_string(body).unwrap_or_else(|_| "\"\"".to_string())
                    );
                    let val = chromium.execute_script(&js).await?;
                    Ok(val.as_str().unwrap_or("").to_string())
                } else {
                    Err(Error::Browser("no chromium page".into()))
                }
            }
            PageMode::Session => self.session.post(url, body.to_string()).await,
        }
    }

    /// Find the first element matching the locator
    pub async fn ele(&self, locator_str: &str) -> Result<Element> {
        match self.mode {
            PageMode::Chromium => {
                if let Some(ref chromium) = self.chromium {
                    let cdp_el = chromium.find_element_raw(locator_str).await?;
                    build_element_from_cdp(&cdp_el, locator_str).await
                } else {
                    Err(Error::Browser("no chromium page".into()))
                }
            }
            PageMode::Session => self.session.ele(locator_str),
        }
    }

    /// Find all elements matching the locator
    pub async fn eles(&self, locator_str: &str) -> Result<Vec<Element>> {
        match self.mode {
            PageMode::Chromium => {
                if let Some(ref chromium) = self.chromium {
                    let cdp_els = chromium.find_elements_raw(locator_str).await?;
                    let mut results = Vec::new();
                    for cdp_el in cdp_els {
                        match build_element_from_cdp(&cdp_el, locator_str).await {
                            Ok(el) => results.push(el),
                            Err(_) => continue,
                        }
                    }
                    Ok(results)
                } else {
                    Err(Error::Browser("no chromium page".into()))
                }
            }
            PageMode::Session => self.session.eles(locator_str),
        }
    }

    /// Get the current page HTML
    pub async fn html(&self) -> Result<String> {
        match self.mode {
            PageMode::Chromium => {
                if let Some(ref chromium) = self.chromium {
                    chromium.html().await
                } else {
                    Err(Error::Browser("no chromium page".into()))
                }
            }
            PageMode::Session => Ok(self.session.html().to_string()),
        }
    }

    /// Get the page title
    pub async fn title(&self) -> Result<String> {
        match self.mode {
            PageMode::Chromium => {
                if let Some(ref chromium) = self.chromium {
                    chromium.title().await
                } else {
                    Err(Error::Browser("no chromium page".into()))
                }
            }
            PageMode::Session => self
                .session
                .title()
                .ok_or_else(|| Error::Browser("no title found".into())),
        }
    }

    /// Get the current URL
    pub async fn url(&self) -> Result<String> {
        match self.mode {
            PageMode::Chromium => {
                if let Some(ref chromium) = self.chromium {
                    chromium.url().await
                } else {
                    Err(Error::Browser("no chromium page".into()))
                }
            }
            PageMode::Session => self
                .session
                .url()
                .map(String::from)
                .ok_or_else(|| Error::Browser("no URL".into())),
        }
    }

    /// Execute JavaScript (Chromium mode only)
    pub async fn execute_script(&self, js: &str) -> Result<serde_json::Value> {
        match self.mode {
            PageMode::Chromium => {
                if let Some(ref chromium) = self.chromium {
                    chromium.execute_script(js).await
                } else {
                    Err(Error::Browser("no chromium page".into()))
                }
            }
            PageMode::Session => Err(Error::Browser(
                "execute_script is only available in Chromium mode".into(),
            )),
        }
    }

    /// Take a screenshot (Chromium mode only)
    pub async fn screenshot(&self, path: &str) -> Result<()> {
        match self.mode {
            PageMode::Chromium => {
                if let Some(ref chromium) = self.chromium {
                    chromium.screenshot(path).await
                } else {
                    Err(Error::Browser("no chromium page".into()))
                }
            }
            PageMode::Session => Err(Error::Browser(
                "screenshot is only available in Chromium mode".into(),
            )),
        }
    }

    /// Switch to Session (HTTP) mode. Syncs cookies from browser.
    pub async fn to_session(&mut self) -> Result<()> {
        if self.mode == PageMode::Session {
            return Ok(());
        }

        // Sync cookies from chromium to session
        if let Some(ref chromium) = self.chromium {
            let cookies = chromium.cookies().await?;
            self.cookie_hub.sync_from_chromium(cookies)?;
        }

        self.mode = PageMode::Session;
        info!("Switched to Session mode");
        Ok(())
    }

    /// Switch to Chromium (browser) mode. Launches browser if needed.
    pub async fn to_chromium(&mut self) -> Result<()> {
        if self.mode == PageMode::Chromium {
            return Ok(());
        }

        // Launch browser if not already running
        if self.chromium.is_none() {
            let chromium = ChromiumPage::with_options(self.opts.chromium.clone()).await?;
            self.chromium = Some(chromium);
        }

        // Sync cookies from session to chromium
        if let (Some(ref chromium), Some(url)) = (&self.chromium, self.session.url()) {
            let cookies = self.cookie_hub.get_cookies(url)?;
            for cookie in cookies {
                let info = crate::chromium_page::CookieInfo {
                    name: cookie.name().to_string(),
                    value: cookie.value().to_string(),
                    domain: match &cookie.domain {
                        cookie_store::CookieDomain::Suffix(d) => Some(d.to_string()),
                        cookie_store::CookieDomain::HostOnly(d) => Some(d.to_string()),
                        _ => None,
                    },
                    path: Some(cookie.path.to_string()),
                    secure: cookie.secure().unwrap_or(false),
                    http_only: cookie.http_only().unwrap_or(false),
                };
                chromium.set_cookie(info).await.ok();
            }
        }

        self.mode = PageMode::Chromium;
        info!("Switched to Chromium mode");
        Ok(())
    }

    /// Manually sync cookies from browser to session
    pub async fn sync_cookies(&mut self) -> Result<()> {
        if let Some(ref chromium) = self.chromium {
            let cookies = chromium.cookies().await?;
            self.cookie_hub.sync_from_chromium(cookies)?;
        }
        Ok(())
    }

    /// Refresh the current page
    pub async fn refresh(&self) -> Result<()> {
        if let Some(ref chromium) = self.chromium {
            chromium.refresh().await
        } else {
            Err(Error::Browser("no chromium page".into()))
        }
    }

    /// Go back in browser history (Chromium mode only)
    pub async fn back(&self) -> Result<()> {
        if let Some(ref chromium) = self.chromium {
            chromium.back().await
        } else {
            Err(Error::Browser("no chromium page".into()))
        }
    }

    /// Go forward in browser history (Chromium mode only)
    pub async fn forward(&self) -> Result<()> {
        if let Some(ref chromium) = self.chromium {
            chromium.forward().await
        } else {
            Err(Error::Browser("no chromium page".into()))
        }
    }

    /// Get a reference to the ChromiumPage (if available)
    pub fn chromium(&self) -> Option<&ChromiumPage> {
        self.chromium.as_ref()
    }

    /// Get a reference to the SessionPage
    pub fn session(&self) -> &SessionPage {
        &self.session
    }

    /// Get a mutable reference to the SessionPage
    pub fn session_mut(&mut self) -> &mut SessionPage {
        &mut self.session
    }

    /// Get the shared cookie hub
    pub fn cookie_hub(&self) -> &Arc<CookieHub> {
        &self.cookie_hub
    }
}

/// Helper: extract element data from a CDP element
async fn build_element_from_cdp(
    cdp_el: &chromiumoxide::Element,
    locator_str: &str,
) -> Result<Element> {
    let html = cdp_el.outer_html().await.ok().flatten().unwrap_or_default();
    let text = cdp_el.inner_text().await.ok().flatten().unwrap_or_default();

    // Get tag name via JS property
    let tag = cdp_el
        .string_property("tagName")
        .await
        .ok()
        .flatten()
        .unwrap_or_default()
        .to_lowercase();

    // Get attributes via JS
    let attrs = cdp_el
        .call_js_fn(
            "function() { var r=[]; for(var i=0;i<this.attributes.length;i++){var a=this.attributes[i]; r.push([a.name,a.value]);} return JSON.stringify(r); }",
            false,
        )
        .await
        .ok()
        .and_then(|r| {
            r.result.value.and_then(|v| {
                let arr: Vec<(String, String)> = serde_json::from_value(v).ok()?;
                Some(arr)
            })
        })
        .unwrap_or_default();

    let locator = crate::locator::parse_locator(locator_str).ok();
    Ok(Element::new(
        crate::element::PageId::Cdp(cdp_el.remote_object_id.clone().into()),
        locator,
        html,
        tag,
        text,
        attrs,
    ))
}
