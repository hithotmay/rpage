//! ChromiumPage — browser automation via Chrome DevTools Protocol.
//!
//! Uses `chromiumoxide` to drive Chrome/Chromium. Stealth mode is enabled
//! by default to avoid bot-detection.

use std::path::PathBuf;
use std::sync::Arc;

use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::network::CookieParam;
use chromiumoxide::Page;
use futures::StreamExt;
use tracing::{debug, info};

use crate::config::ChromiumOptions;
use crate::download::DownloadManager;
use crate::element::Element;
use crate::error::{Error, Result};

/// Cookie info extracted from the browser.
#[derive(Debug, Clone)]
pub struct CookieInfo {
    pub name: String,
    pub value: String,
    pub domain: Option<String>,
    pub path: Option<String>,
    pub secure: bool,
    pub http_only: bool,
}

/// Try to find Chrome on the system.
fn find_chrome() -> Option<PathBuf> {
    let candidates = [
        r"C:\Program Files\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
        r"C:\Program Files\BraveSoftware\Brave-Browser\Application\brave.exe",
        "/usr/bin/google-chrome",
        "/usr/bin/chromium-browser",
        "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    ];
    for p in &candidates {
        let pb = PathBuf::from(p);
        if pb.exists() {
            return Some(pb);
        }
    }
    None
}

/// ChromiumPage wraps a headful/headless Chrome instance via CDP.
pub struct ChromiumPage {
    browser: Browser,
    page: Page,
    opts: ChromiumOptions,
    download_manager: Arc<DownloadManager>,
}

impl ChromiumPage {
    /// Launch with default options (headed, stealth enabled).
    pub async fn new() -> Result<Self> {
        Self::with_options(ChromiumOptions::default()).await
    }

    /// Launch with custom options.
    pub async fn with_options(opts: ChromiumOptions) -> Result<Self> {
        let mut cfg = BrowserConfig::builder();

        // Auto-detect Chrome if not specified
        if let Some(ref path) = opts.browser_path {
            cfg = cfg.chrome_executable(path);
        } else if let Some(chrome) = find_chrome() {
            info!("Auto-detected Chrome at {}", chrome.display());
            cfg = cfg.chrome_executable(chrome);
        }

        if opts.no_sandbox {
            cfg = cfg.no_sandbox();
        }
        if let Some(ref ud) = opts.user_data_dir {
            cfg = cfg.user_data_dir(ud);
        }
        if !opts.user_agent.is_empty() {
            cfg = cfg.arg(format!("--user-agent={}", opts.user_agent));
        }
        for a in &opts.extra_args {
            cfg = cfg.arg(a.as_str());
        }

        // Stealth: disable automation flag
        cfg = cfg.arg("--disable-blink-features=AutomationControlled");

        cfg = cfg.window_size(opts.viewport.width, opts.viewport.height);

        if opts.headless {
            // --headless=new doesn't expose "HeadlessChrome" in UA
            cfg = cfg.new_headless_mode();
            // Extra safety: override UA to remove "HeadlessChrome"
            cfg = cfg.arg("--user-agent=Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36");
        } else {
            cfg = cfg.with_head();
        }

        if opts.disable_gpu {
            cfg = cfg.arg("--disable-gpu");
        }

        let config = cfg
            .build()
            .map_err(|e| Error::Browser(format!("build config: {e}")))?;

        let (browser, handler) = Browser::launch(config)
            .await
            .map_err(|e| Error::Browser(format!("launch: {e}")))?;

        tokio::spawn(async move {
            let mut h = handler;
            while h.next().await.is_some() {}
        });

        let page = browser
            .new_page("about:blank")
            .await
            .map_err(|e| Error::Browser(format!("new page: {e}")))?;

        // Stealth: inject anti-detection JS.
        // 1) evaluate() on about:blank so it takes effect immediately
        // 2) evaluate_on_new_document() so it runs before every future navigation
        let stealth_js = r#"
            Object.defineProperty(navigator, 'webdriver', {get: () => undefined});
            Object.defineProperty(navigator, 'plugins', {get: () => [1,2,3,4,5]});
            Object.defineProperty(navigator, 'languages', {get: () => ['zh-CN','zh','en']});
            window.chrome = { runtime: {} };
            const origQuery = window.navigator.permissions.query;
            window.navigator.permissions.query = (parameters) => (
                parameters.name === 'notifications' ?
                    Promise.resolve({ state: Notification.permission }) :
                    origQuery(parameters)
            );
        "#;
        // Run immediately on about:blank
        if let Err(e) = page.evaluate(stealth_js).await {
            debug!("stealth evaluate failed: {e:?}");
        }
        // Also register for every new document load
        if let Err(e) = page.evaluate_on_new_document(stealth_js).await {
            debug!("stealth on-new-doc failed: {e:?}");
        }

        // Override User-Agent to remove "HeadlessChrome" marker
        if opts.headless {
            let ua = if opts.user_agent.is_empty() {
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"
            } else {
                &opts.user_agent
            };
            if let Err(e) = page.set_user_agent(ua).await {
                debug!("set_user_agent failed: {e:?}");
            }
        }

        info!("ChromiumPage ready");
        Ok(Self {
            browser,
            page,
            opts,
            download_manager: Arc::new(DownloadManager::new()),
        })
    }

    // ── Navigation ───────────────────────────────────────────

    /// Navigate to a URL.
    pub async fn get(&self, url: &str) -> Result<()> {
        debug!("get({url})");
        self.page
            .goto(url)
            .await
            .map_err(|e| Error::Browser(format!("navigate: {e}")))?;
        Ok(())
    }

    /// Refresh current page.
    pub async fn refresh(&self) -> Result<()> {
        self.page
            .reload()
            .await
            .map_err(|e| Error::Browser(format!("refresh: {e}")))?;
        Ok(())
    }

    /// Go back.
    pub async fn back(&self) -> Result<()> {
        self.page
            .evaluate("history.back()")
            .await
            .map_err(|e| Error::Browser(format!("back: {e}")))?;
        Ok(())
    }

    /// Go forward.
    pub async fn forward(&self) -> Result<()> {
        self.page
            .evaluate("history.forward()")
            .await
            .map_err(|e| Error::Browser(format!("forward: {e}")))?;
        Ok(())
    }

    // ── Element finding (return rpage::Element with page ref) ──

    /// Find the first element and return an rpage Element (with CDP page ref).
    pub async fn ele(&self, locator_str: &str) -> Result<Element> {
        let locator = crate::locator::parse_locator(locator_str)?;
        let selector = locator_to_selector(&locator)?;
        let cdp_el = self
            .page
            .find_element(&selector)
            .await
            .map_err(|e| Error::ElementNotFound(format!("{e}")))?;

        let html = cdp_el.outer_html().await.ok().flatten().unwrap_or_default();
        let text = cdp_el.inner_text().await.ok().flatten().unwrap_or_default();
        let tag = cdp_el
            .string_property("tagName")
            .await
            .ok()
            .flatten()
            .unwrap_or_default()
            .to_lowercase();
        let attrs = cdp_el
            .call_js_fn(
                "function(){ var r=[]; for(var i=0;i<this.attributes.length;i++){var a=this.attributes[i]; r.push([a.name,a.value]);} return JSON.stringify(r); }",
                false,
            )
            .await
            .ok()
            .and_then(|r| {
                r.result.value.and_then(|v| serde_json::from_value(v).ok())
            })
            .unwrap_or_default();

        Ok(Element::new_cdp(
            self.page.clone(),
            cdp_el.remote_object_id.clone().into(),
            Some(locator),
            html,
            tag,
            text,
            attrs,
        ))
    }

    /// Find all matching elements.
    pub async fn eles(&self, locator_str: &str) -> Result<Vec<Element>> {
        let locator = crate::locator::parse_locator(locator_str)?;
        let selector = locator_to_selector(&locator)?;
        let cdp_els = self
            .page
            .find_elements(&selector)
            .await
            .map_err(|e| Error::ElementNotFound(format!("{e}")))?;

        let mut results = Vec::with_capacity(cdp_els.len());
        for cdp_el in &cdp_els {
            let html = cdp_el.outer_html().await.ok().flatten().unwrap_or_default();
            let text = cdp_el.inner_text().await.ok().flatten().unwrap_or_default();
            let tag = cdp_el
                .string_property("tagName")
                .await
                .ok()
                .flatten()
                .unwrap_or_default()
                .to_lowercase();
            let attrs = cdp_el
                .call_js_fn(
                    "function(){ var r=[]; for(var i=0;i<this.attributes.length;i++){var a=this.attributes[i]; r.push([a.name,a.value]);} return JSON.stringify(r); }",
                    false,
                )
                .await
                .ok()
                .and_then(|r| {
                    r.result.value.and_then(|v| serde_json::from_value(v).ok())
                })
                .unwrap_or_default();

            results.push(Element::new_cdp(
                self.page.clone(),
                cdp_el.remote_object_id.clone().into(),
                Some(locator.clone()),
                html,
                tag,
                text,
                attrs,
            ));
        }
        Ok(results)
    }

    // ── Page info ────────────────────────────────────────────

    /// Page HTML.
    pub async fn html(&self) -> Result<String> {
        self.page
            .content()
            .await
            .map_err(|e| Error::Browser(format!("content: {e}")))
    }

    /// Page title.
    pub async fn title(&self) -> Result<String> {
        self.page
            .get_title()
            .await
            .map_err(|e| Error::Browser(format!("title: {e}")))
            .map(|t| t.unwrap_or_default())
    }

    /// Current URL.
    pub async fn url(&self) -> Result<String> {
        self.page
            .url()
            .await
            .map_err(|e| Error::Browser(format!("url: {e}")))
            .map(|u| u.unwrap_or_default())
    }

    // ── JavaScript ───────────────────────────────────────────

    /// Execute JS, return the value.
    pub async fn execute(&self, js: &str) -> Result<serde_json::Value> {
        let r = self
            .page
            .evaluate(js)
            .await
            .map_err(|e| Error::Browser(format!("eval: {e}")))?;
        Ok(r.value().cloned().unwrap_or(serde_json::Value::Null))
    }

    /// Execute JS on every new document.
    pub async fn evaluate_on_new_document(&self, js: &str) -> Result<()> {
        self.page
            .evaluate_on_new_document(js)
            .await
            .map_err(|e| Error::Browser(format!("init script: {e}")))?;
        Ok(())
    }

    // ── Screenshot ───────────────────────────────────────────

    /// Screenshot → PNG bytes.
    pub async fn screenshot_bytes(&self) -> Result<Vec<u8>> {
        use chromiumoxide::page::ScreenshotParams;
        self.page
            .screenshot(ScreenshotParams::builder().build())
            .await
            .map_err(|e| Error::Browser(format!("screenshot: {e}")))
    }

    /// Screenshot → file.
    pub async fn screenshot(&self, path: &str) -> Result<()> {
        let bytes = self.screenshot_bytes().await?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    // ── Cookies ──────────────────────────────────────────────

    /// Get all cookies.
    pub async fn cookies(&self) -> Result<Vec<CookieInfo>> {
        let cookies = self
            .page
            .get_cookies()
            .await
            .map_err(|e| Error::Browser(format!("cookies: {e}")))?;
        Ok(cookies
            .iter()
            .map(|c| CookieInfo {
                name: c.name.clone(),
                value: c.value.clone(),
                domain: Some(c.domain.clone()),
                path: Some(c.path.clone()),
                secure: c.secure,
                http_only: c.http_only,
            })
            .collect())
    }

    /// Set a cookie.
    pub async fn set_cookie(&self, cookie: CookieInfo) -> Result<()> {
        let mut cp = CookieParam::new(&cookie.name, &cookie.value);
        if let Some(ref d) = cookie.domain {
            cp.domain = Some(d.clone());
        }
        if let Some(ref p) = cookie.path {
            cp.path = Some(p.clone());
        }
        if cookie.secure {
            cp.secure = Some(true);
        }
        if cookie.http_only {
            cp.http_only = Some(true);
        }
        self.page
            .set_cookie(cp)
            .await
            .map_err(|e| Error::Browser(format!("set cookie: {e}")))?;
        Ok(())
    }

    // ── Tabs ─────────────────────────────────────────────────

    /// Get all open pages/tabs.
    pub async fn tabs(&self) -> Result<Vec<Page>> {
        self.browser
            .pages()
            .await
            .map_err(|e| Error::Browser(format!("tabs: {e}")))
    }

    /// Open a new tab.
    pub async fn new_tab(&self) -> Result<Page> {
        self.browser
            .new_page("about:blank")
            .await
            .map_err(|e| Error::Browser(format!("new tab: {e}")))
    }

    // ── Accessors ────────────────────────────────────────────

    pub fn inner_page(&self) -> &Page {
        &self.page
    }
    pub fn browser(&self) -> &Browser {
        &self.browser
    }
    pub fn options(&self) -> &ChromiumOptions {
        &self.opts
    }
    pub fn download_manager(&self) -> &Arc<DownloadManager> {
        &self.download_manager
    }
}

/// Convert our Locator to a CSS/XPath selector string for chromiumoxide.
fn locator_to_selector(locator: &crate::locator::Locator) -> Result<String> {
    match locator {
        crate::locator::Locator::Css(sel) => Ok(sel.clone()),
        crate::locator::Locator::XPath(xp) => Ok(format!("xpath:{xp}")),
        crate::locator::Locator::Text(t) => {
            Ok(format!("xpath://*[text()='{}']", t.replace('\'', "\\'")))
        }
        crate::locator::Locator::TextContains(t) => Ok(format!(
            "xpath://*[contains(text(),'{}')]",
            t.replace('\'', "\\'")
        )),
        crate::locator::Locator::AttrEquals { attr, value } => Ok(format!(
            "xpath://*[@{}='{}']",
            attr,
            value.replace('\'', "\\'")
        )),
        crate::locator::Locator::AttrContains { attr, value } => Ok(format!(
            "xpath://*[contains(@{},'{}')]",
            attr,
            value.replace('\'', "\\'")
        )),
        crate::locator::Locator::Chain(locators) => locators
            .last()
            .ok_or_else(|| Error::InvalidLocator("empty chain".into()))
            .and_then(locator_to_selector),
    }
}
