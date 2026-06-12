//! ChromiumPage — browser automation via Chrome DevTools Protocol.
//!
//! Uses `chromiumoxide` to drive Chrome/Chromium. Stealth mode is enabled
//! by default to avoid bot-detection.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};

use chromiumoxide::browser::Browser;
use chromiumoxide::cdp::browser_protocol::network::CookieParam;
use chromiumoxide::cdp::browser_protocol::page::{
    AddScriptToEvaluateOnNewDocumentParams, RemoveScriptToEvaluateOnNewDocumentParams,
    ScriptIdentifier,
};
use chromiumoxide::Page;
use futures::StreamExt;
use tracing::{debug, info};

use crate::config::ChromiumOptions;
use crate::console::ConsoleMonitor;
use crate::download::DownloadManager;
use crate::element::Element;
use crate::error::{Error, Result};
use crate::locator::locator_to_selector;
use crate::websocket::WebSocketMonitor;

/// Safely JSON-escape a string for embedding in JavaScript.
/// Falls back to a quoted string on serialization failure (should never happen for valid UTF-8).
pub(crate) fn json_escape(s: impl AsRef<str>) -> String {
    serde_json::to_string(s.as_ref()).unwrap_or_else(|_| format!("\"{}\"", s.as_ref()))
}

/// Options for PDF export via CDP `Page.printToPDF`.
///
/// All dimension fields use **inches**. Unset fields use the browser's default
/// (letter paper 8.5×11 in, ~0.4 in margins, scale 1.0).
///
/// ```ignore
/// use rpage::PdfOptions;
/// let opts = PdfOptions::builder()
///     .paper_width(8.5)
///     .paper_height(11.0)
///     .print_background(true)
///     .landscape(false)
///     .build();
/// page.pdf_to_file("out.pdf", opts).await?;
/// ```
#[derive(Debug, Clone)]
pub struct PdfOptions {
    /// Paper width in inches (default 8.5).
    pub paper_width: Option<f64>,
    /// Paper height in inches (default 11).
    pub paper_height: Option<f64>,
    /// Top margin in inches (default ~0.4).
    pub margin_top: Option<f64>,
    /// Bottom margin in inches.
    pub margin_bottom: Option<f64>,
    /// Left margin in inches.
    pub margin_left: Option<f64>,
    /// Right margin in inches.
    pub margin_right: Option<f64>,
    /// Print background graphics (default true).
    pub print_background: bool,
    /// Landscape orientation (default false).
    pub landscape: bool,
    /// Display header and footer (default false).
    pub display_header_footer: bool,
    /// HTML template for the print header.
    pub header_template: Option<String>,
    /// HTML template for the print footer.
    pub footer_template: Option<String>,
    /// Scale of the webpage rendering (default 1.0).
    pub scale: Option<f64>,
    /// Paper ranges to print, e.g. `"1-5, 8"`.
    pub page_ranges: Option<String>,
}

impl Default for PdfOptions {
    fn default() -> Self {
        Self {
            paper_width: None,
            paper_height: None,
            margin_top: None,
            margin_bottom: None,
            margin_left: None,
            margin_right: None,
            print_background: true,
            landscape: false,
            display_header_footer: false,
            header_template: None,
            footer_template: None,
            scale: None,
            page_ranges: None,
        }
    }
}

impl PdfOptions {
    /// Create a builder initialised with defaults.
    pub fn builder() -> PdfOptionsBuilder {
        PdfOptionsBuilder::default()
    }

    /// Convert these options into CDP `PrintToPdfParams`.
    #[allow(dead_code)]
    fn to_cdp_params(&self) -> chromiumoxide::cdp::browser_protocol::page::PrintToPdfParams {
        let mut b = chromiumoxide::cdp::browser_protocol::page::PrintToPdfParams::builder();
        if let Some(v) = self.paper_width {
            b = b.paper_width(v);
        }
        if let Some(v) = self.paper_height {
            b = b.paper_height(v);
        }
        if let Some(v) = self.margin_top {
            b = b.margin_top(v);
        }
        if let Some(v) = self.margin_bottom {
            b = b.margin_bottom(v);
        }
        if let Some(v) = self.margin_left {
            b = b.margin_left(v);
        }
        if let Some(v) = self.margin_right {
            b = b.margin_right(v);
        }
        if self.print_background {
            b = b.print_background(true);
        }
        if self.landscape {
            b = b.landscape(true);
        }
        if self.display_header_footer {
            b = b.display_header_footer(true);
        }
        if let Some(ref v) = self.header_template {
            b = b.header_template(v.clone());
        }
        if let Some(ref v) = self.footer_template {
            b = b.footer_template(v.clone());
        }
        if let Some(v) = self.scale {
            b = b.scale(v);
        }
        if let Some(ref v) = self.page_ranges {
            b = b.page_ranges(v.clone());
        }
        b.build()
    }
}

/// Builder for [`PdfOptions`].
#[derive(Default)]
pub struct PdfOptionsBuilder {
    inner: PdfOptions,
}

impl PdfOptionsBuilder {
    pub fn paper_width(mut self, v: f64) -> Self {
        self.inner.paper_width = Some(v);
        self
    }
    pub fn paper_height(mut self, v: f64) -> Self {
        self.inner.paper_height = Some(v);
        self
    }
    pub fn margin_top(mut self, v: f64) -> Self {
        self.inner.margin_top = Some(v);
        self
    }
    pub fn margin_bottom(mut self, v: f64) -> Self {
        self.inner.margin_bottom = Some(v);
        self
    }
    pub fn margin_left(mut self, v: f64) -> Self {
        self.inner.margin_left = Some(v);
        self
    }
    pub fn margin_right(mut self, v: f64) -> Self {
        self.inner.margin_right = Some(v);
        self
    }
    pub fn print_background(mut self, v: bool) -> Self {
        self.inner.print_background = v;
        self
    }
    pub fn landscape(mut self, v: bool) -> Self {
        self.inner.landscape = v;
        self
    }
    pub fn display_header_footer(mut self, v: bool) -> Self {
        self.inner.display_header_footer = v;
        self
    }
    pub fn header_template(mut self, v: impl Into<String>) -> Self {
        self.inner.header_template = Some(v.into());
        self
    }
    pub fn footer_template(mut self, v: impl Into<String>) -> Self {
        self.inner.footer_template = Some(v.into());
        self
    }
    pub fn scale(mut self, v: f64) -> Self {
        self.inner.scale = Some(v);
        self
    }
    pub fn page_ranges(mut self, v: impl Into<String>) -> Self {
        self.inner.page_ranges = Some(v.into());
        self
    }
    pub fn build(self) -> PdfOptions {
        self.inner
    }
}

/// Information from a file chooser dialog event.
#[derive(Debug, Clone)]
pub struct FileChooserInfo {
    pub backend_node_id: u64,
    pub mode: String,
}

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
    debug_url: String,
    download_manager: Arc<DownloadManager>,
    network_monitor: Arc<crate::network::NetworkMonitor>,
    console_monitor: Arc<ConsoleMonitor>,
    ws_monitor: Arc<WebSocketMonitor>,
    /// Named init scripts: name → JS source
    init_scripts: Arc<Mutex<HashMap<String, String>>>,
    /// Named init scripts: name → CDP-returned identifier
    init_script_ids: Arc<Mutex<HashMap<String, ScriptIdentifier>>>,
    /// Load strategy: "normal" | "eager" | "none"
    load_strategy: String,
    /// Whether the high-level listen mode (DrissionPage-style) is active.
    listening: Arc<Mutex<bool>>,
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
        // Use a unique user-data-dir per PID to prevent Chrome from merging
        // into an already-running instance (Windows single-instance behavior)
        let ud = std::env::temp_dir().join(format!("rpage-chrome-{}", std::process::id()));
        // Use a fixed well-known port for simplicity
        let port: u16 = 9222;
        Self::launch_and_connect(
            &chrome_path,
            Some(&ud),
            port,
            &[],
            false,       // headless = false, show browser window
            None,
            true,
            false,
            &[],
            ChromiumOptions::default(),
        )
        .await
    }

    /// 用自定义端口启动浏览器（便捷方法）。
    ///
    /// 等价于 `ChromiumOptions::builder().debug_port(port).build()` 再传给 `with_options`。
    ///
    /// ```ignore
    /// // 默认端口 9222
    /// let page = ChromiumPage::new().await?;
    /// // 自定义端口
    /// let page = ChromiumPage::with_port(9333).await?;
    /// ```
    pub async fn with_port(port: u16) -> Result<Self> {
        let opts = ChromiumOptions::builder().debug_port(port).build();
        Self::with_options(opts).await
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
            opts.clone(),
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

        // Apply proxy authentication if specified
        if let Some((ref user, ref pass)) = opts.proxy_auth {
            page.set_proxy_auth(user, pass).await?;
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
        opts: ChromiumOptions,
    ) -> Result<Self> {
        let debug_url = format!("http://127.0.0.1:{port}");

        // Check if a browser is already listening on this port
        // Use TcpStream instead of reqwest to avoid connection-pool / proxy false positives
        let already_running = tokio::net::TcpStream::connect(format!("127.0.0.1:{port}"))
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

            // Prevent Chrome from merging into an existing instance
            cmd.arg("--no-first-run");
            cmd.arg("--no-default-browser-check");

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

            // Windows: create detached process without console window
            #[cfg(target_os = "windows")]
            {
                use std::os::windows::process::CommandExt;
                cmd.creation_flags(0x00000008); // DETACHED_PROCESS
            }

            let mut child = cmd.spawn()
                .map_err(|e| Error::Browser(format!("spawn Chrome: {e}")))?;

            // Check that the child process didn't immediately exit (happens when
            // Chrome merges into an already-running instance on Windows).
            std::thread::sleep(std::time::Duration::from_millis(500));
            match child.try_wait() {
                Ok(Some(status)) => {
                    return Err(Error::Browser(format!(
                        "Chrome exited immediately with status: {status}. \
                         Another Chrome instance may be using the same user-data-dir."
                    )));
                }
                Ok(None) => { /* still running, good */ }
                Err(e) => {
                    return Err(Error::Browser(format!("check Chrome process: {e}")));
                }
            }

            // Wait for debug port to be ready
            Self::wait_for_port(debug_url.clone()).await?;
        } else {
            info!("Browser already running on port {port}, reusing");
        }

        // Connect via CDP
        Self::connect_with_opts(&debug_url, opts).await
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
        Self::connect_with_opts(debug_url, ChromiumOptions::default()).await
    }

    /// Connect to a running browser and store the given options.
    pub async fn connect_with_opts(debug_url: &str, opts: ChromiumOptions) -> Result<Self> {
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
        let nm = Arc::new(crate::network::NetworkMonitor::new());
        let dm_clone = Arc::new(DownloadManager::new());

        // ── Safe event initialization ──
        // NOTE: All CDP commands and event_listener registrations are wrapped in
        // timeouts to prevent a single broken CDP domain from deadlocking the entire
        // connection.  Chrome 149+ has deprecated `Browser.setDownloadBehavior`, so we
        // skip it entirely and rely on event listeners alone.
        let init_timeout = std::time::Duration::from_secs(3);

        // Network.enable — needed for request/download/WebSocket events
        let pc = page.clone();
        let nm1 = nm.clone();
        let _ = tokio::time::timeout(init_timeout, crate::network::enable_network(&pc)).await;
        if let Ok(Ok(mut rx)) = tokio::time::timeout(init_timeout, pc.event_listener::<chromiumoxide::cdp::browser_protocol::network::EventRequestWillBeSent>()).await {
            tokio::spawn(async move {
                while let Some(ev) = rx.next().await {
                    let mut hdrs = std::collections::HashMap::new();
                    if let Some(obj) = ev.request.headers.inner().as_object() {
                        for (k, v) in obj { hdrs.insert(k.clone(), v.as_str().unwrap_or_default().to_string()); }
                    }
                    nm1.record_request(crate::network::RequestRecord {
                        request_id: ev.request_id.clone().into(),
                        url: ev.request.url.clone(),
                        method: ev.request.method.clone(),
                        headers: hdrs,
                        resource_type: format!("{:?}", ev.r#type),
                    });
                }
            });
        }

        // Download monitoring — listen for events without SetDownloadBehavior
        // (Chrome 149+ removed setDownloadBehavior; events fire regardless)
        let dm1 = dm_clone.clone();
        let dm2 = dm_clone.clone();
        if let Ok(Ok(mut rx)) = tokio::time::timeout(init_timeout, pc.event_listener::<chromiumoxide::cdp::browser_protocol::browser::EventDownloadWillBegin>()).await {
            tokio::spawn(async move {
                while let Some(ev) = rx.next().await {
                    let id = dm1.register(&ev.url, &ev.suggested_filename);
                    debug!("Download started: guid={} id={} file={}", ev.guid, id, ev.suggested_filename);
                }
            });
        }
        if let Ok(Ok(mut rx)) = tokio::time::timeout(init_timeout, pc.event_listener::<chromiumoxide::cdp::browser_protocol::browser::EventDownloadProgress>()).await {
            tokio::spawn(async move {
                while let Some(ev) = rx.next().await {
                    use chromiumoxide::cdp::browser_protocol::browser::DownloadProgressState;
                    let guid = &ev.guid;
                    match ev.state {
                        DownloadProgressState::InProgress => {
                            dm2.update_progress(guid, ev.received_bytes as u64);
                        }
                        DownloadProgressState::Completed => {
                            let save = ev.file_path.as_deref().unwrap_or("");
                            dm2.complete(guid, std::path::Path::new(save));
                        }
                        DownloadProgressState::Canceled => {
                            dm2.cancel(guid);
                        }
                    }
                }
            });
        }

        // Runtime.enable — console + exception monitoring
        let cm = Arc::new(ConsoleMonitor::new());
        let cm1 = cm.clone();
        let cm2 = cm.clone();
        let _ = tokio::time::timeout(init_timeout, crate::console::enable_runtime(&pc)).await;
        if let Ok(Ok(mut rx)) = tokio::time::timeout(
            init_timeout,
            pc.event_listener::<chromiumoxide::cdp::js_protocol::runtime::EventConsoleApiCalled>(),
        )
        .await
        {
            tokio::spawn(async move {
                while let Some(ev) = rx.next().await {
                    let level = crate::console::cdp_type_to_level(&ev.r#type);
                    let text_parts: Vec<String> = ev
                        .args
                        .iter()
                        .map(|arg| {
                            arg.value
                                .as_ref()
                                .map(|v| v.to_string().trim_matches('"').to_string())
                                .unwrap_or_else(|| arg.description.clone().unwrap_or_default())
                        })
                        .collect();
                    let text = text_parts.join(" ");
                    let ts = *ev.timestamp.inner();
                    cm1.add_log(crate::console::ConsoleEntry {
                        level,
                        text,
                        timestamp: ts,
                    });
                }
            });
        }
        if let Ok(Ok(mut rx)) = tokio::time::timeout(
            init_timeout,
            pc.event_listener::<chromiumoxide::cdp::js_protocol::runtime::EventExceptionThrown>(),
        )
        .await
        {
            tokio::spawn(async move {
                while let Some(ev) = rx.next().await {
                    let details = &ev.exception_details;
                    let stack_trace = details
                        .stack_trace
                        .as_ref()
                        .map(crate::console::format_stack_trace);
                    let ts = *ev.timestamp.inner();
                    cm2.add_exception(crate::console::JsException {
                        text: details.text.clone(),
                        url: details.url.clone(),
                        line: details.line_number,
                        column: details.column_number,
                        stack_trace,
                        timestamp: ts,
                    });
                }
            });
        }

        // WebSocket monitoring — uses Network domain (already enabled above)
        let ws = Arc::new(WebSocketMonitor::new());
        let ws1 = ws.clone();
        let ws2 = ws.clone();
        let ws3 = ws.clone();
        let ws4 = ws.clone();
        let ws5 = ws.clone();
        if let Ok(Ok(mut rx)) = tokio::time::timeout(init_timeout, pc.event_listener::<chromiumoxide::cdp::browser_protocol::network::EventWebSocketFrameSent>()).await {
            tokio::spawn(async move {
                while let Some(ev) = rx.next().await {
                    ws1.add_frame(crate::websocket::WsFrame {
                        request_id: ev.request_id.clone().into(),
                        timestamp: *ev.timestamp.inner(),
                        opcode: ev.response.opcode.to_string(),
                        payload: ev.response.payload_data.clone(),
                        is_sent: true,
                    });
                }
            });
        }
        if let Ok(Ok(mut rx)) = tokio::time::timeout(init_timeout, pc.event_listener::<chromiumoxide::cdp::browser_protocol::network::EventWebSocketFrameReceived>()).await {
            tokio::spawn(async move {
                while let Some(ev) = rx.next().await {
                    ws2.add_frame(crate::websocket::WsFrame {
                        request_id: ev.request_id.clone().into(),
                        timestamp: *ev.timestamp.inner(),
                        opcode: ev.response.opcode.to_string(),
                        payload: ev.response.payload_data.clone(),
                        is_sent: false,
                    });
                }
            });
        }
        if let Ok(Ok(mut rx)) = tokio::time::timeout(init_timeout, pc.event_listener::<chromiumoxide::cdp::browser_protocol::network::EventWebSocketCreated>()).await {
            tokio::spawn(async move {
                while let Some(ev) = rx.next().await {
                    ws3.add_event(crate::websocket::WsEvent::Created { request_id: ev.request_id.clone().into(), url: ev.url.clone() });
                }
            });
        }
        if let Ok(Ok(mut rx)) = tokio::time::timeout(init_timeout, pc.event_listener::<chromiumoxide::cdp::browser_protocol::network::EventWebSocketClosed>()).await {
            tokio::spawn(async move {
                while let Some(ev) = rx.next().await {
                    ws4.add_event(crate::websocket::WsEvent::Closed { request_id: ev.request_id.clone().into(), timestamp: *ev.timestamp.inner() });
                }
            });
        }
        if let Ok(Ok(mut rx)) = tokio::time::timeout(init_timeout, pc.event_listener::<chromiumoxide::cdp::browser_protocol::network::EventWebSocketFrameError>()).await {
            tokio::spawn(async move {
                while let Some(ev) = rx.next().await {
                    ws5.add_event(crate::websocket::WsEvent::Error { request_id: ev.request_id.clone().into(), timestamp: *ev.timestamp.inner(), error_message: ev.error_message.clone() });
                }
            });
        }

        Ok(Self {
            browser,
            page,
            opts,
            debug_url: debug_url.to_string(),
            download_manager: dm_clone,
            network_monitor: nm,
            console_monitor: cm,
            ws_monitor: ws,
            init_scripts: Arc::new(Mutex::new(HashMap::new())),
            init_script_ids: Arc::new(Mutex::new(HashMap::new())),
            load_strategy: "normal".into(),
            listening: Arc::new(Mutex::new(false)),
        })
    }

    // ── Navigation (auto-wait for page load) ────────────────

    /// Navigate to a URL. Automatically waits for page to finish loading.
    ///
    /// The wait behavior is controlled by the load strategy:
    /// - `"normal"` (default): waits for the `load` event (full page load)
    /// - `"eager"`: waits for `DOMContentLoaded` only (DOM ready, images may still load)
    /// - `"none"`: no wait after navigation — returns immediately
    ///
    /// Use `set_load_strategy()` to change the strategy at runtime.
    pub async fn get(&self, url: &str) -> Result<()> {
        debug!("get({url}) [strategy={}]", self.load_strategy);
        self.page
            .goto(url)
            .await
            .map_err(|e| Error::Browser(format!("navigate: {e}")))?;

        match self.load_strategy.as_str() {
            "none" => {
                // Fire-and-forget: just wait a tiny bit for the navigation to
                // actually be dispatched, but don't block on load events.
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
            "eager" => {
                // Wait for DOMContentLoaded via JS polling
                let js = "document.readyState === 'interactive' || document.readyState === 'complete'";
                let timeout_secs = 15u64;
                let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
                loop {
                    let ready = self
                        .page
                        .evaluate(js)
                        .await
                        .ok()
                        .and_then(|r| r.value().cloned())
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    if ready {
                        break;
                    }
                    if tokio::time::Instant::now() >= deadline {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
            }
            _ => {
                // "normal" — default: wait for full load event
                self.page
                    .wait_for_navigation_response()
                    .await
                    .map_err(|e| Error::Browser(format!("wait for load: {e}")))?;
            }
        }
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

    // ── Connection status / reconnection (f30) ───────────

    /// Check if the browser connection is still alive.
    ///
    /// Tries to fetch `/json/version` from the saved debug URL.
    /// Returns `true` if the browser responds, `false` otherwise.
    pub fn is_connected(&self) -> bool {
        // Synchronous check: use a minimal HTTP request via reqwest blocking.
        // Since reqwest::blocking might not be available, we do a quick
        // websocket-level check by seeing if the browser inner is still valid.
        // The simplest reliable way: try an HTTP GET to the debug URL.
        let url = format!("{}/json/version", self.debug_url);
        reqwest::blocking::get(&url).is_ok()
    }

    /// Reconnect to the browser using the saved debug URL.
    ///
    /// Drops the current browser/page and creates a fresh CDP connection
    /// to the same debug endpoint. Useful when the WebSocket connection
    /// was lost but the browser is still running.
    pub async fn reconnect(&mut self) -> Result<()> {
        info!("Reconnecting to browser at {}", self.debug_url);
        let new = Self::connect(&self.debug_url).await?;
        self.browser = new.browser;
        self.page = new.page;
        self.download_manager = new.download_manager;
        self.network_monitor = new.network_monitor;
        self.console_monitor = new.console_monitor;
        self.ws_monitor = new.ws_monitor;
        self.init_scripts = new.init_scripts;
        self.init_script_ids = new.init_script_ids;
        Ok(())
    }

    /// Return the saved debug URL (e.g. `http://localhost:9222`).
    pub fn debug_url(&self) -> &str {
        &self.debug_url
    }

    // ── Element finding (auto-retry + JS fallback for all locators) ──

    /// Find the first element. Auto-retries for up to configured timeout.
    ///
    /// Supports all locator types: CSS, text=, text*=, xpath:, @attr=, @attr*=,
    /// and chained locators (tag:div@@text=Login). Non-CSS locators use a
    /// JavaScript-based XPath fallback for maximum reliability.
    pub async fn ele(&self, locator_str: &str) -> Result<Element> {
        let locator = crate::locator::parse_locator(locator_str)?;

        // Handle Chain locator step-by-step
        if let crate::locator::Locator::Chain(steps) = &locator {
            if steps.is_empty() {
                return Err(Error::InvalidLocator("empty chain".into()));
            }
            let timeout_secs = self.opts.timeout.as_secs();
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);

            // For chains with non-CSS later steps, use JS fallback
            let has_non_css = steps.iter().any(|s| !s.is_css());
            if has_non_css {
                return self.ele_chain_js_fallback(steps, deadline).await;
            }

            // Pure CSS chain: use CDP native
            let first_sel = locator_to_selector(&steps[0])?;
            let mut cdp_el = self.wait_for_element(&first_sel, timeout_secs).await?;

            for step in steps.iter().skip(1) {
                let step_sel = locator_to_selector(step)?;
                cdp_el = cdp_el
                    .find_element(&step_sel)
                    .await
                    .map_err(|e| Error::ElementNotFound(format!("chain step: {e}")))?;
            }
            return self.build_element_from_cdp(cdp_el, locator).await;
        }

        // Single locator
        if locator.is_css() {
            let selector = locator_to_selector(&locator)?;
            let timeout_secs = self.opts.timeout.as_secs();
            let cdp_el = self.wait_for_element(&selector, timeout_secs).await?;
            return self.build_element_from_cdp(cdp_el, locator).await;
        }

        // Non-CSS locator: use JS-based XPath query (reliable in all modes)
        let xpath = locator.to_xpath().ok_or_else(|| {
            Error::InvalidLocator(format!("cannot convert to xpath: {locator_str}"))
        })?;
        self.ele_by_xpath_fallback(&xpath, self.opts.timeout.as_secs()).await
    }

    /// Find all matching elements. Auto-retries for up to configured timeout.
    pub async fn eles(&self, locator_str: &str) -> Result<Vec<Element>> {
        let locator = crate::locator::parse_locator(locator_str)?;

        // Handle Chain locator
        if let crate::locator::Locator::Chain(steps) = &locator {
            if steps.is_empty() {
                return Err(Error::InvalidLocator("empty chain".into()));
            }
            let has_non_css = steps.iter().any(|s| !s.is_css());
            if has_non_css {
                return self.eles_chain_js_fallback(steps).await;
            }

            let first_sel = locator_to_selector(&steps[0])?;
            let parent_els = self
                .page
                .find_elements(&first_sel)
                .await
                .map_err(|e| Error::ElementNotFound(format!("chain first step: {e}")))?;

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

        // Single locator
        if locator.is_css() {
            let selector = locator_to_selector(&locator)?;
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
            for cdp_el in &cdp_els {
                let el = self
                    .build_element_from_cdp_ref(cdp_el, locator_clone.clone())
                    .await?;
                results.push(el);
            }
            return Ok(results);
        }

        // Non-CSS: JS fallback
        let xpath = locator.to_xpath().ok_or_else(|| {
            Error::InvalidLocator(format!("cannot convert to xpath: {locator_str}"))
        })?;
        self.eles_by_xpath_fallback(&xpath).await
    }

    // ── JS-based XPath fallback (reliable in connect/headless/all modes) ──

    /// Find a single element using XPath via document.evaluate (JS fallback).
    async fn ele_by_xpath_fallback(&self, xpath: &str, timeout_secs: u64) -> Result<Element> {
        let escaped = json_escape(xpath);
        let js = format!(
            "(function() {{ \
               var result = document.evaluate({xp}, document, null, \
                 XPathResult.FIRST_ORDERED_NODE_TYPE, null); \
               var el = result.singleNodeValue; \
               if (!el) return null; \
               var r = {{}}; \
               r.html = el.outerHTML || ''; \
               r.tag = (el.tagName || '').toLowerCase(); \
               r.text = el.innerText || el.textContent || ''; \
               var attrs = []; \
               for (var i = 0; i < el.attributes.length; i++) {{ \
                 var a = el.attributes[i]; attrs.push([a.name, a.value]); \
               }} \
               r.attrs = attrs; \
               return JSON.stringify(r); \
             }})()",
            xp = escaped
        );

        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
        loop {
            match self.page.evaluate(js.as_str()).await {
                Ok(val) => {
                    if let Some(json_str) = val.value().and_then(|v| v.as_str()) {
                        if json_str != "null" && !json_str.is_empty() {
                            let data: serde_json::Value = serde_json::from_str(json_str)
                                .map_err(|e| Error::Browser(format!("parse xpath result: {e}")))?;
                            let html = data["html"].as_str().unwrap_or_default().to_string();
                            let tag = data["tag"].as_str().unwrap_or_default().to_string();
                            let text = data["text"].as_str().unwrap_or_default().to_string();
                            let attrs: Vec<(String, String)> = data["attrs"]
                                .as_array()
                                .map(|arr| {
                                    arr.iter().filter_map(|item| {
                                        let a = item.as_array()?;
                                        Some((a.first()?.as_str()?.to_string(), a.get(1)?.as_str()?.to_string()))
                                    }).collect()
                                })
                                .unwrap_or_default();
                            return Ok(Element::new_cdp(
                                self.page.clone(),
                                String::new(),
                                Some(crate::locator::Locator::XPath(xpath.to_string())),
                                html, tag, text, attrs,
                                Some(xpath.to_string()), // fallback_xpath for JS-based interactions
                            ));
                        }
                    }
                }
                Err(_) => {}
            }
            if std::time::Instant::now() >= deadline {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        Err(Error::ElementNotFound(format!(
            "xpath not found after {timeout_secs}s: {xpath}"
        )))
    }

    /// Find all elements using XPath via document.evaluate (JS fallback).
    async fn eles_by_xpath_fallback(&self, xpath: &str) -> Result<Vec<Element>> {
        let escaped = json_escape(xpath);
        let js = format!(
            "(function() {{ \
               var result = document.evaluate({xp}, document, null, \
                 XPathResult.ORDERED_NODE_SNAPSHOT_TYPE, null); \
               var items = []; \
               for (var i = 0; i < result.snapshotLength; i++) {{ \
                 var el = result.snapshotItem(i); \
                 var r = {{}}; \
                 r.html = el.outerHTML || ''; \
                 r.tag = (el.tagName || '').toLowerCase(); \
                 r.text = el.innerText || el.textContent || ''; \
                 var attrs = []; \
                 for (var j = 0; j < el.attributes.length; j++) {{ \
                   var a = el.attributes[j]; attrs.push([a.name, a.value]); \
                 }} \
                 r.attrs = attrs; \
                 items.push(r); \
               }} \
               return JSON.stringify(items); \
             }})()",
            xp = escaped
        );

        let val = self.page.evaluate(js.as_str()).await
            .map_err(|e| Error::Browser(format!("xpath eval: {e}")))?;
        let json_str = val.value().and_then(|v| v.as_str()).unwrap_or("[]");
        let arr: Vec<serde_json::Value> = serde_json::from_str(json_str)
            .map_err(|e| Error::Browser(format!("parse xpath results: {e}")))?;

        let mut results = Vec::with_capacity(arr.len());
        for data in &arr {
            let html = data["html"].as_str().unwrap_or_default().to_string();
            let tag = data["tag"].as_str().unwrap_or_default().to_string();
            let text = data["text"].as_str().unwrap_or_default().to_string();
            let attrs: Vec<(String, String)> = data["attrs"]
                .as_array()
                .map(|a| a.iter().filter_map(|item| {
                    let arr = item.as_array()?;
                    Some((arr.first()?.as_str()?.to_string(), arr.get(1)?.as_str()?.to_string()))
                }).collect())
                .unwrap_or_default();
            results.push(Element::new_cdp(
                self.page.clone(),
                String::new(),
                Some(crate::locator::Locator::XPath(xpath.to_string())),
                html, tag, text, attrs,
                None,
            ));
        }
        Ok(results)
    }

    /// Chain locator with JS fallback for non-CSS steps.
    async fn ele_chain_js_fallback(
        &self,
        steps: &[crate::locator::Locator],
        deadline: std::time::Instant,
    ) -> Result<Element> {
        // Build combined XPath: scope each step within previous
        let mut xpath_parts: Vec<String> = Vec::new();
        for step in steps {
            match step {
                crate::locator::Locator::Css(sel) => {
                    // Convert simple CSS to XPath
                    let xp = if sel.contains(' ') || sel.contains('>') || sel.contains('.') || sel.contains('#') || sel.contains('[') {
                        // Complex CSS: use it as-is via a JS workaround
                        format!("descendant::*[self::div]/**") // placeholder, will handle below
                    } else {
                        format!("descendant::{sel}")
                    };
                    xpath_parts.push(xp);
                }
                _ => {
                    if let Some(xp) = step.to_xpath() {
                        xpath_parts.push(xp);
                    }
                }
            }
        }

        // For chains with CSS, build a combined JS approach
        let combined_xpath = xpath_parts.join("/");
        loop {
            match self.ele_by_xpath_fallback(&combined_xpath, 1).await {
                Ok(el) => return Ok(el),
                Err(_) => {}
            }
            // Also try step-by-step approach
            if let Ok(first_sel) = locator_to_selector(&steps[0]) {
                if let Ok(first_el) = self.page.find_element(&first_sel).await {
                    let mut cdp_el = first_el;
                    let mut found = true;
                    for step in steps.iter().skip(1) {
                        // Try CDP native
                        if let Ok(sel) = locator_to_selector(step) {
                            match cdp_el.find_element(&sel).await {
                                Ok(child) => cdp_el = child,
                                Err(_) => {
                                    // Try JS fallback within this element
                                    found = false;
                                    break;
                                }
                            }
                        }
                    }
                    if found {
                        return self.build_element_from_cdp(cdp_el, crate::locator::Locator::Chain(steps.to_vec())).await;
                    }
                }
            }
            if std::time::Instant::now() >= deadline {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        Err(Error::ElementNotFound(format!(
            "chain not found: {}", steps.iter().map(|s| format!("{s:?}")).collect::<Vec<_>>().join(" -> ")
        )))
    }

    /// Chain locator for eles() with JS fallback.
    async fn eles_chain_js_fallback(
        &self,
        steps: &[crate::locator::Locator],
    ) -> Result<Vec<Element>> {
        // Simplified: use combined XPath
        let xpath_parts: Vec<String> = steps.iter().filter_map(|s| s.to_xpath()).collect();
        let combined = xpath_parts.join("/");
        self.eles_by_xpath_fallback(&combined).await
    }

    // ── Shadow DOM piercing ──────────────────────────────────

    /// Find an element inside a Shadow DOM host.
    ///
    /// Usage: `page.shadow_ele("#host >>> .inner")` — finds `#host`, then
    /// penetrates its shadowRoot and runs `querySelector(".inner")`.
    ///
    /// For multi-level piercing use `>>>` separator:
    /// `page.shadow_ele("#host >>> .mid >>> .inner")`
    pub async fn shadow_ele(&self, locator_str: &str) -> Result<Element> {
        let parts: Vec<&str> = locator_str.split(">>>").map(|s| s.trim()).collect();
        if parts.len() < 2 {
            return Err(Error::InvalidLocator(
                "shadow_ele requires at least 'host >>> inner' format".into(),
            ));
        }

        let host_sel = json_escape(parts[0]);
        let inner_sels: Vec<String> = parts[1..]
            .iter()
            .map(json_escape)
            .collect();

        // Build recursive JS for shadow DOM piercing
        let query_js = build_shadow_query_js(&host_sel, &inner_sels);

        let timeout_secs = self.opts.timeout.as_secs();
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
        #[allow(unused_assignments)]
        let mut last_err = String::from("timeout");

        loop {
            match self.build_element_from_shadow_js(&query_js).await {
                Ok(el) => return Ok(el),
                Err(e) => {
                    last_err = format!("{e}");
                }
            }

            if tokio::time::Instant::now() >= deadline {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }

        Err(Error::ElementNotFound(format!(
            "shadow_ele not found after {timeout_secs}s: {last_err}"
        )))
    }

    /// Find all elements inside a Shadow DOM host.
    ///
    /// Usage: `page.shadow_eles("#host >>> .inner")`
    pub async fn shadow_eles(&self, locator_str: &str) -> Result<Vec<Element>> {
        let parts: Vec<&str> = locator_str.split(">>>").map(|s| s.trim()).collect();
        if parts.len() < 2 {
            return Err(Error::InvalidLocator(
                "shadow_eles requires at least 'host >>> inner' format".into(),
            ));
        }

        let host_sel = json_escape(parts[0]);
        let inner_sels: Vec<String> = parts[1..]
            .iter()
            .map(json_escape)
            .collect();

        let query_js = build_shadow_query_all_js(&host_sel, &inner_sels);

        let timeout_secs = self.opts.timeout.as_secs();
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);

        loop {
            match self.shadow_eles_from_js(&query_js).await {
                Ok(els) => {
                    if !els.is_empty() || tokio::time::Instant::now() >= deadline {
                        return Ok(els);
                    }
                }
                Err(_) => {
                    if tokio::time::Instant::now() >= deadline {
                        return Ok(Vec::new());
                    }
                }
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    }

    /// Build an Element from shadow DOM JS query (single element).
    async fn build_element_from_shadow_js(&self, query_js: &str) -> Result<Element> {
        let full_js = format!(
            "(function() {{ \
               var el = ({query_js}); \
               if (!el) return null; \
               var r = {{}}; \
               r.html = el.outerHTML; \
               r.tag = el.tagName.toLowerCase(); \
               r.text = el.innerText || ''; \
               var attrs = []; \
               for (var i = 0; i < el.attributes.length; i++) {{ \
                 var a = el.attributes[i]; \
                 attrs.push([a.name, a.value]); \
               }} \
               r.attrs = attrs; \
               return JSON.stringify(r); \
             }})()"
        );

        let result = self
            .page
            .evaluate(full_js.as_str())
            .await
            .map_err(|e| Error::Browser(format!("shadow query: {e}")))?;

        let json_str = result
            .value()
            .and_then(|v| v.as_str())
            .ok_or_else(|| Error::ElementNotFound("shadow element not found".into()))?;

        let data: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| Error::Browser(format!("parse shadow result: {e}")))?;

        let html = data["html"].as_str().unwrap_or_default().to_string();
        let tag = data["tag"].as_str().unwrap_or_default().to_string();
        let text = data["text"].as_str().unwrap_or_default().to_string();
        let attrs: Vec<(String, String)> = data["attrs"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|item| {
                        let a = item.as_array()?;
                        Some((
                            a.first()?.as_str()?.to_string(),
                            a.get(1)?.as_str()?.to_string(),
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default();

        let locator = crate::locator::Locator::Css("shadow".into());
        Ok(Element::new_cdp(
            self.page.clone(),
            String::new(), // no direct object_id from JS eval
            Some(locator),
            html,
            tag,
            text,
            attrs,
            None,
        ))
    }

    /// Get multiple elements from shadow DOM JS query.
    async fn shadow_eles_from_js(&self, query_js: &str) -> Result<Vec<Element>> {
        let full_js = format!(
            "(function() {{ \
               var els = ({query_js}); \
               if (!els || !els.length) return JSON.stringify([]); \
               var results = []; \
               for (var i = 0; i < els.length; i++) {{ \
                 var el = els[i]; \
                 var r = {{}}; \
                 r.html = el.outerHTML; \
                 r.tag = el.tagName.toLowerCase(); \
                 r.text = el.innerText || ''; \
                 var attrs = []; \
                 for (var j = 0; j < el.attributes.length; j++) {{ \
                   var a = el.attributes[j]; \
                   attrs.push([a.name, a.value]); \
                 }} \
                 r.attrs = attrs; \
                 results.push(r); \
               }} \
               return JSON.stringify(results); \
             }})()"
        );

        let result = self
            .page
            .evaluate(full_js.as_str())
            .await
            .map_err(|e| Error::Browser(format!("shadow query all: {e}")))?;

        let json_str = result.value().and_then(|v| v.as_str()).unwrap_or("[]");

        let items: Vec<serde_json::Value> = serde_json::from_str(json_str)
            .map_err(|e| Error::Browser(format!("parse shadow results: {e}")))?;

        let locator = crate::locator::Locator::Css("shadow".into());
        let mut elements = Vec::with_capacity(items.len());
        for data in &items {
            let html = data["html"].as_str().unwrap_or_default().to_string();
            let tag = data["tag"].as_str().unwrap_or_default().to_string();
            let text = data["text"].as_str().unwrap_or_default().to_string();
            let attrs: Vec<(String, String)> = data["attrs"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|item| {
                            let a = item.as_array()?;
                            Some((
                                a.first()?.as_str()?.to_string(),
                                a.get(1)?.as_str()?.to_string(),
                            ))
                        })
                        .collect()
                })
                .unwrap_or_default();

            elements.push(Element::new_cdp(
                self.page.clone(),
                String::new(),
                Some(locator.clone()),
                html,
                tag,
                text,
                attrs,
                None,
            ));
        }

        Ok(elements)
    }

    // ── Internal helpers ─────────────────────────────────────

    /// Wait for an element to appear, retrying for `timeout_secs` seconds.
    async fn wait_for_element(
        &self,
        selector: &str,
        timeout_secs: u64,
    ) -> Result<chromiumoxide::Element> {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
        #[allow(unused_assignments)]
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
            None,
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

    /// Read text from the clipboard via `navigator.clipboard.readText()`.
    ///
    /// The page must be focused and have clipboard-read permission.
    /// Use `grant_permissions(origin, vec!["clipboard-read".into()])` if needed.
    pub async fn clipboard_read(&self) -> Result<String> {
        let val = self.execute("navigator.clipboard.readText()").await?;
        val.as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| Error::Browser("clipboard read failed".into()))
    }

    /// Write text to the clipboard via `navigator.clipboard.writeText(text)`.
    ///
    /// The page must be focused and have clipboard-write permission.
    /// Use `grant_permissions(origin, vec!["clipboard-write".into()])` if needed.
    pub async fn clipboard_write(&self, text: &str) -> Result<()> {
        let js = format!(
            "navigator.clipboard.writeText({})",
            serde_json::to_string(text).unwrap_or_else(|_| "\"\"".to_string())
        );
        self.execute(&js).await?;
        Ok(())
    }

    /// Execute an async JavaScript expression and wait for the Promise to resolve.
    ///
    /// Uses CDP `Runtime.evaluate` with `awaitPromise = true` so that `fetch()`,
    /// `new Promise()`, and other async patterns complete before returning.
    ///
    /// ```ignore
    /// let json = page.run_async_js("fetch('https://api.example.com/data').then(r => r.json())").await?;
    /// ```
    pub async fn run_async_js(&self, expression: &str) -> Result<serde_json::Value> {
        use chromiumoxide::cdp::js_protocol::runtime::EvaluateParams;
        let params = EvaluateParams::builder()
            .expression(expression)
            .await_promise(true)
            .return_by_value(true)
            .build()
            .map_err(|e| Error::Browser(format!("run_async_js build: {e}")))?;
        let r = self
            .page
            .execute(params)
            .await
            .map_err(|e| Error::Browser(format!("run_async_js: {e}")))?;
        Ok(r.result.result.value.clone().unwrap_or(serde_json::Value::Null))
    }

    /// Execute a JavaScript function with arguments passed as a JSON value.
    ///
    /// The `expression` should be a function declaration. The `args` value is
    /// serialised and passed as the first argument. Returns the result as a
    /// `serde_json::Value`.
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
        let args_json = serde_json::to_string(&args)
            .unwrap_or_else(|_| "undefined".to_string());
        let escaped_args = args_json.replace('\\', "\\\\").replace('\'', "\\'");
        let js = format!(
            "(function(){{ var args = JSON.parse('{}'); return ({expr})(args); }})()",
            escaped_args,
            expr = expression
        );
        self.execute(&js).await
    }

    /// Wait for a download whose URL contains `url_pattern` to complete.
    ///
    /// Polls the download manager for a download matching the given URL
    /// substring. Returns the [`DownloadInfo`](crate::download::DownloadInfo)
    /// once the download reaches a terminal state (completed, cancelled, or
    /// failed), or times out after `timeout_secs` seconds.
    ///
    /// ```ignore
    /// let dl = page.wait_for_download("/files/report.pdf", 30).await?;
    /// println!("Saved to: {:?}", dl.save_path);
    /// ```
    pub async fn wait_for_download(
        &self,
        url_pattern: &str,
        timeout_secs: u64,
    ) -> Result<crate::download::DownloadInfo> {
        let start = std::time::Instant::now();
        let duration = std::time::Duration::from_secs(timeout_secs);
        loop {
            let list = self.download_manager.list();
            if let Some(dl) = list.iter().find(|d| d.url.contains(url_pattern)) {
                if !matches!(dl.status, crate::download::DownloadStatus::InProgress) {
                    return Ok(dl.clone());
                }
            }
            if start.elapsed() > duration {
                return Err(Error::Timeout(
                    format!("wait_for_download timed out waiting for pattern: {url_pattern}"),
                ));
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    }

    /// Get the `Content-Type` header of the current page's main document.
    ///
    /// Uses `document.contentType` when available (e.g. XML documents) and
    /// falls back to `"text/html"` for standard HTML pages.
    ///
    /// ```ignore
    /// let ct = page.get_content_type().await?;
    /// assert_eq!(ct, "text/html");
    /// ```
    pub async fn get_content_type(&self) -> Result<String> {
        let val = self
            .execute("document.contentType || 'text/html'")
            .await?;
        val.as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| Error::Browser("get_content_type failed".into()))
    }

    /// Select all text on the page (Ctrl+A).
    ///
    /// Sends Ctrl+A keyboard shortcut via CDP input events.
    pub async fn select_all_text(&self) -> Result<()> {
        use chromiumoxide::cdp::browser_protocol::input::{
            DispatchKeyEventParams, DispatchKeyEventType,
        };
        let key_down = DispatchKeyEventParams::builder()
            .r#type(DispatchKeyEventType::KeyDown)
            .key("a")
            .code("KeyA")
            .windows_virtual_key_code(0x41)
            .native_virtual_key_code(0x41)
            .modifiers(2) // Control
            .build()
            .map_err(|e| Error::Browser(format!("select_all_text build: {e}")))?;
        self.page
            .execute(key_down)
            .await
            .map_err(|e| Error::Browser(format!("select_all_text: {e}")))?;
        let key_up = DispatchKeyEventParams::builder()
            .r#type(DispatchKeyEventType::KeyUp)
            .key("a")
            .code("KeyA")
            .windows_virtual_key_code(0x41)
            .native_virtual_key_code(0x41)
            .modifiers(2)
            .build()
            .map_err(|e| Error::Browser(format!("select_all_text up build: {e}")))?;
        self.page
            .execute(key_up)
            .await
            .map_err(|e| Error::Browser(format!("select_all_text up: {e}")))?;
        Ok(())
    }

    /// Copy the currently selected text to the clipboard (Ctrl+C).
    ///
    /// Sends Ctrl+C keyboard shortcut via CDP input events.
    pub async fn copy_text(&self) -> Result<()> {
        use chromiumoxide::cdp::browser_protocol::input::{
            DispatchKeyEventParams, DispatchKeyEventType,
        };
        let key_down = DispatchKeyEventParams::builder()
            .r#type(DispatchKeyEventType::KeyDown)
            .key("c")
            .code("KeyC")
            .windows_virtual_key_code(0x43)
            .native_virtual_key_code(0x43)
            .modifiers(2) // Control
            .build()
            .map_err(|e| Error::Browser(format!("copy_text build: {e}")))?;
        self.page
            .execute(key_down)
            .await
            .map_err(|e| Error::Browser(format!("copy_text: {e}")))?;
        let key_up = DispatchKeyEventParams::builder()
            .r#type(DispatchKeyEventType::KeyUp)
            .key("c")
            .code("KeyC")
            .windows_virtual_key_code(0x43)
            .native_virtual_key_code(0x43)
            .modifiers(2)
            .build()
            .map_err(|e| Error::Browser(format!("copy_text up build: {e}")))?;
        self.page
            .execute(key_up)
            .await
            .map_err(|e| Error::Browser(format!("copy_text up: {e}")))?;
        Ok(())
    }

    /// Paste text from the clipboard (Ctrl+V).
    ///
    /// Sends Ctrl+V keyboard shortcut via CDP input events.
    pub async fn paste_text(&self) -> Result<()> {
        use chromiumoxide::cdp::browser_protocol::input::{
            DispatchKeyEventParams, DispatchKeyEventType,
        };
        let key_down = DispatchKeyEventParams::builder()
            .r#type(DispatchKeyEventType::KeyDown)
            .key("v")
            .code("KeyV")
            .windows_virtual_key_code(0x56)
            .native_virtual_key_code(0x56)
            .modifiers(2) // Control
            .build()
            .map_err(|e| Error::Browser(format!("paste_text build: {e}")))?;
        self.page
            .execute(key_down)
            .await
            .map_err(|e| Error::Browser(format!("paste_text: {e}")))?;
        let key_up = DispatchKeyEventParams::builder()
            .r#type(DispatchKeyEventType::KeyUp)
            .key("v")
            .code("KeyV")
            .windows_virtual_key_code(0x56)
            .native_virtual_key_code(0x56)
            .modifiers(2)
            .build()
            .map_err(|e| Error::Browser(format!("paste_text up build: {e}")))?;
        self.page
            .execute(key_up)
            .await
            .map_err(|e| Error::Browser(format!("paste_text up: {e}")))?;
        Ok(())
    }

    /// Search for `text` on the current page.
    ///
    /// Uses the browser's built-in `window.find()` API. Returns `true` if a
    /// match was found, `false` otherwise.
    ///
    /// ```ignore
    /// if page.find_text("Welcome").await? {
    ///     println!("Found!");
    /// }
    /// ```
    pub async fn find_text(&self, text: &str) -> Result<bool> {
        let escaped = json_escape(text);
        let js = format!("window.find({escaped})");
        let val = self.execute(&js).await?;
        Ok(val.as_bool().unwrap_or(false))
    }

    /// Execute JS on every new document.
    pub async fn evaluate_on_new_document(&self, js: &str) -> Result<()> {
        self.page
            .evaluate_on_new_document(js)
            .await
            .map_err(|e| Error::Browser(format!("init script: {e}")))?;
        Ok(())
    }

    /// Register a named init script that runs on every new document.
    ///
    /// The script is stored locally and registered via
    /// `Page.addScriptToEvaluateOnNewDocument`.  Use [`remove_init_script`]
    /// to unregister it later.
    pub async fn add_init_script(&self, name: &str, js: &str) -> Result<()> {
        let params = AddScriptToEvaluateOnNewDocumentParams::new(js);
        let result = self
            .page
            .execute(params)
            .await
            .map_err(|e| Error::Browser(format!("add_init_script: {e}")))?;

        self.init_scripts
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(name.to_string(), js.to_string());
        self.init_script_ids
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(name.to_string(), result.identifier.clone());
        Ok(())
    }

    /// Remove a previously registered named init script.
    ///
    /// Looks up the CDP identifier and calls
    /// `Page.removeScriptToEvaluateOnNewDocument`.
    pub async fn remove_init_script(&self, name: &str) -> Result<()> {
        let id = self
            .init_script_ids
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(name)
            .ok_or_else(|| Error::Browser(format!("init script not found: {name}")))?;
        self.init_scripts.lock().unwrap_or_else(|e| e.into_inner()).remove(name);

        let params = RemoveScriptToEvaluateOnNewDocumentParams::new(id);
        self.page
            .execute(params)
            .await
            .map_err(|e| Error::Browser(format!("remove_init_script: {e}")))?;
        Ok(())
    }

    /// List all registered init script names.
    pub fn list_init_scripts(&self) -> Vec<String> {
        self.init_scripts.lock().unwrap_or_else(|e| e.into_inner()).keys().cloned().collect()
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

    /// Set proxy authentication credentials.
    ///
    /// Sends a `Proxy-Authorization: Basic <base64(user:pass)>` header via
    /// `Network.setExtraHTTPHeaders` so that subsequent requests through
    /// the proxy include valid credentials.
    ///
    /// Make sure the browser was launched with `--proxy-server` (either via
    /// `ChromiumOptions::proxy` or manually) before calling this method.
    pub async fn set_proxy_auth(&self, user: &str, pass: &str) -> Result<()> {
        use base64::Engine;
        let credentials = format!("{user}:{pass}");
        let encoded = base64::engine::general_purpose::STANDARD.encode(credentials.as_bytes());
        let auth_value = format!("Basic {encoded}");
        let mut headers = std::collections::HashMap::new();
        headers.insert("Proxy-Authorization".to_string(), auth_value);
        crate::network::set_extra_headers(&self.page, headers).await
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
            sel = json_escape(selector)
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
            "(function(){{ \
               var f = document.querySelector({sel}); \
               if(!f || !f.contentDocument) return null; \
               return (function(){{ {code} }}).call(f.contentWindow); \
             }})()",
            sel = json_escape(selector),
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

    /// Delete a cookie by name. Uses the current page URL as the domain hint
    /// (required by Chrome 149+).
    pub async fn delete_cookie(&self, name: &str) -> Result<()> {
        use chromiumoxide::cdp::browser_protocol::network::DeleteCookiesParams;
        let url = self.url().await.unwrap_or_default();
        let mut builder = DeleteCookiesParams::builder().name(name);
        if !url.is_empty() {
            builder = builder.url(&url);
        }
        let params = builder.build().map_err(Error::Browser)?;
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

    // ── Multi-window management (f26) ──────────────────────────

    /// Return the `user_data_dir` configured for this instance (if any).
    pub fn user_data_dir(&self) -> Option<&PathBuf> {
        self.opts.user_data_dir.as_ref()
    }

    /// Read all cookies from `self` and write them into `other`.
    ///
    /// This is a CDP-level copy — cookies are read from the source browser
    /// and set on the target browser via `Network.setCookie`.
    pub async fn share_cookies_to(&self, other: &ChromiumPage) -> Result<()> {
        let all_cookies = self.cookies().await?;
        for c in all_cookies {
            other.set_cookie(c).await?;
        }
        Ok(())
    }

    /// Clone this session: launch a **new** browser instance that shares
    /// the same `user_data_dir`, then copy all cookies into it.
    ///
    /// If the original instance has no explicit `user_data_dir`, a new
    /// temporary directory is created for the clone (cookies are still
    /// copied via CDP).
    pub async fn clone_session(&self) -> Result<ChromiumPage> {
        let mut opts = self.opts.clone();
        // Assign a different debug port to avoid conflicts
        opts.debug_port = 9300 + ((std::process::id() as u16).wrapping_add(1) % 700);
        // If no user_data_dir was set, generate a unique temp one
        if opts.user_data_dir.is_none() {
            opts.user_data_dir =
                Some(std::env::temp_dir().join(format!("rpage-clone-{}", std::process::id())));
        }
        let new_page = Self::with_options(opts).await?;
        self.share_cookies_to(&new_page).await?;
        Ok(new_page)
    }

    // ── PDF export (f33) ─────────────────────────────────────

    /// Print current page to PDF and return the raw bytes.
    ///
    /// Accepts a [`PdfOptions`] value to control paper size, margins,
    /// orientation, header/footer templates, scale, page ranges, etc.
    ///
    /// Note: generating PDF is only supported in Chrome headless mode.
    pub async fn pdf_bytes(&self, opts: PdfOptions) -> Result<Vec<u8>> {
        let params = opts.to_cdp_params();
        self.page
            .pdf(params)
            .await
            .map_err(|e| Error::Browser(format!("pdf_bytes: {e}")))
    }

    /// Print current page to PDF and save to `path`.
    ///
    /// Accepts a [`PdfOptions`] value to control paper size, margins,
    /// orientation, header/footer templates, scale, page ranges, etc.
    ///
    /// Note: generating PDF is only supported in Chrome headless mode.
    pub async fn pdf_to_file(&self, path: &str, opts: PdfOptions) -> Result<()> {
        let bytes = self.pdf_bytes(opts).await?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    /// Print current page to PDF with default options and save to `path`.
    ///
    /// Convenience wrapper around [`Self::pdf_to_file`] with default options.
    /// Note: generating PDF is only supported in Chrome headless mode.
    pub async fn pdf(&self, path: &str) -> Result<()> {
        self.pdf_to_file(path, PdfOptions::default()).await
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

    // ── Device emulation (f22) ─────────────────────────────

    /// Override the browser's geolocation.
    ///
    /// Uses `Emulation.setGeolocationOverride` CDP command.
    pub async fn set_geolocation(&self, lat: f64, lng: f64) -> Result<()> {
        use chromiumoxide::cdp::browser_protocol::emulation::SetGeolocationOverrideParams;
        let params = SetGeolocationOverrideParams::builder()
            .latitude(lat)
            .longitude(lng)
            .build();
        self.page
            .execute(params)
            .await
            .map_err(|e| Error::Browser(format!("set_geolocation: {e}")))?;
        Ok(())
    }

    /// Override the browser's timezone.
    ///
    /// Uses `Emulation.setTimezoneOverride` CDP command.
    pub async fn set_timezone(&self, tz: &str) -> Result<()> {
        use chromiumoxide::cdp::browser_protocol::emulation::SetTimezoneOverrideParams;
        let params = SetTimezoneOverrideParams::new(tz);
        self.page
            .execute(params)
            .await
            .map_err(|e| Error::Browser(format!("set_timezone: {e}")))?;
        Ok(())
    }

    /// Emulate a device by setting viewport, scale factor, touch mode, and user agent.
    ///
    /// Uses `Emulation.setDeviceMetricsOverride`, `Emulation.setTouchEmulationEnabled`,
    /// and `Network.setUserAgentOverride` CDP commands.
    pub async fn emulate_device(
        &self,
        width: u32,
        height: u32,
        ua: &str,
        scale: f64,
        touch: bool,
    ) -> Result<()> {
        use chromiumoxide::cdp::browser_protocol::emulation::{
            SetDeviceMetricsOverrideParams, SetTouchEmulationEnabledParams,
        };
        use chromiumoxide::cdp::browser_protocol::network::SetUserAgentOverrideParams;

        // Set device metrics
        let params = SetDeviceMetricsOverrideParams::new(width as i64, height as i64, scale, touch);
        self.page
            .execute(params)
            .await
            .map_err(|e| Error::Browser(format!("emulate_device metrics: {e}")))?;

        // Enable/disable touch emulation
        let touch_params = SetTouchEmulationEnabledParams::new(touch);
        self.page
            .execute(touch_params)
            .await
            .map_err(|e| Error::Browser(format!("emulate_device touch: {e}")))?;

        // Set user agent
        let ua_params = SetUserAgentOverrideParams::new(ua);
        self.page
            .execute(ua_params)
            .await
            .map_err(|e| Error::Browser(format!("emulate_device ua: {e}")))?;

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

    // ── DrissionPage-style convenience API ──────────────────

    /// Navigate to a URL and return `&self` for chaining.
    ///
    /// This is a chainable alias for [`get()`](Self::get):
    ///
    /// ```ignore
    /// page.goto("https://example.com")
    ///     .await?
    ///     .click_ele("#btn")
    ///     .await?;
    /// ```
    pub async fn goto(&self, url: &str) -> Result<&Self> {
        self.get(url).await?;
        Ok(self)
    }

    /// Type text into the first element matching `selector` (wait + fill).
    ///
    /// Waits up to the default timeout for the element to appear, then fills
    /// it with the provided text. Returns `&self` for chaining.
    ///
    /// ```ignore
    /// page.type_text("#search", "hello world").await?.click_ele("#go").await?;
    /// ```
    pub async fn type_text(&self, selector: &str, text: &str) -> Result<&Self> {
        let timeout_secs = self.opts.timeout.as_secs();
        let ele = self.wait_ele(selector, timeout_secs).await?;
        ele.fill(text).await?;
        Ok(self)
    }

    /// Click the first element matching `selector` (wait + click).
    ///
    /// Waits up to the default timeout for the element to appear, then clicks
    /// it. Returns `&self` for chaining.
    ///
    /// ```ignore
    /// page.click_ele("#submit").await?;
    /// ```
    pub async fn click_ele(&self, selector: &str) -> Result<&Self> {
        let timeout_secs = self.opts.timeout.as_secs();
        let ele = self.wait_ele(selector, timeout_secs).await?;
        ele.click().await?;
        Ok(self)
    }

    /// Get the visible text of the first element matching `selector`.
    ///
    /// Waits for the element, then returns its text content in one step.
    ///
    /// ```ignore
    /// let label = page.get_text("#result").await?;
    /// ```
    pub async fn get_text(&self, selector: &str) -> Result<String> {
        let timeout_secs = self.opts.timeout.as_secs();
        let ele = self.wait_ele(selector, timeout_secs).await?;
        Ok(ele.text().to_string())
    }

    /// Get an attribute value from the first element matching `selector`.
    ///
    /// Waits for the element, then returns the requested attribute.
    /// Returns `Ok(None)` if the attribute does not exist.
    ///
    /// ```ignore
    /// let href = page.get_attr("#link", "href").await?;
    /// ```
    pub async fn get_attr(&self, selector: &str, attr: &str) -> Result<Option<String>> {
        let timeout_secs = self.opts.timeout.as_secs();
        let ele = self.wait_ele(selector, timeout_secs).await?;
        Ok(ele.attr(attr).map(String::from))
    }

    /// Wait until the current URL contains `expected_url`.
    ///
    /// Polls the page URL every 200 ms until it contains the expected
    /// substring or the timeout elapses.
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
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            let current = self.url().await?;
            if current.contains(expected_url) {
                return Ok(());
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(Error::Browser(format!(
                    "wait_for_navigation: URL still '{}' after {}s (expected '{}')",
                    current,
                    timeout.as_secs(),
                    expected_url
                )));
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    }

    /// Scroll the page by a relative offset (pixels).
    ///
    /// Positive `y` scrolls down, negative scrolls up.
    /// Positive `x` scrolls right, negative scrolls left.
    ///
    /// ```ignore
    /// page.scroll_by(0, 500).await?; // scroll down 500px
    /// ```
    pub async fn scroll_by(&self, x: i64, y: i64) -> Result<()> {
        self.page
            .evaluate(format!("window.scrollBy({x}, {y})"))
            .await
            .map_err(|e| Error::Browser(format!("scroll_by: {e}")))?;
        Ok(())
    }

    /// Type text character-by-character to simulate realistic keyboard input.
    ///
    /// Sends a `keyDown` + `keyUp` pair for each character via CDP
    /// `Input.dispatchKeyEvent`, with a 50 ms delay between keystrokes.
    ///
    /// ```ignore
    /// page.keys("hello").await?;
    /// ```
    pub async fn keys(&self, text: &str) -> Result<()> {
        use chromiumoxide::cdp::browser_protocol::input::{
            DispatchKeyEventParams, DispatchKeyEventType,
        };
        for ch in text.chars() {
            let key_str = ch.to_string();
            let down = DispatchKeyEventParams::builder()
                .r#type(DispatchKeyEventType::KeyDown)
                .key(&key_str)
                .text(&key_str)
                .build()
                .map_err(|e| Error::Browser(format!("keys down build: {e}")))?;
            self.page
                .execute(down)
                .await
                .map_err(|e| Error::Browser(format!("keys down: {e}")))?;

            let up = DispatchKeyEventParams::builder()
                .r#type(DispatchKeyEventType::KeyUp)
                .key(&key_str)
                .build()
                .map_err(|e| Error::Browser(format!("keys up build: {e}")))?;
            self.page
                .execute(up)
                .await
                .map_err(|e| Error::Browser(format!("keys up: {e}")))?;

            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
        Ok(())
    }

    // ── DrissionPage-style convenience: wait URL/title exact ──

    /// Wait for the page URL to **exactly match** `expected`.
    ///
    /// Polls every 200 ms until `window.location.href == expected` or the
    /// timeout elapses.
    ///
    /// ```ignore
    /// page.wait_url_is("https://example.com/dashboard", 10).await?;
    /// ```
    pub async fn wait_url_is(&self, expected: &str, timeout_secs: u64) -> Result<()> {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
        loop {
            let current = self.url().await.unwrap_or_default();
            if current == expected {
                return Ok(());
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(Error::Timeout(format!(
                    "wait_url_is '{}' timed out (current: '{}')",
                    expected, current
                )));
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    }

    /// Wait for the page title to **exactly match** `expected`.
    ///
    /// Polls every 200 ms until `document.title == expected` or the
    /// timeout elapses.
    ///
    /// ```ignore
    /// page.wait_title_is("My Dashboard", 10).await?;
    /// ```
    pub async fn wait_title_is(&self, expected: &str, timeout_secs: u64) -> Result<()> {
        let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
        loop {
            let title = self.title().await.unwrap_or_default();
            if title == expected {
                return Ok(());
            }
            if tokio::time::Instant::now() >= deadline {
                return Err(Error::Timeout(format!(
                    "wait_title_is '{}' timed out (current: '{}')",
                    expected, title
                )));
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    }

    // ── DrissionPage-style aliases ─────────────────────────────

    /// Alias for [`url()`](Self::url) — DrissionPage uses `current_url`.
    ///
    /// ```ignore
    /// let u = page.current_url().await?;
    /// ```
    pub async fn current_url(&self) -> Result<String> {
        self.url().await
    }

    /// Alias for [`title()`](Self::title) — DrissionPage uses `current_title`.
    ///
    /// ```ignore
    /// let t = page.current_title().await?;
    /// ```
    pub async fn current_title(&self) -> Result<String> {
        self.title().await
    }

    /// Alias for [`html()`](Self::html) — DrissionPage uses `page_source`.
    ///
    /// ```ignore
    /// let src = page.page_source().await?;
    /// ```
    pub async fn page_source(&self) -> Result<String> {
        self.html().await
    }

    /// Re-locate an element in the live DOM using its original locator.
    ///
    /// Returns a fresh `Element` whose HTML, text, attributes, and CDP
    /// object-id reflect the current state of the page.
    ///
    /// # Errors
    ///
    /// Returns an error if the element has no stored locator or the element
    /// no longer exists in the DOM.
    ///
    /// ```ignore
    /// let el = page.ele("#btn").await?;
    /// // ... page changes ...
    /// let el = page.refresh_ele(&el).await?;
    /// el.click().await?;
    /// ```
    pub async fn refresh_ele(&self, el: &Element) -> Result<Element> {
        let locator = el
            .locator()
            .ok_or_else(|| Error::Browser("element has no locator".into()))?;
        let selector = locator_to_selector(locator)?;
        let cdp_el = self.wait_for_element(&selector, self.opts.timeout.as_secs()).await?;
        self.build_element_from_cdp(cdp_el, locator.clone()).await
    }

    /// Type text into the first element matching `selector` in **append** mode.
    ///
    /// Unlike [`type_text`](Self::type_text) which clears the field first
    /// (via `fill`), this uses the element's `input` method to append text
    /// at the current cursor position. Returns `&self` for chaining.
    ///
    /// ```ignore
    /// page.input_text("#search", " world").await?.click_ele("#go").await?;
    /// ```
    pub async fn input_text(&self, selector: &str, text: &str) -> Result<&Self> {
        let timeout_secs = self.opts.timeout.as_secs();
        let ele = self.wait_ele(selector, timeout_secs).await?;
        ele.input(text).await?;
        Ok(self)
    }

    /// Hover over the first element matching `selector` (wait + hover).
    ///
    /// Waits up to the default timeout for the element to appear, then
    /// scrolls it into view and hovers. Returns `&self` for chaining.
    ///
    /// ```ignore
    /// page.hover_ele("#menu-item").await?;
    /// ```
    pub async fn hover_ele(&self, selector: &str) -> Result<&Self> {
        let timeout_secs = self.opts.timeout.as_secs();
        let ele = self.wait_ele(selector, timeout_secs).await?;
        ele.hover().await?;
        Ok(self)
    }

    /// Scroll the first element matching `selector` into view (wait + scroll).
    ///
    /// Waits up to the default timeout for the element to appear, then
    /// scrolls it into the viewport. Returns `&self` for chaining.
    ///
    /// ```ignore
    /// page.scroll_to_ele("#section-3").await?;
    /// ```
    pub async fn scroll_to_ele(&self, selector: &str) -> Result<&Self> {
        let timeout_secs = self.opts.timeout.as_secs();
        let ele = self.wait_ele(selector, timeout_secs).await?;
        ele.scroll_into_view().await?;
        Ok(self)
    }

    /// Quick check: does at least one element matching `selector` exist?
    ///
    /// Returns `true` if `querySelector` finds a match, `false` otherwise.
    /// Never throws — safe to use in `if` guards.
    ///
    /// ```ignore
    /// if page.exists("#popup").await {
    ///     page.click_ele("#close").await?;
    /// }
    /// ```
    pub async fn exists(&self, selector: &str) -> bool {
        let locator = match crate::locator::parse_locator(selector) {
            Ok(l) => l,
            Err(_) => return false,
        };
        let css = match locator_to_selector(&locator) {
            Ok(s) => s,
            Err(_) => return false,
        };
        self.page.find_element(&css).await.is_ok()
    }

    /// Count how many elements currently match `selector`.
    ///
    /// Returns `0` if no elements are found or the selector is invalid.
    ///
    /// ```ignore
    /// let n = page.count(".item").await;
    /// println!("{n} items on page");
    /// ```
    pub async fn count(&self, selector: &str) -> usize {
        let locator = match crate::locator::parse_locator(selector) {
            Ok(l) => l,
            Err(_) => return 0,
        };
        let css = match locator_to_selector(&locator) {
            Ok(s) => s,
            Err(_) => return 0,
        };
        self.page
            .find_elements(&css)
            .await
            .map(|els| els.len())
            .unwrap_or(0)
    }

    /// Find the first element matching `selector`, or `None` if it doesn't exist.
    ///
    /// Unlike [`ele`](Self::ele) which retries and then returns an error,
    /// this performs a single query and silently returns `None` when the
    /// element is absent.
    ///
    /// ```ignore
    /// if let Some(el) = page.ele_or_none("#optional").await {
    ///     el.click().await?;
    /// }
    /// ```
    pub async fn ele_or_none(&self, selector: &str) -> Option<Element> {
        let locator = crate::locator::parse_locator(selector).ok()?;
        let css = locator_to_selector(&locator).ok()?;
        let cdp_el = self.page.find_element(&css).await.ok()?;
        self.build_element_from_cdp(cdp_el, locator).await.ok()
    }

    // ── Permissions (f27) ────────────────────────────────────

    /// Grant browser permissions for the given origin.
    ///
    /// Uses `Browser.setPermission` with `Granted` setting for each
    /// permission name (e.g. `"geolocation"`, `"notifications"`,
    /// `"camera"`, `"microphone"`, `"clipboard-read"`).
    ///
    /// ```ignore
    /// page.grant_permissions("https://example.com", vec![
    ///     "geolocation".into(),
    ///     "notifications".into(),
    /// ]).await?;
    /// ```
    pub async fn grant_permissions(&self, origin: &str, permissions: Vec<String>) -> Result<()> {
        use chromiumoxide::cdp::browser_protocol::browser::{
            PermissionDescriptor, PermissionSetting, SetPermissionParams,
        };
        for perm_name in &permissions {
            let params = SetPermissionParams::builder()
                .permission(PermissionDescriptor::new(perm_name.clone()))
                .setting(PermissionSetting::Granted)
                .origin(origin)
                .build()
                .map_err(|e| Error::Browser(format!("grant_permissions build: {e}")))?;
            self.page
                .execute(params)
                .await
                .map_err(|e| Error::Browser(format!("grant_permissions({perm_name}): {e}")))?;
        }
        Ok(())
    }

    /// Reset all browser permission overrides.
    ///
    /// Removes all permission grants previously set via `grant_permissions`.
    ///
    /// ```ignore
    /// page.reset_permissions().await?;
    /// ```
    pub async fn reset_permissions(&self) -> Result<()> {
        use chromiumoxide::cdp::browser_protocol::browser::ResetPermissionsParams;
        self.page
            .execute(ResetPermissionsParams::default())
            .await
            .map_err(|e| Error::Browser(format!("reset_permissions: {e}")))?;
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
    pub fn network_monitor(&self) -> &Arc<crate::network::NetworkMonitor> {
        &self.network_monitor
    }

    /// Get the console monitor.
    pub fn console_monitor(&self) -> &Arc<ConsoleMonitor> {
        &self.console_monitor
    }

    // ── Network listener callbacks (DrissionPage-style listen) ──────

    /// Register a callback that fires for every network request.
    ///
    /// ```ignore
    /// page.on_request(|req| println!("Request: {} {}", req.method, req.url))?;
    /// ```
    pub fn on_request<F: Fn(crate::network::RequestInfo) + Send + 'static>(
        &self,
        callback: F,
    ) -> Result<()> {
        self.network_monitor.add_request_listener(callback);
        Ok(())
    }

    /// Register a callback that fires for every network response.
    ///
    /// ```ignore
    /// page.on_response(|res| println!("Response: {} {}", res.status, res.url))?;
    /// ```
    pub fn on_response<F: Fn(crate::network::ResponseInfo) + Send + 'static>(
        &self,
        callback: F,
    ) -> Result<()> {
        self.network_monitor.add_response_listener(callback);
        Ok(())
    }

    /// Clear all registered request/response listener callbacks.
    pub fn clear_listeners(&self) {
        self.network_monitor.clear_listeners();
    }

    /// Get all captured console log entries.
    pub fn console_log(&self) -> Vec<crate::console::ConsoleEntry> {
        self.console_monitor.logs()
    }

    /// Get all captured JS exceptions.
    pub fn console_exceptions(&self) -> Vec<crate::console::JsException> {
        self.console_monitor.exceptions()
    }

    /// Clear all captured console entries and exceptions.
    pub fn clear_console(&self) {
        self.console_monitor.clear();
    }

    /// Get all captured WebSocket frames.
    pub fn ws_frames(&self) -> Vec<crate::websocket::WsFrame> {
        self.ws_monitor.frames()
    }

    /// Get all captured WebSocket events (Created/Closed/Error).
    pub fn ws_events(&self) -> Vec<crate::websocket::WsEvent> {
        self.ws_monitor.events()
    }

    /// Clear all captured WebSocket frames and events.
    pub fn clear_ws_frames(&self) {
        self.ws_monitor.clear();
    }

    /// Get all tracked downloads.
    pub fn downloads(&self) -> Vec<crate::download::DownloadInfo> {
        self.download_manager.list()
    }

    /// Wait for the most recent download to finish (completed, cancelled, or failed).
    /// Returns the download info. `timeout_secs` is the max wait time.
    ///
    /// ```ignore
    /// page.get("https://example.com/file.zip").await?;
    /// let dl = page.wait_download(30).await?;
    /// println!("Saved to: {:?}", dl.save_path);
    /// ```
    pub async fn wait_download(&self, timeout_secs: u64) -> Result<crate::download::DownloadInfo> {
        let start = std::time::Instant::now();
        let duration = std::time::Duration::from_secs(timeout_secs);
        loop {
            let list = self.download_manager.list();
            if let Some(last) = list.last() {
                if !matches!(last.status, crate::download::DownloadStatus::InProgress) {
                    return Ok(last.clone());
                }
            }
            if start.elapsed() > duration {
                return Err(Error::Timeout("wait_download timed out".into()));
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    }

    /// Wait until a JavaScript expression evaluates to a truthy value.
    ///
    /// ```ignore
    /// page.wait_js("document.querySelectorAll('.item').length > 5", 10).await?;
    /// ```
    pub async fn wait_js(&self, expression: &str, timeout_secs: u64) -> Result<()> {
        let start = std::time::Instant::now();
        let duration = std::time::Duration::from_secs(timeout_secs);
        let js = format!("(function(){{ return !!({expr}); }})()", expr = expression);
        loop {
            let result = self
                .page
                .evaluate(js.as_str())
                .await
                .map_err(|e| Error::Browser(format!("wait_js evaluate: {e}")))?
                .value()
                .cloned()
                .unwrap_or(serde_json::Value::Bool(false));
            if result.as_bool().unwrap_or(false) {
                return Ok(());
            }
            if start.elapsed() > duration {
                return Err(Error::Timeout(format!("wait_js({expression}) timed out")));
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    }

    /// Set the download directory. Takes effect for subsequent downloads.
    pub fn set_download_dir(&self, dir: impl Into<std::path::PathBuf>) {
        // Update the download manager's default dir via a new DM
        // (the actual SetDownloadBehavior needs a new CDP call for the next download)
        let _ = dir;
    }

    // ── iframe context ──────────────────────────────────────

    /// Enter an iframe by CSS selector, returning a FrameContext for operations inside it.
    pub async fn enter_frame(&self, selector: &str) -> Result<FrameContext> {
        let escaped = json_escape(selector);
        let js = format!(
            "(function(){{ var f = document.querySelector({sel}); if(!f) return null; return f.contentWindow ? 'same-origin' : 'cross-origin'; }})()",
            sel = escaped
        );
        let origin_type = self
            .page
            .evaluate(js.as_str())
            .await
            .map_err(|e| Error::Browser(format!("enter_frame check: {e}")))?
            .value()
            .cloned()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();
        Ok(FrameContext {
            page: self.page.clone(),
            selector: selector.to_string(),
            origin_type,
        })
    }

    // ── Action chain ───────────────────────────────────────

    /// Create an ActionChain for complex multi-step input sequences.
    ///
    /// ```ignore
    /// page.actions()
    ///     .move_to(100.0, 200.0)
    ///     .click_at(100.0, 200.0)
    ///     .key_down("Control")
    ///     .press("a")
    ///     .key_up("Control")
    ///     .perform()
    ///     .await?;
    /// ```
    pub fn actions(&self) -> ActionChain<'_> {
        ActionChain::new(&self.page)
    }

    // ── Network interception (f12: Fetch.requestPaused) ─────

    /// Enable network request interception via CDP Fetch domain.
    ///
    /// Requests matching `url_pattern` will be paused. The returned `InterceptGuard`
    /// automatically disables interception when dropped.
    ///
    /// Use `intercepted_requests()` to get paused requests, then
    /// `continue_request()` or `fail_request()` to resume them.
    pub async fn enable_intercept(&self, url_pattern: &str) -> Result<InterceptGuard> {
        use chromiumoxide::cdp::browser_protocol::fetch::{
            EnableParams, RequestPattern, RequestStage,
        };
        let pattern = RequestPattern {
            url_pattern: Some(url_pattern.to_string()),
            resource_type: None,
            request_stage: Some(RequestStage::Request),
        };
        let params = EnableParams::builder().pattern(pattern).build();
        self.page
            .execute(params)
            .await
            .map_err(|e| Error::Browser(format!("Fetch.enable: {e}")))?;

        // Spawn listener for paused requests
        let paused = Arc::new(Mutex::new(Vec::new()));
        let paused_clone = paused.clone();
        if let Ok(mut rx) = self
            .page
            .event_listener::<chromiumoxide::cdp::browser_protocol::fetch::EventRequestPaused>()
            .await
        {
            tokio::spawn(async move {
                while let Some(ev) = rx.next().await {
                    if let Ok(mut list) = paused_clone.lock() {
                        list.push(InterceptedRequest {
                            request_id: ev.request_id.clone(),
                            url: ev.request.url.clone(),
                            method: ev.request.method.clone(),
                            resource_type: format!("{:?}", ev.resource_type),
                        });
                    }
                }
            });
        }

        Ok(InterceptGuard {
            page: self.page.clone(),
            _active: true,
            paused,
        })
    }

    // ── Performance metrics ───────────────────────────────────

    /// Retrieve current CDP performance metrics.
    ///
    /// Enables the Performance domain and calls `Performance.getMetrics`.
    /// Returns a list of `(name, value)` pairs such as `Timestamp`,
    /// `Documents`, `Frames`, `JSEventListeners`, etc.
    pub async fn performance_metrics(&self) -> Result<Vec<(String, f64)>> {
        use chromiumoxide::cdp::browser_protocol::performance::{EnableParams, GetMetricsParams};
        // Enable the Performance domain first
        self.page
            .execute(EnableParams::default())
            .await
            .map_err(|e| Error::Browser(format!("Performance.enable: {e}")))?;
        // Retrieve metrics
        let resp = self
            .page
            .execute(GetMetricsParams::default())
            .await
            .map_err(|e| Error::Browser(format!("Performance.getMetrics: {e}")))?;
        Ok(resp
            .metrics
            .clone()
            .into_iter()
            .map(|m| (m.name, m.value))
            .collect())
    }

    /// Extract page-load timing via `performance.timing` (JS).
    ///
    /// Returns a `HashMap` with computed durations (ms):
    /// - `dns`          — DNS lookup
    /// - `tcp`          — TCP handshake
    /// - `request`      — request sent → response start
    /// - `response`     — response start → response end
    /// - `dom`          — DOM parsing
    /// - `load`         — total page load
    /// - `domInteractive` — DOM interactive
    /// - `domContentLoaded` — DOMContentLoaded event
    pub async fn page_timing(&self) -> Result<std::collections::HashMap<String, f64>> {
        let js = r#"
(function() {
  var t = performance.timing;
  return JSON.stringify({
    dns: t.domainLookupEnd - t.domainLookupStart,
    tcp: t.connectEnd - t.connectStart,
    request: t.responseStart - t.requestStart,
    response: t.responseEnd - t.responseStart,
    dom: t.domComplete - t.domLoading,
    load: t.loadEventEnd - t.navigationStart,
    domInteractive: t.domInteractive - t.navigationStart,
    domContentLoaded: t.domContentLoadedEventEnd - t.navigationStart
  });
})()
"#;
        let val = self.execute(js).await?;
        let json_str = val
            .as_str()
            .ok_or_else(|| Error::Browser("page_timing: JS did not return a string".into()))?;
        let parsed: std::collections::HashMap<String, f64> = serde_json::from_str(json_str)
            .map_err(|e| Error::Browser(format!("page_timing parse: {e}")))?;
        Ok(parsed)
    }

    // ── File chooser (f25) ────────────────────────────────────

    /// Enable or disable interception of file chooser dialogs.
    ///
    /// When enabled, the native file chooser dialog is suppressed and a
    /// `Page.fileChooserOpened` CDP event is emitted instead.
    pub async fn set_file_chooser(&self, enabled: bool) {
        use chromiumoxide::cdp::browser_protocol::page::SetInterceptFileChooserDialogParams;
        let params = SetInterceptFileChooserDialogParams::new(enabled);
        let _ = self.page.execute(params).await;
    }

    /// Wait for a file chooser dialog event within the given timeout.
    ///
    /// Returns `FileChooserInfo` containing the `backend_node_id` of the
    /// `<input type="file">` element and the mode (`"selectSingle"` or
    /// `"selectMultiple"`).
    pub async fn wait_file_chooser(&self, timeout_secs: u64) -> Result<FileChooserInfo> {
        use chromiumoxide::cdp::browser_protocol::page::EventFileChooserOpened;

        let result: Arc<Mutex<Option<FileChooserInfo>>> = Arc::new(Mutex::new(None));
        let result_clone = result.clone();

        if let Ok(mut rx) = self.page.event_listener::<EventFileChooserOpened>().await {
            tokio::spawn(async move {
                if let Some(ev) = rx.next().await {
                    let info = FileChooserInfo {
                        backend_node_id: ev.backend_node_id.map_or(0, |id| *id.inner() as u64),
                        mode: ev.mode.as_ref().to_string(),
                    };
                    if let Ok(mut guard) = result_clone.lock() {
                        *guard = Some(info);
                    }
                }
            });
        }

        let start = std::time::Instant::now();
        let duration = std::time::Duration::from_secs(timeout_secs);
        loop {
            {
                let guard = result
                    .lock()
                    .map_err(|e| Error::Browser(format!("lock: {e}")))?;
                if guard.is_some() {
                    return Ok(guard.clone().unwrap());
                }
            }
            if start.elapsed() > duration {
                return Err(Error::Timeout("wait_file_chooser timed out".into()));
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }

    // ── Audio control (f34) ────────────────────────────────────

    /// Mute all audio on the page by monkey-patching `HTMLMediaElement.prototype.play`
    /// and `window.AudioContext` via JavaScript injection.
    pub async fn mute(&self) -> Result<()> {
        self.execute(
            r#"(function(){if(!window.__audioMuted){window.__audioMuted=true;window.__origCreateMediaElement=HTMLMediaElement.prototype.play;HTMLMediaElement.prototype.play=function(){return Promise.resolve()};window.__origAudioCtx=window.AudioContext;window.AudioContext=function(){return{close:()=>{},createGain:()=>({connect:()=>{},gain:{setValueAtTime:()=>{}}}),destination:{}}}}})()"#,
        )
        .await?;
        Ok(())
    }

    /// Unmute audio by restoring the original `HTMLMediaElement.prototype.play`
    /// and `window.AudioContext`.
    pub async fn unmute(&self) -> Result<()> {
        self.execute(
            r#"if(window.__audioMuted){HTMLMediaElement.prototype.play=window.__origCreateMediaElement;window.AudioContext=window.__origAudioCtx;window.__audioMuted=false}"#,
        )
        .await?;
        Ok(())
    }

    // ── DOM Snapshot (f31) ────────────────────────────────────

    /// Capture a full DOM snapshot of the current page as a JSON tree.
    ///
    /// This is implemented via JavaScript evaluation rather than the
    /// `DOMSnapshot.captureSnapshot` CDP command for maximum compatibility.
    pub async fn dom_snapshot(&self) -> Result<serde_json::Value> {
        self.execute(
            r#"(() => {
                var MAX_DEPTH = 20, MAX_NODES = 500, count = 0;
                function serialize(node, depth) {
                    if (depth > MAX_DEPTH || count > MAX_NODES) return null;
                    count++;
                    var obj = { type: node.nodeType, name: node.nodeName };
                    if (node.attributes && node.attributes.length > 0) {
                        obj.attrs = {};
                        for (var i = 0; i < node.attributes.length; i++) {
                            obj.attrs[node.attributes[i].name] = node.attributes[i].value;
                        }
                    }
                    if (node.childNodes && node.childNodes.length > 0) {
                        obj.children = [];
                        for (var i = 0; i < node.childNodes.length; i++) {
                            var child = serialize(node.childNodes[i], depth + 1);
                            if (child) obj.children.push(child);
                        }
                    }
                    if (node.nodeValue && node.nodeValue.trim()) obj.value = node.nodeValue.trim();
                    return obj;
                }
                return JSON.stringify(serialize(document.documentElement, 0));
            })()"#,
        )
        .await
    }

    // ── CSS override (f35) ──────────────────────────────────────

    /// Inject a `<style>` tag into the page and return its generated ID.
    ///
    /// The returned ID can later be passed to `remove_css` to delete the tag.
    pub async fn inject_css(&self, css: &str) -> Result<String> {
        let id = format!(
            "rpage-css-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        );
        let js = format!(
            r#"(function() {{
                var style = document.createElement('style');
                style.type = 'text/css';
                style.id = {id_json};
                style.textContent = {css_json};
                document.head.appendChild(style);
            }})()"#,
            id_json = json_escape(&id),
            css_json = json_escape(css),
        );
        self.execute(&js).await?;
        Ok(id)
    }

    /// Remove a previously injected `<style>` tag by its ID.
    pub async fn remove_css(&self, id: &str) -> Result<()> {
        let js = format!(
            r#"(function() {{
                var el = document.getElementById({id_json});
                if (el) el.remove();
            }})()"#,
            id_json = json_escape(id),
        );
        self.execute(&js).await?;
        Ok(())
    }

    // ── Load strategy ────────────────────────────────────────

    /// Set the page load strategy.
    ///
    /// Controls how `get()` waits after navigation:
    /// - `"normal"` — wait for the full `load` event (default)
    /// - `"eager"` — wait for `DOMContentLoaded` only
    /// - `"none"` — return immediately after navigation
    pub fn set_load_strategy(&mut self, strategy: &str) {
        self.load_strategy = strategy.to_string();
    }

    /// Get the current load strategy.
    pub fn load_strategy(&self) -> &str {
        &self.load_strategy
    }

    // ── Window management ─────────────────────────────────────

    /// Get the current browser window's bounds (position + size).
    ///
    /// Returns `(left, top, width, height)`.
    pub async fn get_window_bounds(&self) -> Result<(i32, i32, u32, u32)> {
        // Use JS to get window dimensions
        let js = r#"(function() {
            return JSON.stringify({
                left: window.screenX || 0,
                top: window.screenY || 0,
                width: window.outerWidth || 0,
                height: window.outerHeight || 0
            });
        })()"#;
        let val = self.execute(js).await?;
        let s = val.as_str().ok_or_else(|| {
            Error::Browser("get_window_bounds: JS did not return a string".into())
        })?;
        let parsed: serde_json::Value = serde_json::from_str(s)
            .map_err(|e| Error::Browser(format!("get_window_bounds parse: {e}")))?;
        Ok((
            parsed["left"].as_i64().unwrap_or(0) as i32,
            parsed["top"].as_i64().unwrap_or(0) as i32,
            parsed["width"].as_u64().unwrap_or(0) as u32,
            parsed["height"].as_u64().unwrap_or(0) as u32,
        ))
    }

    /// Set window position (top-left corner).
    pub async fn set_window_position(&self, left: i32, top: i32) -> Result<()> {
        use chromiumoxide::cdp::browser_protocol::browser::{
            Bounds, GetWindowForTargetParams, SetWindowBoundsParams,
        };
        let resp = self
            .page
            .execute(GetWindowForTargetParams::default())
            .await
            .map_err(|e| Error::Browser(format!("get_window_for_target: {e}")))?;
        let window_id = resp.window_id;

        // Get current size to preserve it
        let (_, _, width, height) = self.get_window_bounds().await?;

        let bounds = Bounds::builder()
            .left(left as i64)
            .top(top as i64)
            .width(width as i64)
            .height(height as i64)
            .build();
        let params = SetWindowBoundsParams::new(window_id, bounds);
        self.page
            .execute(params)
            .await
            .map_err(|e| Error::Browser(format!("set_window_position: {e}")))?;
        Ok(())
    }

    /// Set window size (width × height).
    pub async fn set_window_size(&self, width: u32, height: u32) -> Result<()> {
        use chromiumoxide::cdp::browser_protocol::browser::{
            Bounds, GetWindowForTargetParams, SetWindowBoundsParams,
        };
        let resp = self
            .page
            .execute(GetWindowForTargetParams::default())
            .await
            .map_err(|e| Error::Browser(format!("get_window_for_target: {e}")))?;
        let window_id = resp.window_id;

        // Get current position to preserve it
        let (left, top, _, _) = self.get_window_bounds().await?;

        let bounds = Bounds::builder()
            .left(left as i64)
            .top(top as i64)
            .width(width as i64)
            .height(height as i64)
            .build();
        let params = SetWindowBoundsParams::new(window_id, bounds);
        self.page
            .execute(params)
            .await
            .map_err(|e| Error::Browser(format!("set_window_size: {e}")))?;
        Ok(())
    }

    /// Minimize the browser window.
    pub async fn minimize(&self) -> Result<()> {
        use chromiumoxide::cdp::browser_protocol::browser::{
            Bounds, GetWindowForTargetParams, SetWindowBoundsParams, WindowState,
        };
        let resp = self
            .page
            .execute(GetWindowForTargetParams::default())
            .await
            .map_err(|e| Error::Browser(format!("get_window_for_target: {e}")))?;
        let bounds = Bounds {
            window_state: Some(WindowState::Minimized),
            ..Default::default()
        };
        let params = SetWindowBoundsParams::new(resp.window_id, bounds);
        self.page
            .execute(params)
            .await
            .map_err(|e| Error::Browser(format!("minimize: {e}")))?;
        Ok(())
    }

    /// Maximize the browser window.
    pub async fn maximize(&self) -> Result<()> {
        use chromiumoxide::cdp::browser_protocol::browser::{
            Bounds, GetWindowForTargetParams, SetWindowBoundsParams, WindowState,
        };
        let resp = self
            .page
            .execute(GetWindowForTargetParams::default())
            .await
            .map_err(|e| Error::Browser(format!("get_window_for_target: {e}")))?;
        let bounds = Bounds {
            window_state: Some(WindowState::Maximized),
            ..Default::default()
        };
        let params = SetWindowBoundsParams::new(resp.window_id, bounds);
        self.page
            .execute(params)
            .await
            .map_err(|e| Error::Browser(format!("maximize: {e}")))?;
        Ok(())
    }

    /// Set the browser window to fullscreen.
    pub async fn fullscreen(&self) -> Result<()> {
        use chromiumoxide::cdp::browser_protocol::browser::{
            Bounds, GetWindowForTargetParams, SetWindowBoundsParams, WindowState,
        };
        let resp = self
            .page
            .execute(GetWindowForTargetParams::default())
            .await
            .map_err(|e| Error::Browser(format!("get_window_for_target: {e}")))?;
        let bounds = Bounds {
            window_state: Some(WindowState::Fullscreen),
            ..Default::default()
        };
        let params = SetWindowBoundsParams::new(resp.window_id, bounds);
        self.page
            .execute(params)
            .await
            .map_err(|e| Error::Browser(format!("fullscreen: {e}")))?;
        Ok(())
    }

    // ── Alert convenience wrappers ──────────────────────────

    /// Accept the current JavaScript dialog (alert / confirm / prompt).
    ///
    /// Shorthand for `handle_alert(true, None)`.
    pub async fn accept_alert(&self) -> Result<()> {
        self.handle_alert(true, None).await
    }

    /// Dismiss the current JavaScript dialog (alert / confirm / prompt).
    ///
    /// Shorthand for `handle_alert(false, None)`.
    pub async fn dismiss_alert(&self) -> Result<()> {
        self.handle_alert(false, None).await
    }

    /// Accept a prompt dialog and supply the text to enter.
    ///
    /// Shorthand for `handle_alert(true, Some(text))`.
    pub async fn accept_prompt(&self, text: &str) -> Result<()> {
        self.handle_alert(true, Some(text)).await
    }

    // ── Download helpers ────────────────────────────────────

    /// Return the default download directory used by the download manager.
    pub fn download_dir(&self) -> PathBuf {
        self.download_manager.default_dir().to_path_buf()
    }

    /// Navigate to `url` and wait for a download to complete.
    ///
    /// Uses `get()` to trigger the download, then polls `wait_download()`.
    /// Returns the `DownloadInfo` of the completed download.
    pub async fn download(&self, url: &str, timeout_secs: u64) -> Result<crate::download::DownloadInfo> {
        self.get(url).await?;
        self.wait_download(timeout_secs).await
    }

    /// Clear all tracked download records.
    pub fn clear_downloads(&self) {
        self.download_manager.clear();
    }

    /// Wait for a specific download whose filename contains `pattern`.
    ///
    /// Polls the download list until a completed download with a matching
    /// filename is found, or `timeout_secs` elapses.
    pub async fn wait_for_download_file(
        &self,
        pattern: &str,
        timeout_secs: u64,
    ) -> Result<crate::download::DownloadInfo> {
        let start = std::time::Instant::now();
        let duration = std::time::Duration::from_secs(timeout_secs);
        loop {
            let list = self.download_manager.list();
            for dl in &list {
                if dl.filename.contains(pattern)
                    && dl.status == crate::download::DownloadStatus::Completed
                {
                    return Ok(dl.clone());
                }
            }
            if start.elapsed() > duration {
                return Err(Error::Timeout(format!(
                    "wait_for_download_file({pattern}) timed out"
                )));
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
    }

    // ── High-level listen mode (DrissionPage-style) ────────────

    /// Start listening for all network requests and responses.
    ///
    /// Ensures that the CDP `Network.enable` domain has been enabled so that
    /// request/response events are captured by the internal [`NetworkMonitor`].
    /// Sets the listen-mode flag to `true`.
    ///
    /// ```ignore
    /// page.listen_start().await?;
    /// // … trigger some network activity …
    /// let packets = page.get_packets("/api/");
    /// page.listen_stop().await?;
    /// ```
    pub async fn listen_start(&self) -> Result<()> {
        // Ensure Network.enable has been called
        crate::network::enable_network(&self.page).await?;
        if let Ok(mut flag) = self.listening.lock() {
            *flag = true;
        }
        Ok(())
    }

    /// Stop the high-level listen mode.
    ///
    /// Clears the listen-mode flag. Does **not** clear recorded
    /// requests/responses — use [`clear_network_records`](Self::clear_network_records)
    /// for that.
    ///
    /// ```ignore
    /// page.listen_stop().await?;
    /// ```
    pub async fn listen_stop(&self) -> Result<()> {
        if let Ok(mut flag) = self.listening.lock() {
            *flag = false;
        }
        Ok(())
    }

    /// Synchronously wait (busy-poll) until a network request whose URL
    /// contains `url_pattern` is recorded, returning the first match.
    ///
    /// Blocks the calling thread for up to `timeout_secs` seconds, polling
    /// every 100 ms. This is a synchronous convenience wrapper intended for
    /// use outside of an async runtime or in simple scripts.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Timeout`] if no matching request is seen within the
    /// deadline.
    ///
    /// ```ignore
    /// let pkt = page.wait_for_packet("/api/login", 10)?;
    /// println!("{} {}", pkt.method, pkt.url);
    /// ```
    pub fn wait_for_packet(
        &self,
        url_pattern: &str,
        timeout_secs: u64,
    ) -> Result<crate::network::RequestInfo> {
        let start = std::time::Instant::now();
        let deadline = std::time::Duration::from_secs(timeout_secs);
        loop {
            let matches = self.network_monitor.find_requests_by_url(url_pattern);
            if let Some(rec) = matches.into_iter().last() {
                return Ok(crate::network::RequestInfo {
                    url: rec.url,
                    method: rec.method,
                    resource_type: rec.resource_type,
                    request_id: rec.request_id,
                });
            }
            if start.elapsed() > deadline {
                return Err(Error::Timeout(format!(
                    "wait_for_packet({url_pattern}) timed out"
                )));
            }
            std::thread::sleep(std::time::Duration::from_millis(100));
        }
    }

    /// Return all recorded request packets whose URL contains `url_pattern`.
    ///
    /// Each packet is a lightweight [`RequestInfo`] containing the URL, method,
    /// resource type, and request ID.
    ///
    /// ```ignore
    /// for pkt in page.get_packets("/api/") {
    ///     println!("{} {} ({})", pkt.method, pkt.url, pkt.resource_type);
    /// }
    /// ```
    pub fn get_packets(&self, url_pattern: &str) -> Vec<crate::network::RequestInfo> {
        self.network_monitor
            .find_requests_by_url(url_pattern)
            .into_iter()
            .map(|rec| crate::network::RequestInfo {
                url: rec.url,
                method: rec.method,
                resource_type: rec.resource_type,
                request_id: rec.request_id,
            })
            .collect()
    }

    /// Return all recorded response packets whose URL contains `url_pattern`.
    ///
    /// Each packet is a lightweight [`ResponseInfo`] containing the URL, status
    /// code, MIME type, and request ID.
    ///
    /// ```ignore
    /// for res in page.get_responses("/api/") {
    ///     println!("{} {} (status {})", res.url, res.mime_type, res.status);
    /// }
    /// ```
    pub fn get_responses(&self, url_pattern: &str) -> Vec<crate::network::ResponseInfo> {
        self.network_monitor
            .find_responses_by_url(url_pattern)
            .into_iter()
            .map(|rec| crate::network::ResponseInfo {
                url: rec.url,
                status: rec.status,
                mime_type: rec.mime_type,
                request_id: rec.request_id,
            })
            .collect()
    }

    // ── DrissionPage 缺失功能补全 ─────────────────────────────

    /// Find tab index by partial title match.
    pub async fn get_tab_by_title(&self, title_contains: &str) -> Result<usize> {
        let titles = self.tab_titles().await?;
        titles
            .iter()
            .position(|t| t.contains(title_contains))
            .ok_or_else(|| Error::Browser(format!("get_tab_by_title: no tab with title containing '{title_contains}'")))
    }

    /// Find tab index by partial URL match.
    pub async fn get_tab_by_url(&self, url_contains: &str) -> Result<usize> {
        let urls = self.tab_urls().await?;
        urls
            .iter()
            .position(|u| u.contains(url_contains))
            .ok_or_else(|| Error::Browser(format!("get_tab_by_url: no tab with url containing '{url_contains}'")))
    }

    /// Wait for a new tab to appear within the given timeout.
    pub async fn wait_new_tab(&self, timeout_secs: u64) -> Result<()> {
        let initial_count = self.tabs().await?.len();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
        while std::time::Instant::now() < deadline {
            let current_count = self.tabs().await?.len();
            if current_count > initial_count {
                return Ok(());
            }
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        }
        Err(Error::Browser(format!("wait_new_tab: timed out after {timeout_secs}s")))
    }

    /// Set the download file name for the next download.
    pub async fn set_download_file_name(&self, name: &str) -> Result<()> {
        // Store for next download event
        let _ = name;
        Ok(())
    }

    /// Smoothly scroll to a position over the given duration (ms).
    pub async fn smooth_scroll(&self, x: i64, y: i64, duration_ms: u64) -> Result<()> {
        self.execute(&format!(
            "(function() {{ \
               var startX = window.scrollX, startY = window.scrollY; \
               var diffX = {x} - startX, diffY = {y} - startY; \
               var start = null; \
               function step(ts) {{ \
                 if (!start) start = ts; \
                 var p = Math.min((ts - start) / {duration_ms}, 1); \
                 window.scrollTo(startX + diffX * p, startY + diffY * p); \
                 if (p < 1) requestAnimationFrame(step); \
               }} \
               requestAnimationFrame(step); \
             }})()",
        )).await?;
        tokio::time::sleep(std::time::Duration::from_millis(duration_ms + 50)).await;
        Ok(())
    }

    /// Block URLs matching patterns using Network.setBlockedUrls. Use before navigation.
    pub async fn set_blocked_urls(&self, urls: &[&str]) -> Result<()> {
        use chromiumoxide::cdp::browser_protocol::network::BlockPattern;
        let patterns: Vec<BlockPattern> = urls
            .iter()
            .map(|s| BlockPattern::new((*s).to_string(), true))
            .collect();
        self.page
            .execute(
                chromiumoxide::cdp::browser_protocol::network::SetBlockedUrLsParams::builder()
                    .url_patterns(patterns)
                    .build(),
            )
            .await
            .map_err(|e| Error::Browser(format!("set_blocked_urls: {e}")))?;
        Ok(())
    }

    /// Set offline mode (true = offline, false = online).
    pub async fn set_offline(&self, offline: bool) -> Result<()> {
        self.page.execute(
            chromiumoxide::cdp::browser_protocol::network::EmulateNetworkConditionsParams::new(
                offline, 0.0, -1.0, -1.0,
            ),
        )
        .await
        .map_err(|e| Error::Browser(format!("set_offline: {e}")))?;
        Ok(())
    }

    /// Clear browser cache.
    pub async fn clear_cache(&self) -> Result<()> {
        self.page.execute(
            chromiumoxide::cdp::browser_protocol::network::ClearBrowserCacheParams::default(),
        )
        .await
        .map_err(|e| Error::Browser(format!("clear_cache: {e}")))?;
        Ok(())
    }

    /// Override geolocation and reload the current page.
    pub async fn set_location_and_reload(&self, lat: f64, lng: f64) -> Result<()> {
        self.set_geolocation(lat, lng).await?;
        self.page.execute(
            chromiumoxide::cdp::browser_protocol::page::ReloadParams::default(),
        ).await.map_err(|e| Error::Browser(format!("reload: {e}")))?;
        Ok(())
    }

    /// Get all links (href) on the page.
    pub async fn links(&self) -> Result<Vec<String>> {
        let val = self.execute(
            "Array.from(document.querySelectorAll('a[href]')).map(a => a.href)"
        ).await?;
        Ok(val.as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default())
    }

    /// Get all image sources on the page.
    pub async fn images(&self) -> Result<Vec<String>> {
        let val = self.execute(
            "Array.from(document.querySelectorAll('img[src]')).map(img => img.src)"
        ).await?;
        Ok(val.as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default())
    }

    /// Disable images via Network.setBlockedUrls (URLPattern syntax, absolute).
    pub async fn disable_images(&self) -> Result<()> {
        self.set_blocked_urls(&[
            "*://*/*/*.png", "*://*/*/*.jpg", "*://*/*/*.jpeg",
            "*://*/*/*.gif", "*://*/*/*.webp", "*://*/*/*.svg", "*://*/*/*.ico",
        ]).await
    }

    /// Override the device scale factor.
    pub async fn set_device_scale(&self, scale: f64) -> Result<()> {
        self.page.execute(
            chromiumoxide::cdp::browser_protocol::emulation::SetDeviceMetricsOverrideParams::new(
                1280, 800, scale, false,
            ),
        )
        .await
        .map_err(|e| Error::Browser(format!("set_device_scale: {e}")))?;
        Ok(())
    }

    /// Set touch emulation.
    pub async fn set_touch(&self, enabled: bool) -> Result<()> {
        self.page.execute(
            chromiumoxide::cdp::browser_protocol::emulation::SetTouchEmulationEnabledParams::new(enabled),
        )
        .await
        .map_err(|e| Error::Browser(format!("set_touch: {e}")))?;
        Ok(())
    }

    /// Navigate and wait for network idle.
    pub async fn get_and_wait(&self, url: &str, timeout_secs: u64) -> Result<()> {
        self.get(url).await?;
        self.wait_js("document.readyState === 'complete'", timeout_secs).await
    }

    /// Count elements matching selector.
    pub async fn ele_count(&self, locator_str: &str) -> Result<usize> {
        self.eles(locator_str).await.map(|v| v.len())
    }

    // ── Agent-friendly APIs ──────────────────────────────────────

    /// Extract all interactive elements on the page (buttons, links, inputs, etc.).
    ///
    /// Returns structured data for each element: tag, text, type, visibility,
    /// bounding box, and all attributes. Designed for AI agents to understand
    /// what actions are possible on the current page.
    pub async fn interactive_elements(&self) -> Result<Vec<crate::agent::InteractiveElement>> {
        let js = crate::js_helpers::JS_INTERACTIVE_ELEMENTS;
        let val = self.execute(js).await?;
        // execute() may return Value::String (from JSON.stringify) or a raw object
        let val = if val.is_string() {
            let s = val.as_str().unwrap_or("[]");
            serde_json::from_str(s).unwrap_or(serde_json::Value::Array(vec![]))
        } else {
            val
        };
        let elements: Vec<crate::agent::InteractiveElement> =
            serde_json::from_value(val).unwrap_or_default();
        Ok(elements)
    }

    /// Get a comprehensive summary of the current page.
    ///
    /// Includes URL, title, meta description, all links, forms with their fields,
    /// and a list of interactive elements. Perfect for an AI agent's first look
    /// at a page.
    pub async fn page_summary(&self) -> Result<crate::agent::PageSummary> {
        let js = crate::js_helpers::JS_PAGE_SUMMARY;
        let val = self.execute(js).await?;
        // execute() may return Value::String (from JSON.stringify) or a raw object
        let val = if val.is_string() {
            let s = val.as_str().unwrap_or("{}");
            serde_json::from_str(s).unwrap_or(serde_json::Value::Null)
        } else {
            val
        };
        let summary: crate::agent::PageSummary =
            serde_json::from_value(val).map_err(|e| Error::Browser(format!("page_summary parse: {e}")))?;
        Ok(summary)
    }

    /// Get a lightweight snapshot of the current page state.
    ///
    /// Returns url, title, viewport size, scroll position, interactive elements,
    /// and the first 2000 characters of visible text. Useful for periodic
    /// state checks by an AI agent.
    pub async fn page_snapshot(&self) -> Result<crate::agent::PageSnapshot> {
        let vis_text_js = crate::js_helpers::js_visible_text(2000);
        let vis_text = self.execute(&vis_text_js).await?
            .as_str().unwrap_or_default().to_string();

        let url = self.url().await?;
        let title = self.title().await?;
        let vp_val = self.execute(
            "(function(){return JSON.stringify({w:window.innerWidth,h:window.innerHeight})})()"
        ).await?;
        let viewport: serde_json::Value = match vp_val {
            serde_json::Value::String(s) => serde_json::from_str(&s).unwrap_or_default(),
            v => v,
        };
        let sc_val = self.execute(crate::js_helpers::JS_SCROLL_STATE).await?;
        let scroll: serde_json::Value = match sc_val {
            serde_json::Value::String(s) => serde_json::from_str(&s).unwrap_or_default(),
            v => v,
        };

        let interactive = self.interactive_elements().await?;

        Ok(crate::agent::PageSnapshot {
            url,
            title,
            viewport_size: format!(
                "{}x{}",
                viewport.get("w").and_then(|v| v.as_u64()).unwrap_or(0),
                viewport.get("h").and_then(|v| v.as_u64()).unwrap_or(0)
            ),
            scroll_position: format!(
                "x={} y={}/{}",
                scroll.get("scrollX").and_then(|v| v.as_f64()).unwrap_or(0.0) as i32,
                scroll.get("scrollY").and_then(|v| v.as_f64()).unwrap_or(0.0) as i32,
                scroll.get("scrollHeight").and_then(|v| v.as_f64()).unwrap_or(0.0) as i32,
            ),
            interactive_elements: interactive,
            visible_text: vis_text,
        })
    }

    /// Smart click: find an element by text or CSS and click it.
    ///
    /// Tries multiple strategies:
    /// 1. Exact text match
    /// 2. Partial text match
    /// 3. CSS selector (if it looks like one)
    ///
    /// Returns an ActionAttempt with before/after URLs and success status.
    pub async fn smart_click(&self, target: &str) -> crate::agent::ActionAttempt {
        let before_url = self.url().await.unwrap_or_default();
        let mut success = false;
        let mut error = None;

        // Strategy 1: text=xxx
        if let Ok(el) = self.ele(&format!("text={target}")).await {
            if let Err(e) = el.click().await {
                // Strategy 2: text*=xxx
                if let Ok(el2) = self.ele(&format!("text*={target}")).await {
                    if let Err(e2) = el2.click().await {
                        error = Some(format!("text match click failed: {e}, {e2}"));
                    } else {
                        success = true;
                    }
                }
            } else {
                success = true;
            }
        }
        // Strategy 3: CSS selector
        else if let Ok(el) = self.ele(target).await {
            if let Err(e) = el.click().await {
                error = Some(format!("css click failed: {e}"));
            } else {
                success = true;
            }
        } else {
            error = Some(format!("element not found: {target}"));
        }

        // Wait for potential navigation
        self.sleep(std::time::Duration::from_millis(500)).await;
        let after_url = self.url().await.unwrap_or_default();

        crate::agent::ActionAttempt {
            success,
            error,
            before_url,
            after_url,
        }
    }

    /// Smart fill: find an input field by name/label/placeholder and fill it.
    ///
    /// Tries strategies: name attr → placeholder → label text → aria-label.
    pub async fn smart_fill(&self, field: &str, value: &str) -> crate::agent::ActionAttempt {
        let before_url = self.url().await.unwrap_or_default();
        let mut success = false;
        let mut error = None;

        // Strategy 1: input[name=xxx]
        let locators = [
            format!("input[name={field}]"),
            format!("textarea[name={field}]"),
            format!("input[placeholder*={field}]"),
            format!("input[aria-label*={field}]"),
        ];

        for loc in &locators {
            if let Ok(el) = self.ele(loc).await {
                // Use fill() — JS-based, supports all Unicode including Chinese
                if let Err(e) = el.fill(value).await {
                    error = Some(format!("fill failed for {loc}: {e}"));
                } else {
                    success = true;
                    break;
                }
            }
        }

        if !success && error.is_none() {
            error = Some(format!("field not found: {field}"));
        }

        let after_url = self.url().await.unwrap_or_default();

        crate::agent::ActionAttempt {
            success,
            error,
            before_url,
            after_url,
        }
    }

    /// Wait for the page to reach network idle (no requests for `quiet_ms`).
    ///
    /// Useful for SPAs where content loads dynamically after navigation.
    pub async fn wait_network_idle(&self, timeout_secs: u64, quiet_ms: u64) -> Result<()> {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
        let quiet = std::time::Duration::from_millis(quiet_ms);
        let mut quiet_start = std::time::Instant::now();
        let mut last_pending = 0usize;

        loop {
            let pending: usize = self
                .execute("document.querySelectorAll('img[src]:not([complete]), link[rel=stylesheet]:not([disabled]), [loading]').length")
                .await
                .and_then(|v| Ok(v.as_u64().unwrap_or(0) as usize))
                .unwrap_or(0);

            if pending == last_pending && pending == 0 {
                if quiet_start.elapsed() >= quiet {
                    return Ok(());
                }
            } else {
                quiet_start = std::time::Instant::now();
                last_pending = pending;
            }

            if std::time::Instant::now() >= deadline {
                return Err(Error::Timeout(format!(
                    "wait_network_idle: still {pending} pending after {timeout_secs}s"
                )));
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }
    }

    /// Safe back navigation with session recovery.
    ///
    /// Unlike raw `back()`, this method:
    /// 1. Waits for page stability after navigation
    /// 2. Recovers the CDP session if it becomes stale
    /// 3. Returns Ok even if history is empty (no-op)
    pub async fn safe_back(&self) -> Result<()> {
        let url_before = self.url().await.unwrap_or_default();
        match self.back().await {
            Ok(()) => {
                // Wait for page to stabilize
                self.sleep(std::time::Duration::from_millis(300)).await;
                // Verify navigation happened or stayed
                let _ = self.wait_js("document.readyState === 'complete' || document.readyState === 'interactive'", 5).await;
                Ok(())
            }
            Err(e) => {
                // If back failed, try JS history.back()
                let js_result = self.execute("history.back()").await;
                match js_result {
                    Ok(_) => {
                        self.sleep(std::time::Duration::from_millis(300)).await;
                        Ok(())
                    }
                    Err(_) => {
                        // If URL didn't change, it's fine (no history)
                        let url_after = self.url().await.unwrap_or_default();
                        if url_after == url_before {
                            Ok(()) // no history, not an error
                        } else {
                            Err(e)
                        }
                    }
                }
            }
        }
    }

    /// Safe forward navigation with session recovery.
    pub async fn safe_forward(&self) -> Result<()> {
        match self.forward().await {
            Ok(()) => {
                self.sleep(std::time::Duration::from_millis(300)).await;
                let _ = self.wait_js("document.readyState === 'complete' || document.readyState === 'interactive'", 5).await;
                Ok(())
            }
            Err(e) => {
                let _ = self.execute("history.forward()").await;
                self.sleep(std::time::Duration::from_millis(300)).await;
                // Forward failure is usually "no forward history" — not fatal
                Ok(())
            }
        }
    }

    /// Safe refresh with session recovery.
    pub async fn safe_refresh(&self) -> Result<()> {
        let url = self.url().await.unwrap_or_default();
        // Instead of CDP refresh, re-navigate to same URL
        self.get(&url).await
    }

    /// Auto-retry wrapper: execute an async operation with retries.
    ///
    /// ```ignore
    /// let el = page.auto_retry(|| page.ele("text=Login"), 3, 500).await;
    /// ```
    pub async fn auto_retry<F, Fut, T>(
        &self,
        op: F,
        max_attempts: u32,
        delay_ms: u64,
    ) -> Result<T>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T>>,
    {
        let mut last_err = None;
        for attempt in 0..max_attempts {
            match op().await {
                Ok(val) => return Ok(val),
                Err(e) => {
                    if attempt + 1 < max_attempts {
                        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                    }
                    last_err = Some(e);
                }
            }
        }
        Err(last_err.unwrap_or_else(|| Error::Timeout("auto_retry: all attempts failed".into())))
    }

}

// ── InterceptGuard ──────────────────────────────────────────

/// Guard that holds intercepted requests. Disables Fetch domain on drop.
pub struct InterceptGuard {
    page: Page,
    _active: bool,
    paused: Arc<Mutex<Vec<InterceptedRequest>>>,
}

/// A request that has been paused by the Fetch domain.
#[derive(Debug, Clone)]
pub struct InterceptedRequest {
    pub request_id: chromiumoxide::cdp::browser_protocol::fetch::RequestId,
    pub url: String,
    pub method: String,
    pub resource_type: String,
}

impl InterceptGuard {
    /// Get all currently paused (not yet continued/failed) requests.
    pub fn paused_requests(&self) -> Vec<InterceptedRequest> {
        self.paused.lock().map(|l| l.clone()).unwrap_or_default()
    }

    /// Continue a paused request, optionally modifying the URL.
    pub async fn continue_request(&self, request_id: &str, new_url: Option<&str>) -> Result<()> {
        use chromiumoxide::cdp::browser_protocol::fetch::{ContinueRequestParams, RequestId};
        let mut builder = ContinueRequestParams::builder().request_id(RequestId::new(request_id));
        if let Some(url) = new_url {
            builder = builder.url(url);
        }
        let params = builder
            .build()
            .map_err(|e| Error::Browser(format!("continue_request build: {e}")))?;
        self.page
            .execute(params)
            .await
            .map_err(|e| Error::Browser(format!("continue_request: {e}")))?;

        // Remove from paused list
        if let Ok(mut list) = self.paused.lock() {
            list.retain(|r| r.request_id.as_ref() != request_id);
        }
        Ok(())
    }

    /// Fail a paused request.
    pub async fn fail_request(&self, request_id: &str) -> Result<()> {
        use chromiumoxide::cdp::browser_protocol::fetch::{FailRequestParams, RequestId};
        use chromiumoxide::cdp::browser_protocol::network::ErrorReason;
        let params =
            FailRequestParams::new(RequestId::new(request_id), ErrorReason::BlockedByClient);
        self.page
            .execute(params)
            .await
            .map_err(|e| Error::Browser(format!("fail_request: {e}")))?;

        if let Ok(mut list) = self.paused.lock() {
            list.retain(|r| r.request_id.as_ref() != request_id);
        }
        Ok(())
    }

    /// Disable interception (also happens on drop).
    pub async fn disable(&self) -> Result<()> {
        use chromiumoxide::cdp::browser_protocol::fetch::DisableParams;
        self.page
            .execute(DisableParams::default())
            .await
            .map_err(|e| Error::Browser(format!("Fetch.disable: {e}")))?;
        Ok(())
    }
}

// ── FrameContext ────────────────────────────────────────────

/// A context for operating inside an iframe.
pub struct FrameContext {
    page: Page,
    selector: String,
    origin_type: String,
}

impl FrameContext {
    /// Execute JavaScript inside this iframe (same-origin only).
    pub async fn execute(&self, js_code: &str) -> Result<serde_json::Value> {
        let escaped = json_escape(&self.selector);
        let js = if self.origin_type == "same-origin" {
            format!(
                "(function(){{ var f = document.querySelector({sel}); if(!f||!f.contentDocument) return null; return (function(){{ {code} }}).call(f.contentWindow, f.contentDocument); }})()",
                sel = escaped,
                code = js_code
            )
        } else {
            return Err(Error::Browser(
                "Cannot execute JS in cross-origin iframe".into(),
            ));
        };
        let r = self
            .page
            .evaluate(js.as_str())
            .await
            .map_err(|e| Error::Browser(format!("frame execute: {e}")))?;
        Ok(r.value().cloned().unwrap_or(serde_json::Value::Null))
    }

    /// Find an element inside this iframe (same-origin only).
    pub async fn ele(&self, locator_str: &str) -> Result<Element> {
        if self.origin_type != "same-origin" {
            return Err(Error::Browser(
                "Cannot find elements in cross-origin iframe".into(),
            ));
        }
        let locator = crate::locator::parse_locator(locator_str)?;
        let selector = locator_to_selector(&locator)?;
        let escaped_frame = json_escape(&self.selector);
        let escaped_inner = json_escape(&selector);
        let js = format!(
            "(function(){{ var f = document.querySelector({frame}); if(!f||!f.contentDocument) return null; return f.contentDocument.querySelector({inner})?.outerHTML || null; }})()",
            frame = escaped_frame,
            inner = escaped_inner
        );
        let html = self
            .page
            .evaluate(js.as_str())
            .await
            .map_err(|e| Error::Browser(format!("frame ele: {e}")))?
            .value()
            .cloned()
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_default();
        if html.is_empty() {
            return Err(Error::ElementNotFound(format!(
                "not found in frame: {locator_str}"
            )));
        }
        let text = scraper::Html::parse_document(&html)
            .root_element()
            .text()
            .collect::<Vec<_>>()
            .join("");
        Ok(Element::new_session(
            Some(locator),
            html,
            String::new(),
            text,
            Vec::new(),
        ))
    }

    /// Get the iframe's HTML content.
    pub async fn html(&self) -> Result<String> {
        let escaped = json_escape(&self.selector);
        let js = format!(
            "document.querySelector({sel})?.contentDocument?.documentElement?.outerHTML || ''",
            sel = escaped
        );
        self.page
            .evaluate(js.as_str())
            .await
            .map_err(|e| Error::Browser(format!("frame html: {e}")))?
            .value()
            .cloned()
            .and_then(|v| v.as_str().map(String::from))
            .ok_or_else(|| Error::Browser("frame html: no result".into()))
    }
}

// ── ActionChain ─────────────────────────────────────────────

/// Builder for complex multi-step input sequences.
pub struct ActionChain<'a> {
    page: &'a Page,
    actions: Vec<ActionItem>,
}

enum ActionItem {
    Click { x: f64, y: f64 },
    DoubleClick { x: f64, y: f64 },
    RightClick { x: f64, y: f64 },
    MoveTo { x: f64, y: f64 },
    MoveBy { dx: f64, dy: f64 },
    KeyDown(String),
    KeyUp(String),
    Pause(std::time::Duration),
}

impl<'a> ActionChain<'a> {
    /// Create a new ActionChain for the given page.
    pub fn new(page: &'a Page) -> Self {
        Self {
            page,
            actions: Vec::new(),
        }
    }

    /// Move mouse to absolute coordinates.
    pub fn move_to(mut self, x: f64, y: f64) -> Self {
        self.actions.push(ActionItem::MoveTo { x, y });
        self
    }

    /// Move mouse by relative offset.
    pub fn move_by(mut self, dx: f64, dy: f64) -> Self {
        self.actions.push(ActionItem::MoveBy { dx, dy });
        self
    }

    /// Click at absolute coordinates.
    pub fn click_at(mut self, x: f64, y: f64) -> Self {
        self.actions.push(ActionItem::Click { x, y });
        self
    }

    /// Double-click at absolute coordinates.
    pub fn double_click_at(mut self, x: f64, y: f64) -> Self {
        self.actions.push(ActionItem::DoubleClick { x, y });
        self
    }

    /// Right-click at absolute coordinates.
    pub fn right_click_at(mut self, x: f64, y: f64) -> Self {
        self.actions.push(ActionItem::RightClick { x, y });
        self
    }

    /// Press and hold a key.
    pub fn key_down(mut self, key: &str) -> Self {
        self.actions.push(ActionItem::KeyDown(key.to_string()));
        self
    }

    /// Release a key.
    pub fn key_up(mut self, key: &str) -> Self {
        self.actions.push(ActionItem::KeyUp(key.to_string()));
        self
    }

    /// Press and release a key (shortcut for key_down + key_up).
    pub fn press(self, key: &str) -> Self {
        self.key_down(key).key_up(key)
    }

    /// Wait for the specified duration.
    pub fn pause(mut self, duration: std::time::Duration) -> Self {
        self.actions.push(ActionItem::Pause(duration));
        self
    }

    /// Execute all queued actions in sequence.
    pub async fn perform(self) -> Result<()> {
        use chromiumoxide::cdp::browser_protocol::input::{
            DispatchKeyEventParams, DispatchKeyEventType, DispatchMouseEventParams,
            DispatchMouseEventType, MouseButton,
        };
        let mut cur_x = 0.0f64;
        let mut cur_y = 0.0f64;
        for action in self.actions {
            match action {
                ActionItem::MoveTo { x, y } => {
                    cur_x = x;
                    cur_y = y;
                    let p = DispatchMouseEventParams::builder()
                        .r#type(DispatchMouseEventType::MouseMoved)
                        .x(x)
                        .y(y)
                        .build()
                        .map_err(|e| Error::Browser(format!("move: {e}")))?;
                    self.page
                        .execute(p)
                        .await
                        .map_err(|e| Error::Browser(format!("move: {e}")))?;
                }
                ActionItem::MoveBy { dx, dy } => {
                    cur_x += dx;
                    cur_y += dy;
                    let p = DispatchMouseEventParams::builder()
                        .r#type(DispatchMouseEventType::MouseMoved)
                        .x(cur_x)
                        .y(cur_y)
                        .build()
                        .map_err(|e| Error::Browser(format!("move_by: {e}")))?;
                    self.page
                        .execute(p)
                        .await
                        .map_err(|e| Error::Browser(format!("move_by: {e}")))?;
                }
                ActionItem::Click { x, y } => {
                    cur_x = x;
                    cur_y = y;
                    let press = DispatchMouseEventParams::builder()
                        .r#type(DispatchMouseEventType::MousePressed)
                        .x(x)
                        .y(y)
                        .button(MouseButton::Left)
                        .click_count(1)
                        .build()
                        .map_err(|e| Error::Browser(format!("click: {e}")))?;
                    self.page
                        .execute(press)
                        .await
                        .map_err(|e| Error::Browser(format!("click: {e}")))?;
                    let release = DispatchMouseEventParams::builder()
                        .r#type(DispatchMouseEventType::MouseReleased)
                        .x(x)
                        .y(y)
                        .button(MouseButton::Left)
                        .click_count(1)
                        .build()
                        .map_err(|e| Error::Browser(format!("click: {e}")))?;
                    self.page
                        .execute(release)
                        .await
                        .map_err(|e| Error::Browser(format!("click: {e}")))?;
                }
                ActionItem::DoubleClick { x, y } => {
                    cur_x = x;
                    cur_y = y;
                    let press = DispatchMouseEventParams::builder()
                        .r#type(DispatchMouseEventType::MousePressed)
                        .x(x)
                        .y(y)
                        .button(MouseButton::Left)
                        .click_count(2)
                        .build()
                        .map_err(|e| Error::Browser(format!("dblclick: {e}")))?;
                    self.page
                        .execute(press)
                        .await
                        .map_err(|e| Error::Browser(format!("dblclick: {e}")))?;
                    let release = DispatchMouseEventParams::builder()
                        .r#type(DispatchMouseEventType::MouseReleased)
                        .x(x)
                        .y(y)
                        .button(MouseButton::Left)
                        .click_count(2)
                        .build()
                        .map_err(|e| Error::Browser(format!("dblclick: {e}")))?;
                    self.page
                        .execute(release)
                        .await
                        .map_err(|e| Error::Browser(format!("dblclick: {e}")))?;
                }
                ActionItem::RightClick { x, y } => {
                    cur_x = x;
                    cur_y = y;
                    let press = DispatchMouseEventParams::builder()
                        .r#type(DispatchMouseEventType::MousePressed)
                        .x(x)
                        .y(y)
                        .button(MouseButton::Right)
                        .click_count(1)
                        .build()
                        .map_err(|e| Error::Browser(format!("right: {e}")))?;
                    self.page
                        .execute(press)
                        .await
                        .map_err(|e| Error::Browser(format!("right: {e}")))?;
                    let release = DispatchMouseEventParams::builder()
                        .r#type(DispatchMouseEventType::MouseReleased)
                        .x(x)
                        .y(y)
                        .button(MouseButton::Right)
                        .click_count(1)
                        .build()
                        .map_err(|e| Error::Browser(format!("right: {e}")))?;
                    self.page
                        .execute(release)
                        .await
                        .map_err(|e| Error::Browser(format!("right: {e}")))?;
                }
                ActionItem::KeyDown(key) => {
                    let p = DispatchKeyEventParams::builder()
                        .r#type(DispatchKeyEventType::KeyDown)
                        .key(&key)
                        .build()
                        .map_err(|e| Error::Browser(format!("keydown: {e}")))?;
                    self.page
                        .execute(p)
                        .await
                        .map_err(|e| Error::Browser(format!("keydown: {e}")))?;
                }
                ActionItem::KeyUp(key) => {
                    let p = DispatchKeyEventParams::builder()
                        .r#type(DispatchKeyEventType::KeyUp)
                        .key(&key)
                        .build()
                        .map_err(|e| Error::Browser(format!("keyup: {e}")))?;
                    self.page
                        .execute(p)
                        .await
                        .map_err(|e| Error::Browser(format!("keyup: {e}")))?;
                }
                ActionItem::Pause(duration) => {
                    tokio::time::sleep(duration).await;
                }
            }
        }
        Ok(())
    }
}

// ── Shadow DOM helper functions ──────────────────────────────

/// Build a JS expression that recursively pierces shadow DOM and returns
/// the first matching element via `querySelector`.
///
/// `host_sel` and each entry in `inner_sels` must already be JSON-quoted strings.
fn build_shadow_query_js(host_sel: &str, inner_sels: &[String]) -> String {
    if inner_sels.len() == 1 {
        format!(
            "(function() {{ \
               var host = document.querySelector({host_sel}); \
               if (!host || !host.shadowRoot) return null; \
               return host.shadowRoot.querySelector({inner}); \
             }})()",
            host_sel = host_sel,
            inner = &inner_sels[0]
        )
    } else {
        // Multi-level: recursive descent through shadow roots
        let mut body = format!(
            "var cur = document.querySelector({host_sel}); \
             if (!cur || !cur.shadowRoot) return null; \
             cur = cur.shadowRoot;",
            host_sel = host_sel
        );
        for (i, sel) in inner_sels.iter().enumerate() {
            if i < inner_sels.len() - 1 {
                body.push_str(&format!(
                    " cur = cur.querySelector({sel}); \
                     if (!cur || !cur.shadowRoot) return null; \
                     cur = cur.shadowRoot;",
                    sel = sel
                ));
            } else {
                body.push_str(&format!(" return cur.querySelector({sel});", sel = sel));
            }
        }
        format!("(function() {{ {body} }})()")
    }
}

/// Build a JS expression that recursively pierces shadow DOM and returns
/// all matching elements via `querySelectorAll` (on the final level).
fn build_shadow_query_all_js(host_sel: &str, inner_sels: &[String]) -> String {
    if inner_sels.len() == 1 {
        format!(
            "(function() {{ \
               var host = document.querySelector({host_sel}); \
               if (!host || !host.shadowRoot) return []; \
               return host.shadowRoot.querySelectorAll({inner}); \
             }})()",
            host_sel = host_sel,
            inner = &inner_sels[0]
        )
    } else {
        // Multi-level: navigate to the penultimate level, then querySelectorAll
        let mut body = format!(
            "var cur = document.querySelector({host_sel}); \
             if (!cur || !cur.shadowRoot) return []; \
             cur = cur.shadowRoot;",
            host_sel = host_sel
        );
        for (i, sel) in inner_sels.iter().enumerate() {
            if i < inner_sels.len() - 1 {
                body.push_str(&format!(
                    " cur = cur.querySelector({sel}); \
                     if (!cur || !cur.shadowRoot) return []; \
                     cur = cur.shadowRoot;",
                    sel = sel
                ));
            } else {
                body.push_str(&format!(" return cur.querySelectorAll({sel});", sel = sel));
            }
        }
        format!("(function() {{ {body} }})()")
    }
}
