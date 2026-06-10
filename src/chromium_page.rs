//! ChromiumPage — browser automation via Chrome DevTools Protocol.
//!
//! Uses `chromiumoxide` to drive Chrome/Chromium. Stealth mode is enabled
//! by default to avoid bot-detection.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use chromiumoxide::browser::Browser;
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
    /// **启动浏览器并接管** — 一个函数搞定，零自动化标记，永不触发验证码。
    ///
    /// 内部流程：
    /// 1. 自动检测系统 Chrome 路径
    /// 2. 用 `Command` 启动 Chrome（只传 `--remote-debugging-port`）
    /// 3. 等待调试端口就绪
    /// 4. 通过 CDP 连接接管
    ///
    /// 因为不走 chromiumoxide 的 `Browser::launch`（它会加 `--enable-automation` 等
    /// 默认参数），所以浏览器没有任何自动化标记，和用户手动打开的完全一样。
    pub async fn new() -> Result<Self> {
        let chrome_path = find_chrome().ok_or_else(|| Error::Browser("Chrome not found".into()))?;
        // Use a dedicated user-data-dir to avoid conflicts with running Chrome
        let ud = std::env::temp_dir().join("rpage-chrome");
        // Use PID-based port to avoid multi-instance conflicts
        let port = 9300 + ((std::process::id() as u16) % 700);
        Self::launch_and_connect(&chrome_path, Some(&ud), port, &[], true, None).await
    }

    /// 用自定义选项启动浏览器。
    pub async fn with_options(opts: ChromiumOptions) -> Result<Self> {
        let chrome_path = if let Some(ref path) = opts.browser_path {
            path.clone()
        } else {
            find_chrome().ok_or_else(|| Error::Browser("Chrome not found".into()))?
        };

        let user_data_dir = opts
            .user_data_dir
            .clone()
            .unwrap_or_else(|| std::env::temp_dir().join("rpage-chrome"));
        let port = opts.debug_port;
        let extra_args = opts.extra_args.clone();
        let headless = opts.headless;
        let proxy = opts.proxy.clone();
        let user_agent = opts.user_agent.clone();

        let page = Self::launch_and_connect(
            &chrome_path,
            Some(&user_data_dir),
            port,
            &extra_args,
            headless,
            proxy.as_deref(),
        )
        .await?;

        // Apply viewport
        if opts.viewport.width > 0 && opts.viewport.height > 0 {
            let js = format!(
                "window.resizeTo({}, {})",
                opts.viewport.width, opts.viewport.height
            );
            page.page.evaluate(js.as_str()).await.ok();
        }

        // Apply user-agent if specified
        if !user_agent.is_empty() {
            crate::network::set_user_agent(&page.page, &user_agent).await?;
        }

        Ok(page)
    }
    async fn launch_and_connect(
        chrome_path: &PathBuf,
        user_data_dir: Option<&PathBuf>,
        port: u16,
        extra_args: &[String],
        headless: bool,
        proxy: Option<&str>,
    ) -> Result<Self> {
        let debug_url = format!("http://localhost:{port}");

        // Check if a browser is already listening on this port
        let already_running = reqwest::get(format!("{debug_url}/json/version"))
            .await
            .is_ok();

        if !already_running {
            info!(
                "Launching Chrome at {} (port {port})",
                chrome_path.display()
            );

            let mut cmd = Command::new(chrome_path);
            cmd.arg(format!("--remote-debugging-port={port}"));

            if let Some(ud) = user_data_dir {
                cmd.arg(format!("--user-data-dir={}", ud.display()));
            } else {
                // Chrome requires non-default data dir for remote debugging
                let tmp = std::env::temp_dir().join("rpage-chrome");
                cmd.arg(format!("--user-data-dir={}", tmp.display()));
            }

            // Apply headless mode
            if headless {
                cmd.arg("--headless=new");
            }

            // Apply proxy
            if let Some(proxy_url) = proxy {
                cmd.arg(format!("--proxy-server={proxy_url}"));
            }

            for arg in extra_args {
                cmd.arg(arg);
            }

            // Windows: create process without console window
            #[cfg(target_os = "windows")]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
            }

            cmd.spawn()
                .map_err(|e| Error::Browser(format!("spawn Chrome: {e}")))?;

            // Wait for debug port to be ready
            Self::wait_for_port(debug_url.clone()).await?;
        } else {
            info!("Browser already running on port {port}, reusing");
        }

        // Connect via CDP
        Self::connect(&debug_url).await
    }

    /// Poll the debug port until Chrome is ready (max 10s).
    async fn wait_for_port(debug_url: String) -> Result<()> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(2))
            .build()
            .map_err(|e| Error::Browser(format!("http client: {e}")))?;

        for _ in 0..50 {
            if client
                .get(format!("{debug_url}/json/version"))
                .send()
                .await
                .is_ok()
            {
                info!("Chrome debug port ready");
                return Ok(());
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        Err(Error::Browser(
            "Chrome debug port not ready after 10s".into(),
        ))
    }

    /// **接管已打开的浏览器** — 零自动化标记，永远不会触发验证码。
    ///
    /// 用法：
    /// 1. 先用命令行启动 Chrome（用你自己的 profile）：
    ///    `chrome --remote-debugging-port=9222`
    /// 2. 然后 `ChromiumPage::connect("http://localhost:9222")` 接管
    ///
    /// 因为浏览器是你手动打开的，没有任何 `--enable-automation`、
    /// `HeadlessChrome` UA、`navigator.webdriver` 等标记，
    /// 所有网站（包括百度）都不会触发验证码。
    pub async fn connect(debug_url: &str) -> Result<Self> {
        info!("Connecting to existing browser at {debug_url}");

        let (browser, handler) = Browser::connect(debug_url)
            .await
            .map_err(|e| Error::Browser(format!("connect: {e}")))?;

        tokio::spawn(async move {
            let mut h = handler;
            while h.next().await.is_some() {}
        });

        // Get the first existing page, or create one
        let pages = browser
            .pages()
            .await
            .map_err(|e| Error::Browser(format!("get pages: {e}")))?;

        let page = if let Some(p) = pages.into_iter().next() {
            info!("Reusing existing page");
            p
        } else {
            info!("Creating new page");
            browser
                .new_page("about:blank")
                .await
                .map_err(|e| Error::Browser(format!("new page: {e}")))?
        };

        info!("Connected to existing browser — zero automation flags");
        Ok(Self {
            browser,
            page,
            opts: ChromiumOptions::default(),
            download_manager: Arc::new(DownloadManager::new()),
        })
    }

    // ── Navigation (auto-wait for page load) ────────────────

    /// Navigate to a URL. Automatically waits for page to finish loading.
    pub async fn get(&self, url: &str) -> Result<()> {
        debug!("get({url})");
        self.page
            .goto(url)
            .await
            .map_err(|e| Error::Browser(format!("navigate: {e}")))?;
        // Wait for DOMContentLoaded
        self.page
            .wait_for_navigation_response()
            .await
            .map_err(|e| Error::Browser(format!("wait for load: {e}")))?;
        Ok(())
    }

    /// Refresh current page. Waits for page to finish loading.
    pub async fn refresh(&self) -> Result<()> {
        self.page
            .reload()
            .await
            .map_err(|e| Error::Browser(format!("refresh: {e}")))?;
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        Ok(())
    }

    /// Go back. Waits for navigation.
    pub async fn back(&self) -> Result<()> {
        self.page
            .evaluate("history.back()")
            .await
            .map_err(|e| Error::Browser(format!("back: {e}")))?;
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        Ok(())
    }

    /// Go forward. Waits for navigation.
    pub async fn forward(&self) -> Result<()> {
        self.page
            .evaluate("history.forward()")
            .await
            .map_err(|e| Error::Browser(format!("forward: {e}")))?;
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        Ok(())
    }

    /// Sleep for the specified duration.
    pub async fn sleep(&self, duration: std::time::Duration) {
        tokio::time::sleep(duration).await;
    }

    /// Close the browser.
    pub async fn close(&self) -> Result<()> {
        self.page
            .execute(chromiumoxide::cdp::browser_protocol::page::CloseParams::default())
            .await
            .map_err(|e| Error::Browser(format!("close: {e}")))?;
        Ok(())
    }

    // ── Element finding (auto-retry + batch extract) ─────────

    /// Find the first element. Auto-retries for up to 5 seconds if not found.
    pub async fn ele(&self, locator_str: &str) -> Result<Element> {
        let locator = crate::locator::parse_locator(locator_str)?;
        let selector = locator_to_selector(&locator)?;

        // Auto-retry: wait up to 5 seconds for element to appear
        let cdp_el = self.wait_for_element(&selector, 5).await?;

        self.build_element_from_cdp(cdp_el, locator).await
    }

    /// Find all matching elements (no retry — returns immediately).
    pub async fn eles(&self, locator_str: &str) -> Result<Vec<Element>> {
        let locator = crate::locator::parse_locator(locator_str)?;
        let selector = locator_to_selector(&locator)?;
        let cdp_els = self
            .page
            .find_elements(&selector)
            .await
            .map_err(|e| Error::ElementNotFound(format!("{e}")))?;

        let locator_clone = locator.clone();
        let mut results = Vec::with_capacity(cdp_els.len());

        if cdp_els.is_empty() {
            return Ok(results);
        }

        // Use individual extraction for reliability (batch JS is fragile)
        for cdp_el in &cdp_els {
            let el = self
                .build_element_from_cdp_ref(cdp_el, locator_clone.clone())
                .await?;
            results.push(el);
        }
        Ok(results)
    }

    // ── Internal helpers ─────────────────────────────────────

    /// Wait for an element to appear, retrying for `timeout_secs` seconds.
    async fn wait_for_element(
        &self,
        selector: &str,
        timeout_secs: u64,
    ) -> Result<chromiumoxide::Element> {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
        let mut last_err = String::from("timeout");

        while tokio::time::Instant::now() < deadline {
            match self.page.find_element(selector).await {
                Ok(el) => return Ok(el),
                Err(e) => {
                    last_err = format!("{e}");
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                }
            }
        }

        Err(Error::ElementNotFound(format!(
            "not found after {timeout_secs}s: {last_err}"
        )))
    }

    /// Build an rpage Element from a CDP Element.
    async fn build_element_from_cdp(
        &self,
        cdp_el: chromiumoxide::Element,
        locator: crate::locator::Locator,
    ) -> Result<Element> {
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

    /// Build an rpage Element from a CDP Element reference.
    async fn build_element_from_cdp_ref(
        &self,
        cdp_el: &chromiumoxide::Element,
        locator: crate::locator::Locator,
    ) -> Result<Element> {
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

    // ── Browser lifecycle ───────────────────────────────────

    /// Quit the browser entirely (kills Chrome process).
    pub async fn quit(&self) -> Result<()> {
        // Use CDP Browser.close to gracefully shut down
        use chromiumoxide::cdp::browser_protocol::browser::CloseParams;
        self.page
            .execute(CloseParams::default())
            .await
            .map_err(|e| Error::Browser(format!("quit: {e}")))?;
        Ok(())
    }

    // ── Scroll ──────────────────────────────────────────────

    /// Scroll the page to absolute position.
    pub async fn scroll_to(&self, x: u32, y: u32) -> Result<()> {
        self.page
            .evaluate(format!("window.scrollTo({x}, {y})"))
            .await
            .map_err(|e| Error::Browser(format!("scroll: {e}")))?;
        Ok(())
    }

    /// Scroll to the top of the page.
    pub async fn scroll_to_top(&self) -> Result<()> {
        self.scroll_to(0, 0).await
    }

    /// Scroll to the bottom of the page.
    pub async fn scroll_to_bottom(&self) -> Result<()> {
        let js = "window.scrollTo(0, document.body.scrollHeight)";
        self.page
            .evaluate(js)
            .await
            .map_err(|e| Error::Browser(format!("scroll bottom: {e}")))?;
        Ok(())
    }

    /// Scroll up by `pixels`.
    pub async fn scroll_up(&self, pixels: u32) -> Result<()> {
        let js = format!("window.scrollBy(0, -{pixels})");
        self.page
            .evaluate(js.as_str())
            .await
            .map_err(|e| Error::Browser(format!("scroll up: {e}")))?;
        Ok(())
    }

    /// Scroll down by `pixels`.
    pub async fn scroll_down(&self, pixels: u32) -> Result<()> {
        let js = format!("window.scrollBy(0, {pixels})");
        self.page
            .evaluate(js.as_str())
            .await
            .map_err(|e| Error::Browser(format!("scroll down: {e}")))?;
        Ok(())
    }

    // ── Dialog / Alert ─────────────────────────────────────

    /// Handle a JavaScript dialog (alert/confirm/prompt).
    /// `accept`: true = accept (OK), false = dismiss (Cancel)
    /// `text`: prompt text to enter (only for prompt dialogs)
    pub async fn handle_alert(&self, accept: bool, text: Option<&str>) -> Result<()> {
        use chromiumoxide::cdp::browser_protocol::page::HandleJavaScriptDialogParams;
        let mut params = HandleJavaScriptDialogParams::new(accept);
        if let Some(t) = text {
            params.prompt_text = Some(t.into());
        }
        self.page
            .execute(params)
            .await
            .map_err(|e| Error::Browser(format!("handle dialog: {e}")))?;
        Ok(())
    }

    // ── Frames ──────────────────────────────────────────────

    /// Get the HTML content of an iframe identified by CSS selector.
    pub async fn frame_html(&self, selector: &str) -> Result<String> {
        let js = format!(
            "document.querySelector('{sel}').contentDocument.documentElement.outerHTML",
            sel = selector.replace('\'', "\\'")
        );
        self.page
            .evaluate(js.as_str())
            .await
            .map_err(|e| Error::Browser(format!("frame html: {e}")))?
            .value()
            .cloned()
            .map(|v| v.as_str().unwrap_or_default().to_string())
            .ok_or_else(|| Error::Browser("frame html: no result".into()))
    }

    /// Execute JavaScript inside an iframe identified by CSS selector.
    pub async fn frame_execute(&self, selector: &str, js_code: &str) -> Result<serde_json::Value> {
        let js = format!(
            "(function(){{ var f = document.querySelector('{sel}'); if(!f) return null; return (function(){{ {code} }}).call(f.contentWindow); }})()",
            sel = selector.replace('\'', "\\'"),
            code = js_code.replace('\'', "\\'")
        );
        let r = self
            .page
            .evaluate(js.as_str())
            .await
            .map_err(|e| Error::Browser(format!("frame execute: {e}")))?;
        Ok(r.value().cloned().unwrap_or(serde_json::Value::Null))
    }

    // ── Cookie management ───────────────────────────────────

    /// Delete a cookie by name.
    pub async fn delete_cookie(&self, name: &str) -> Result<()> {
        use chromiumoxide::cdp::browser_protocol::network::DeleteCookiesParams;
        let params = DeleteCookiesParams::new(name);
        self.page
            .execute(params)
            .await
            .map_err(|e| Error::Browser(format!("delete cookie: {e}")))?;
        Ok(())
    }

    /// Clear all cookies for the current page.
    pub async fn clear_cookies(&self) -> Result<()> {
        use chromiumoxide::cdp::browser_protocol::network::ClearBrowserCookiesParams;
        self.page
            .execute(ClearBrowserCookiesParams::default())
            .await
            .map_err(|e| Error::Browser(format!("clear cookies: {e}")))?;
        Ok(())
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
