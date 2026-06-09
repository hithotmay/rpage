//! ChromiumPage - browser automation via Chrome DevTools Protocol

use std::sync::Arc;

use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::cdp::browser_protocol::network::CookieParam;
use chromiumoxide::Page;
use futures::StreamExt;
use tracing::{debug, info, instrument};

use crate::config::ChromiumOptions;
use crate::download::DownloadManager;
use crate::error::{Error, Result};

/// Cookie information extracted from the browser
#[derive(Debug, Clone)]
pub struct CookieInfo {
    pub name: String,
    pub value: String,
    pub domain: Option<String>,
    pub path: Option<String>,
    pub secure: bool,
    pub http_only: bool,
}

/// ChromiumPage wraps a headful/headless Chrome instance via CDP.
pub struct ChromiumPage {
    browser: Browser,
    page: Page,
    opts: ChromiumOptions,
    download_manager: Arc<DownloadManager>,
}

impl ChromiumPage {
    /// Launch a new browser with default options (headless)
    pub async fn new() -> Result<Self> {
        Self::with_options(ChromiumOptions::default()).await
    }

    /// Launch a new browser with custom options
    #[instrument]
    pub async fn with_options(opts: ChromiumOptions) -> Result<Self> {
        let mut config_builder = BrowserConfig::builder();

        if opts.no_sandbox {
            config_builder = config_builder.no_sandbox();
        }

        if let Some(ref path) = opts.browser_path {
            config_builder = config_builder.chrome_executable(path);
        }

        if let Some(ref user_data) = opts.user_data_dir {
            config_builder = config_builder.user_data_dir(user_data);
        }

        for ext_dir in &opts.extension_dirs {
            config_builder = config_builder.arg(format!("--load-extension={}", ext_dir.display()));
        }

        if !opts.user_agent.is_empty() {
            config_builder = config_builder.arg(format!("--user-agent={}", opts.user_agent));
        }

        for arg in &opts.extra_args {
            config_builder = config_builder.arg(arg.as_str());
        }

        if opts.headless {
            config_builder = config_builder.new_headless_mode();
        } else {
            config_builder = config_builder.with_head();
        }

        config_builder = config_builder.window_size(opts.viewport.width, opts.viewport.height);

        if opts.disable_gpu {
            config_builder = config_builder.arg("--disable-gpu");
        }

        let config = config_builder
            .build()
            .map_err(|e| Error::Browser(format!("failed to build browser config: {e}")))?;

        let (browser, handler) = Browser::launch(config)
            .await
            .map_err(|e| Error::Browser(format!("failed to launch browser: {e}")))?;

        // Spawn the CDP handler as a stream
        tokio::spawn(async move {
            let mut h = handler;
            while h.next().await.is_some() {}
        });

        // Open a new page
        let page = browser
            .new_page("about:blank")
            .await
            .map_err(|e| Error::Browser(format!("failed to open new page: {e}")))?;

        info!("ChromiumPage created successfully");

        Ok(Self {
            browser,
            page,
            opts,
            download_manager: Arc::new(DownloadManager::new()),
        })
    }

    /// Navigate to a URL
    #[instrument(skip(self))]
    pub async fn get(&self, url: &str) -> Result<()> {
        debug!("Navigating to {url}");
        self.page
            .goto(url)
            .await
            .map_err(|e| Error::Browser(format!("navigation failed: {e}")))?;
        Ok(())
    }

    /// Find the first element matching the locator (returns the raw CDP element)
    pub async fn find_element_raw(&self, locator_str: &str) -> Result<chromiumoxide::Element> {
        let locator = crate::locator::parse_locator(locator_str)?;
        let selector = self.locator_to_css_or_xpath(&locator)?;
        self.page
            .find_element(&selector)
            .await
            .map_err(|e| Error::ElementNotFound(format!("{e}")))
    }

    /// Find all elements matching the locator
    pub async fn find_elements_raw(
        &self,
        locator_str: &str,
    ) -> Result<Vec<chromiumoxide::Element>> {
        let locator = crate::locator::parse_locator(locator_str)?;
        let selector = self.locator_to_css_or_xpath(&locator)?;
        self.page
            .find_elements(&selector)
            .await
            .map_err(|e| Error::ElementNotFound(format!("{e}")))
    }

    /// Convert our Locator to a CSS selector or XPath string
    fn locator_to_css_or_xpath(&self, locator: &crate::locator::Locator) -> Result<String> {
        match locator {
            crate::locator::Locator::Css(sel) => Ok(sel.clone()),
            crate::locator::Locator::XPath(xp) => Ok(xp.clone()),
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
            crate::locator::Locator::Chain(locators) => {
                if let Some(last) = locators.last() {
                    self.locator_to_css_or_xpath(last)
                } else {
                    Err(Error::InvalidLocator("empty chain".into()))
                }
            }
        }
    }

    /// Get the current page HTML
    #[instrument(skip(self))]
    pub async fn html(&self) -> Result<String> {
        self.page
            .content()
            .await
            .map_err(|e| Error::Browser(format!("get content: {e}")))
    }

    /// Get the page title
    #[instrument(skip(self))]
    pub async fn title(&self) -> Result<String> {
        let result = self
            .page
            .get_title()
            .await
            .map_err(|e| Error::Browser(format!("get title: {e}")))?;
        Ok(result.unwrap_or_default())
    }

    /// Get the current URL
    #[instrument(skip(self))]
    pub async fn url(&self) -> Result<String> {
        let result = self
            .page
            .url()
            .await
            .map_err(|e| Error::Browser(format!("get url: {e}")))?;
        Ok(result.unwrap_or_default())
    }

    /// Execute JavaScript and return the result
    #[instrument(skip(self))]
    pub async fn execute_script(&self, js: &str) -> Result<serde_json::Value> {
        let result = self
            .page
            .evaluate(js)
            .await
            .map_err(|e| Error::Browser(format!("execute script: {e}")))?;
        Ok(result.value().cloned().unwrap_or(serde_json::Value::Null))
    }

    /// Take a screenshot and save to file
    #[instrument(skip(self))]
    pub async fn screenshot(&self, path: &str) -> Result<()> {
        let bytes = self.screenshot_bytes().await?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    /// Take a screenshot and return PNG bytes
    #[instrument(skip(self))]
    pub async fn screenshot_bytes(&self) -> Result<Vec<u8>> {
        use chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotParams;
        let params = CaptureScreenshotParams::builder()
            .format(chromiumoxide::cdp::browser_protocol::page::CaptureScreenshotFormat::Png)
            .build();
        self.page
            .screenshot(params)
            .await
            .map_err(|e| Error::Browser(format!("screenshot: {e}")))
    }

    /// Get all browser cookies
    #[instrument(skip(self))]
    pub async fn cookies(&self) -> Result<Vec<CookieInfo>> {
        let cookies = self
            .page
            .get_cookies()
            .await
            .map_err(|e| Error::Browser(format!("get cookies: {e}")))?;

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

    /// Set a cookie in the browser
    #[instrument(skip(self))]
    pub async fn set_cookie(&self, cookie: CookieInfo) -> Result<()> {
        let mut cp = CookieParam::new(&cookie.name, &cookie.value);
        if let Some(ref domain) = cookie.domain {
            cp.domain = Some(domain.clone());
        }
        if let Some(ref path) = cookie.path {
            cp.path = Some(path.clone());
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

    /// Get all tabs / pages
    #[instrument(skip(self))]
    pub async fn tabs(&self) -> Result<Vec<Page>> {
        self.browser
            .pages()
            .await
            .map_err(|e| Error::Browser(format!("get pages: {e}")))
    }

    /// Open a new tab
    #[instrument(skip(self))]
    pub async fn new_tab(&self) -> Result<Page> {
        self.browser
            .new_page("about:blank")
            .await
            .map_err(|e| Error::Browser(format!("new tab: {e}")))
    }

    /// Refresh the current page
    #[instrument(skip(self))]
    pub async fn refresh(&self) -> Result<()> {
        self.page
            .reload()
            .await
            .map_err(|e| Error::Browser(format!("refresh: {e}")))?;
        Ok(())
    }

    /// Go back in browser history
    #[instrument(skip(self))]
    pub async fn back(&self) -> Result<()> {
        self.page
            .evaluate("history.back()")
            .await
            .map_err(|e| Error::Browser(format!("back: {e}")))?;
        Ok(())
    }

    /// Go forward in browser history
    #[instrument(skip(self))]
    pub async fn forward(&self) -> Result<()> {
        self.page
            .evaluate("history.forward()")
            .await
            .map_err(|e| Error::Browser(format!("forward: {e}")))?;
        Ok(())
    }

    /// Get a reference to the download manager
    pub fn download_manager(&self) -> &Arc<DownloadManager> {
        &self.download_manager
    }

    /// Get a reference to the inner CDP page
    pub fn inner_page(&self) -> &Page {
        &self.page
    }

    /// Get a reference to the browser
    pub fn browser(&self) -> &Browser {
        &self.browser
    }

    /// Get a reference to the options
    pub fn options(&self) -> &ChromiumOptions {
        &self.opts
    }
}
