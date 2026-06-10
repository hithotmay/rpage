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
use crate::locator::locator_to_selector;

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
    // 1. Check RPAGE_CHROME_PATH environment variable
    if let Ok(path) = std::env::var("RPAGE_CHROME_PATH") {
        let p = PathBuf::from(&path);
        if p.exists() {
            return Some(p);
        }
    }

    // 2. Check PATH for common binary names
    if let Ok(path_var) = std::env::var("PATH") {
        let separator = if cfg!(windows) { ';' } else { ':' };
        for dir in path_var.split(separator) {
            let candidates: &[&str] = if cfg!(windows) {
                &["chrome.exe", "chromium.exe"]
            } else {
                &["chrome", "chromium", "google-chrome", "chromium-browser"]
            };
            for name in candidates {
                let full = PathBuf::from(dir).join(name);
                if full.exists() {
                    return Some(full);
                }
            }
        }
    }

    // 3. Check standard install paths
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
        Self::launch_and_connect(
            &chrome_path,
            Some(&ud),
            port,
            &[],
            true,
            None,
            true,
            false,
            &[],
        )
        .await
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
            opts.disable_gpu,
            opts.no_sandbox,
            &opts.extension_dirs,
        )
        .await?;

        // Apply viewport
        if opts.viewport.width > 0 && opts.viewport.height > 0 {
            use chromiumoxide::cdp::browser_protocol::emulation::SetDeviceMetricsOverrideParams;
            let params = SetDeviceMetricsOverrideParams::new(
                opts.viewport.width as i64,
                opts.viewport.height as i64,
                1.0,
                false,
            );
            page.page
                .execute(params)
                .await
                .map_err(|e| Error::Browser(format!("viewport: {e}")))?;
        }

        // Apply user-agent if specified
        if !user_agent.is_empty() {
            crate::network::set_user_agent(&page.page, &user_agent).await?;
        }

        Ok(page)
    }
    #[allow(clippy::too_many_arguments)]
    async fn launch_and_connect(
        chrome_path: &PathBuf,
        user_data_dir: Option<&PathBuf>,
        port: u16,
        extra_args: &[String],
        headless: bool,
        proxy: Option<&str>,
        disable_gpu: bool,
        no_sandbox: bool,
        extension_dirs: &[PathBuf],
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

            // Apply disable-gpu
            if disable_gpu {
                cmd.arg("--disable-gpu");
            }

            // Apply no-sandbox
            if no_sandbox {
                cmd.arg("--no-sandbox");
            }

            // Apply extensions
            for dir in extension_dirs {
                cmd.arg(format!("--load-extension={}", dir.display()));
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

        // Apply stealth scripts
        crate::stealth::apply_stealth(&page, &crate::stealth::StealthConfig::default())
            .await
            .ok();

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
        // Best effort wait for navigation — don't fail if no actual navigation occurs
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            self.page.wait_for_navigation_response(),
        )
        .await;
        Ok(())
    }

    /// Go back. Waits for navigation.
    pub async fn back(&self) -> Result<()> {
        self.page
            .evaluate("history.back()")
            .await
            .map_err(|e| Error::Browser(format!("back: {e}")))?;
        // Best effort wait for navigation — don't fail for SPAs without real navigation
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            self.page.wait_for_navigation_response(),
        )
        .await;
        Ok(())
    }

    /// Go forward. Waits for navigation.
    pub async fn forward(&self) -> Result<()> {
        self.page
            .evaluate("history.forward()")
            .await
            .map_err(|e| Error::Browser(format!("forward: {e}")))?;
        // Best effort wait for navigation — don't fail for SPAs without real navigation
        let _ = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            self.page.wait_for_navigation_response(),
        )
        .await;
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

        // Handle Chain locator step-by-step: narrow scope through each step
        if let crate::locator::Locator::Chain(steps) = &locator {
            if steps.is_empty() {
                return Err(Error::InvalidLocator("empty chain".into()));
            }
            // Find the first element using first locator
            let first_sel = locator_to_selector(&steps[0])?;
            let timeout_secs = self.opts.timeout.as_secs();
            let mut cdp_el = self.wait_for_element(&first_sel, timeout_secs).await?;

            // For each subsequent step, search within the current element
            for step in steps.iter().skip(1) {
                let step_sel = locator_to_selector(step)?;
                cdp_el = cdp_el
                    .find_element(&step_sel)
                    .await
                    .map_err(|e| Error::ElementNotFound(format!("chain step: {e}")))?;
            }

            return self.build_element_from_cdp(cdp_el, locator).await;
        }

        let selector = locator_to_selector(&locator)?;

        // Auto-retry: wait up to configured timeout for element to appear
        let timeout_secs = self.opts.timeout.as_secs();
        let cdp_el = self.wait_for_element(&selector, timeout_secs).await?;

        self.build_element_from_cdp(cdp_el, locator).await
    }

    /// Find all matching elements (no retry — returns immediately).
    pub async fn eles(&self, locator_str: &str) -> Result<Vec<Element>> {
        let locator = crate::locator::parse_locator(locator_str)?;

        // Handle Chain locator step-by-step: narrow scope through each step
        if let crate::locator::Locator::Chain(steps) = &locator {
            if steps.is_empty() {
                return Err(Error::InvalidLocator("empty chain".into()));
            }
            // Find parent elements using first locator
            let first_sel = locator_to_selector(&steps[0])?;
            let parent_els = self
                .page
                .find_elements(&first_sel)
                .await
                .map_err(|e| Error::ElementNotFound(format!("chain first step: {e}")))?;

            // For each parent, find children matching remaining steps
            let mut results = Vec::new();
            for parent in parent_els {
                let mut inner_els = vec![parent];

                for step in steps.iter().skip(1) {
                    let step_sel = locator_to_selector(step)?;
                    let mut next_els = Vec::new();
                    for el in &inner_els {
                        if let Ok(children) = el.find_elements(&step_sel).await {
                            next_els.extend(children);
                        }
                    }
                    inner_els = next_els;
                    if inner_els.is_empty() {
                        break;
                    }
                }

                for cdp_el in &inner_els {
                    let el = self
                        .build_element_from_cdp_ref(cdp_el, locator.clone())
                        .await?;
                    results.push(el);
                }
            }
            return Ok(results);
        }

        let selector = locator_to_selector(&locator)?;

        // Auto-retry: wait up to configured timeout for at least one element
        let deadline = tokio::time::Instant::now() + self.opts.timeout;
        let mut cdp_els = self
            .page
            .find_elements(&selector)
            .await
            .map_err(|e| Error::ElementNotFound(format!("{e}")))?;

        while cdp_els.is_empty() && tokio::time::Instant::now() < deadline {
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            cdp_els = self
                .page
                .find_elements(&selector)
                .await
                .map_err(|e| Error::ElementNotFound(format!("{e}")))?;
        }

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
    async fn build_element_from_cdp_inner(
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

    /// Build an rpage Element from an owned CDP Element.
    async fn build_element_from_cdp(
        &self,
        cdp_el: chromiumoxide::Element,
        locator: crate::locator::Locator,
    ) -> Result<Element> {
        self.build_element_from_cdp_inner(&cdp_el, locator).await
    }

    /// Build an rpage Element from a CDP Element reference.
    async fn build_element_from_cdp_ref(
        &self,
        cdp_el: &chromiumoxide::Element,
        locator: crate::locator::Locator,
    ) -> Result<Element> {
        self.build_element_from_cdp_inner(cdp_el, locator).await
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

    /// Get all tab titles.
    pub async fn tab_titles(&self) -> Result<Vec<String>> {
        let pages = self
            .browser
            .pages()
            .await
            .map_err(|e| Error::Browser(format!("pages: {e}")))?;
        let mut titles = Vec::new();
        for p in &pages {
            if let Ok(t) = p.get_title().await {
                titles.push(t.unwrap_or_default());
            }
        }
        Ok(titles)
    }

    /// Get all tab URLs.
    pub async fn tab_urls(&self) -> Result<Vec<String>> {
        let pages = self
            .browser
            .pages()
            .await
            .map_err(|e| Error::Browser(format!("pages: {e}")))?;
        let mut urls = Vec::new();
        for p in &pages {
            if let Ok(u) = p.url().await {
                urls.push(u.unwrap_or_default());
            }
        }
        Ok(urls)
    }

    /// Switch to a tab by its index (0-based). Brings the tab to front.
    pub async fn switch_to_tab(&self, index: usize) -> Result<()> {
        let pages = self
            .browser
            .pages()
            .await
            .map_err(|e| Error::Browser(format!("pages: {e}")))?;
        let target = pages
            .get(index)
            .ok_or_else(|| Error::ElementNotFound(format!("tab index {index}")))?;
        target
            .bring_to_front()
            .await
            .map_err(|e| Error::Browser(format!("bring_to_front: {e}")))?;
        Ok(())
    }

    /// Close a tab by index.
    pub async fn close_tab(&self, index: usize) -> Result<()> {
        let pages = self
            .browser
            .pages()
            .await
            .map_err(|e| Error::Browser(format!("pages: {e}")))?;
        let target = pages
            .get(index)
            .ok_or_else(|| Error::ElementNotFound(format!("tab index {index}")))?;
        target
            .execute(chromiumoxide::cdp::browser_protocol::page::CloseParams::default())
            .await
            .map_err(|e| Error::Browser(format!("close_tab: {e}")))?;
        Ok(())
    }

    // ── Conditional wait ───────────────────────────────────

    /// Wait for an element matching the locator to appear.
    pub async fn wait_ele(&self, locator_str: &str, timeout_secs: u64) -> Result<Element> {
        let locator = crate::locator::parse_locator(locator_str)?;
        let selector = locator_to_selector(&locator)?;
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);

        loop {
            match self.page.find_element(&selector).await {
                Ok(cdp_el) => {
                    return self.build_element_from_cdp(cdp_el, locator).await;
                }
                Err(_) => {
                    if tokio::time::Instant::now() >= deadline {
                        return Err(Error::Timeout(format!(
                            "wait_ele '{}' timed out after {}s",
                            locator_str, timeout_secs
                        )));
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                }
            }
        }
    }

    /// Wait for an element matching the locator to become hidden or be removed.
    pub async fn wait_ele_hidden(&self, locator_str: &str, timeout_secs: u64) -> Result<()> {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
        let selector = {
            let locator = crate::locator::parse_locator(locator_str)?;
            locator_to_selector(&locator)?
        };
        loop {
            match self.page.find_element(&selector).await {
                Ok(_) => {
                    // Element still exists, check if visible via JS
                    let escaped = selector.replace('\\', "\\\\").replace('\'', "\\'");
                    let js = format!(
                        "!!(document.querySelector('{s}')?.offsetWidth || document.querySelector('{s}')?.offsetHeight)",
                        s = escaped
                    );
                    let visible = self
                        .page
                        .evaluate(js.as_str())
                        .await
                        .ok()
                        .and_then(|r| r.value().cloned())
                        .and_then(|v| v.as_bool())
                        .unwrap_or(true);
                    if !visible {
                        return Ok(());
                    }
                }
                Err(_) => {
                    // Element not found = gone
                    return Ok(());
                }
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(Error::Timeout(format!(
                    "wait_ele_hidden '{}' timed out after {}s",
                    locator_str, timeout_secs
                )));
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    }

    /// Wait for an element matching the locator to be removed from the DOM entirely.
    pub async fn wait_ele_deleted(&self, locator_str: &str, timeout_secs: u64) -> Result<()> {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
        let selector = {
            let locator = crate::locator::parse_locator(locator_str)?;
            locator_to_selector(&locator)?
        };
        loop {
            match self.page.find_element(&selector).await {
                Ok(_) => {
                    // Still exists
                }
                Err(_) => {
                    // Element gone
                    return Ok(());
                }
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(Error::Timeout(format!(
                    "wait_ele_deleted '{}' timed out after {}s",
                    locator_str, timeout_secs
                )));
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    }

    /// Wait for page title to contain the given text.
    pub async fn wait_title_contains(&self, text: &str, timeout_secs: u64) -> Result<()> {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
        loop {
            let title = self.title().await.unwrap_or_default();
            if title.contains(text) {
                return Ok(());
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(Error::Timeout(format!("wait_title '{}' timed out", text)));
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    }

    /// Wait for URL to contain the given text.
    pub async fn wait_url_contains(&self, text: &str, timeout_secs: u64) -> Result<()> {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
        loop {
            let url = self.url().await.unwrap_or_default();
            if url.contains(text) {
                return Ok(());
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(Error::Timeout(format!("wait_url '{}' timed out", text)));
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    }

    // ── Runtime configuration ──────────────────────────────

    /// Set extra HTTP headers for all subsequent requests.
    pub async fn set_extra_headers(
        &self,
        headers: std::collections::HashMap<String, String>,
    ) -> Result<()> {
        crate::network::set_extra_headers(&self.page, headers).await
    }

    /// Override user agent at runtime.
    pub async fn set_user_agent(&self, user_agent: &str) -> Result<()> {
        crate::network::set_user_agent(&self.page, user_agent).await
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
            "document.querySelector({sel}).contentDocument.documentElement.outerHTML",
            sel = serde_json::to_string(selector).unwrap()
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
            "(function(){{ var f = document.querySelector({sel}); if(!f) return null; return (function(){{ {code} }}).call(f.contentWindow); }})()",
            sel = serde_json::to_string(selector).unwrap(),
            code = js_code
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

    // ── PDF export ──────────────────────────────────────────

    /// Print current page to PDF and save to `path`.
    ///
    /// Note: generating PDF is only supported in Chrome headless mode.
    pub async fn pdf(&self, path: &str) -> Result<()> {
        use chromiumoxide::cdp::browser_protocol::page::PrintToPdfParams;
        let bytes = self
            .page
            .pdf(PrintToPdfParams::default())
            .await
            .map_err(|e| Error::Browser(format!("pdf: {e}")))?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    // ── Viewport ────────────────────────────────────────────

    /// Set viewport size at runtime.
    pub async fn set_viewport(&self, width: u32, height: u32) -> Result<()> {
        use chromiumoxide::cdp::browser_protocol::emulation::SetDeviceMetricsOverrideParams;
        let params = SetDeviceMetricsOverrideParams::new(width as i64, height as i64, 1.0, false);
        self.page
            .execute(params)
            .await
            .map_err(|e| Error::Browser(format!("viewport: {e}")))?;
        Ok(())
    }

    // ── Keyboard (page-level) ──────────────────────────────

    /// Press a key at page level (no element focus needed).
    pub async fn press(&self, key: &str) -> Result<()> {
        use chromiumoxide::cdp::browser_protocol::input::{
            DispatchKeyEventParams, DispatchKeyEventType,
        };
        let down = DispatchKeyEventParams::builder()
            .r#type(DispatchKeyEventType::KeyDown)
            .key(key)
            .build()
            .map_err(|e| Error::Browser(format!("key build: {e}")))?;
        self.page
            .execute(down)
            .await
            .map_err(|e| Error::Browser(format!("press: {e}")))?;
        let up = DispatchKeyEventParams::builder()
            .r#type(DispatchKeyEventType::KeyUp)
            .key(key)
            .build()
            .map_err(|e| Error::Browser(format!("key build: {e}")))?;
        self.page
            .execute(up)
            .await
            .map_err(|e| Error::Browser(format!("press up: {e}")))?;
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
