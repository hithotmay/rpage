//! Synchronous wrapper around `ChromiumPage`.
//!
//! Holds an internal `tokio::runtime::Runtime` and wraps every async method
//! with `runtime.block_on()`, so callers don't need `#[tokio::main]`.

use std::path::PathBuf;
use std::time::Duration;

use crate::agent::{ActionAttempt, InteractiveElement, PageSnapshot, PageSummary};
use crate::chromium_page::{
    ActionChain, ChromiumPage, CookieInfo, FileChooserInfo, FrameContext, InterceptGuard,
};
use crate::config::ChromiumOptions;
use crate::download::DownloadInfo;
use crate::element::Element;
use crate::error::Result;
use crate::network::{RequestInfo, ResponseInfo};

/// Synchronous browser page — wraps `ChromiumPage` with an internal tokio runtime.
pub struct SyncPage {
    inner: ChromiumPage,
    rt: tokio::runtime::Runtime,
}

// ── constructors ──────────────────────────────────────────────────────

impl SyncPage {
    /// Launch a browser and create a synchronous page handle.
    pub fn new() -> Result<Self> {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| crate::error::Error::Browser(e.to_string()))?;
        let inner = rt.block_on(ChromiumPage::new())?;
        Ok(Self { inner, rt })
    }

    /// Launch with custom options.
    pub fn with_options(opts: ChromiumOptions) -> Result<Self> {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| crate::error::Error::Browser(e.to_string()))?;
        let inner = rt.block_on(ChromiumPage::with_options(opts))?;
        Ok(Self { inner, rt })
    }

    /// Connect to an already-running Chrome debug port.
    pub fn connect(debug_url: &str) -> Result<Self> {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| crate::error::Error::Browser(e.to_string()))?;
        let inner = rt.block_on(ChromiumPage::connect(debug_url))?;
        Ok(Self { inner, rt })
    }

    /// Connect with custom options.
    pub fn connect_with_opts(debug_url: &str, opts: ChromiumOptions) -> Result<Self> {
        let rt = tokio::runtime::Runtime::new()
            .map_err(|e| crate::error::Error::Browser(e.to_string()))?;
        let inner = rt.block_on(ChromiumPage::connect_with_opts(debug_url, opts))?;
        Ok(Self { inner, rt })
    }
}

// ── helper: run async closure on our runtime ──────────────────────────

impl SyncPage {
    #[inline]
    fn rt(&self) -> &tokio::runtime::Runtime {
        &self.rt
    }

    #[inline]
    fn page(&self) -> &ChromiumPage {
        &self.inner
    }
}

// ── macro to reduce boilerplate for simple async wrappers ─────────────

macro_rules! sync_fn {
    // &self, no extra args
    ($vis:vis fn $name:ident(&self $(,)? ) -> $ret:ty ) => {
        $vis fn $name(&self) -> $ret {
            self.rt().block_on(self.page().$name())
        }
    };
    // &self, with args — just forward
    ($vis:vis fn $name:ident(&self, $($arg:ident : $aty:ty),* $(,)? ) -> $ret:ty ) => {
        $vis fn $name(&self, $($arg: $aty),*) -> $ret {
            self.rt().block_on(self.page().$name($($arg),*))
        }
    };
}

// ── navigation ────────────────────────────────────────────────────────

impl SyncPage {
    sync_fn!(pub fn get(&self, url: &str) -> Result<()>);
    sync_fn!(pub fn refresh(&self) -> Result<()>);
    sync_fn!(pub fn back(&self) -> Result<()>);
    sync_fn!(pub fn forward(&self) -> Result<()>);

    pub fn sleep(&self, dur: Duration) {
        self.rt().block_on(self.page().sleep(dur));
    }

    sync_fn!(pub fn close(&self) -> Result<()>);

    pub fn reconnect(&mut self) -> Result<()> {
        self.rt.block_on(self.inner.reconnect())
    }

    sync_fn!(pub fn quit(&self) -> Result<()>);

    pub fn is_connected(&self) -> bool {
        self.page().is_connected()
    }

    pub fn debug_url(&self) -> &str {
        self.page().debug_url()
    }
}

// ── element finding ───────────────────────────────────────────────────

impl SyncPage {
    sync_fn!(pub fn ele(&self, locator_str: &str) -> Result<Element>);
    sync_fn!(pub fn eles(&self, locator_str: &str) -> Result<Vec<Element>>);
    sync_fn!(pub fn shadow_ele(&self, locator_str: &str) -> Result<Element>);
    sync_fn!(pub fn shadow_eles(&self, locator_str: &str) -> Result<Vec<Element>>);
    sync_fn!(pub fn ele_count(&self, locator_str: &str) -> Result<usize>);
}

// ── page info ─────────────────────────────────────────────────────────

impl SyncPage {
    sync_fn!(pub fn html(&self) -> Result<String>);
    sync_fn!(pub fn title(&self) -> Result<String>);
    sync_fn!(pub fn url(&self) -> Result<String>);
}

// ── JavaScript ────────────────────────────────────────────────────────

impl SyncPage {
    sync_fn!(pub fn execute(&self, js: &str) -> Result<serde_json::Value>);
    sync_fn!(pub fn run_async_js(&self, expression: &str) -> Result<serde_json::Value>);

    pub fn run_js_with_args(
        &self,
        expression: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value> {
        self.rt().block_on(self.page().run_js_with_args(expression, args))
    }

    sync_fn!(pub fn evaluate_on_new_document(&self, js: &str) -> Result<()>);
    sync_fn!(pub fn add_init_script(&self, name: &str, js: &str) -> Result<()>);
    sync_fn!(pub fn remove_init_script(&self, name: &str) -> Result<()>);

    pub fn list_init_scripts(&self) -> Vec<String> {
        self.page().list_init_scripts()
    }
}

// ── clipboard / text ─────────────────────────────────────────────────

impl SyncPage {
    sync_fn!(pub fn clipboard_read(&self) -> Result<String>);
    sync_fn!(pub fn clipboard_write(&self, text: &str) -> Result<()>);
    sync_fn!(pub fn select_all_text(&self) -> Result<()>);
    sync_fn!(pub fn copy_text(&self) -> Result<()>);
    sync_fn!(pub fn paste_text(&self) -> Result<()>);
    sync_fn!(pub fn find_text(&self, text: &str) -> Result<bool>);
}

// ── screenshots / PDF ────────────────────────────────────────────────

impl SyncPage {
    sync_fn!(pub fn screenshot_bytes(&self) -> Result<Vec<u8>>);
    sync_fn!(pub fn screenshot(&self, path: &str) -> Result<()>);
}

// ── cookies ───────────────────────────────────────────────────────────

impl SyncPage {
    sync_fn!(pub fn cookies(&self) -> Result<Vec<CookieInfo>>);
    sync_fn!(pub fn set_cookie(&self, cookie: CookieInfo) -> Result<()>);
    sync_fn!(pub fn delete_cookie(&self, name: &str) -> Result<()>);
    sync_fn!(pub fn clear_cookies(&self) -> Result<()>);
}

// ── tabs ──────────────────────────────────────────────────────────────

impl SyncPage {
    sync_fn!(pub fn tabs(&self) -> Result<Vec<chromiumoxide::Page>>);
    sync_fn!(pub fn new_tab(&self) -> Result<chromiumoxide::Page>);
    sync_fn!(pub fn tab_titles(&self) -> Result<Vec<String>>);
    sync_fn!(pub fn tab_urls(&self) -> Result<Vec<String>>);
    sync_fn!(pub fn switch_to_tab(&self, index: usize) -> Result<()>);
    sync_fn!(pub fn close_tab(&self, index: usize) -> Result<()>);
    sync_fn!(pub fn get_tab_by_title(&self, title_contains: &str) -> Result<usize>);
    sync_fn!(pub fn get_tab_by_url(&self, url_contains: &str) -> Result<usize>);
    sync_fn!(pub fn wait_new_tab(&self, timeout_secs: u64) -> Result<()>);
}

// ── waits ─────────────────────────────────────────────────────────────

impl SyncPage {
    sync_fn!(pub fn wait_ele(&self, locator_str: &str, timeout_secs: u64) -> Result<Element>);
    sync_fn!(pub fn wait_ele_hidden(&self, locator_str: &str, timeout_secs: u64) -> Result<()>);
    sync_fn!(pub fn wait_ele_deleted(&self, locator_str: &str, timeout_secs: u64) -> Result<()>);
    sync_fn!(pub fn wait_title_contains(&self, text: &str, timeout_secs: u64) -> Result<()>);
    sync_fn!(pub fn wait_url_contains(&self, text: &str, timeout_secs: u64) -> Result<()>);
    sync_fn!(pub fn wait_js(&self, expression: &str, timeout_secs: u64) -> Result<()>);
    sync_fn!(pub fn wait_network_idle(&self, timeout_secs: u64, quiet_ms: u64) -> Result<()>);
}

// ── headers / user-agent ──────────────────────────────────────────────

impl SyncPage {
    sync_fn!(pub fn set_extra_headers(&self, headers: std::collections::HashMap<String, String>) -> Result<()>);
    sync_fn!(pub fn set_user_agent(&self, user_agent: &str) -> Result<()>);
    sync_fn!(pub fn set_proxy_auth(&self, user: &str, pass: &str) -> Result<()>);
}

// ── scrolling ─────────────────────────────────────────────────────────

impl SyncPage {
    sync_fn!(pub fn scroll_to(&self, x: u32, y: u32) -> Result<()>);
    sync_fn!(pub fn scroll_to_top(&self) -> Result<()>);
    sync_fn!(pub fn scroll_to_bottom(&self) -> Result<()>);
    sync_fn!(pub fn scroll_up(&self, pixels: u32) -> Result<()>);
    sync_fn!(pub fn scroll_down(&self, pixels: u32) -> Result<()>);
    sync_fn!(pub fn smooth_scroll(&self, x: i64, y: i64, duration_ms: u64) -> Result<()>);
}

// ── dialogs / alerts ─────────────────────────────────────────────────

impl SyncPage {
    sync_fn!(pub fn handle_alert(&self, accept: bool, text: Option<&str>) -> Result<()>);
    sync_fn!(pub fn accept_alert(&self) -> Result<()>);
    sync_fn!(pub fn dismiss_alert(&self) -> Result<()>);
    sync_fn!(pub fn accept_prompt(&self, text: &str) -> Result<()>);
}

// ── frames ────────────────────────────────────────────────────────────

impl SyncPage {
    sync_fn!(pub fn frame_html(&self, selector: &str) -> Result<String>);
    sync_fn!(pub fn frame_execute(&self, selector: &str, js_code: &str) -> Result<serde_json::Value>);
    sync_fn!(pub fn enter_frame(&self, selector: &str) -> Result<FrameContext>);
}

// ── download ──────────────────────────────────────────────────────────

impl SyncPage {
    sync_fn!(pub fn wait_for_download(&self, url_pattern: &str, timeout_secs: u64) -> Result<DownloadInfo>);
    sync_fn!(pub fn wait_download(&self, timeout_secs: u64) -> Result<DownloadInfo>);
    sync_fn!(pub fn download(&self, url: &str, timeout_secs: u64) -> Result<DownloadInfo>);
    sync_fn!(pub fn wait_for_download_file(&self, filename_contains: &str, timeout_secs: u64) -> Result<DownloadInfo>);
    sync_fn!(pub fn set_download_file_name(&self, name: &str) -> Result<()>);

    pub fn downloads(&self) -> Vec<DownloadInfo> {
        self.page().downloads()
    }

    pub fn download_dir(&self) -> PathBuf {
        self.page().download_dir()
    }

    pub fn set_download_dir(&self, dir: impl Into<PathBuf>) {
        self.page().set_download_dir(dir);
    }

    pub fn clear_downloads(&self) {
        self.page().clear_downloads();
    }
}

// ── window management ─────────────────────────────────────────────────

impl SyncPage {
    sync_fn!(pub fn get_window_bounds(&self) -> Result<(i32, i32, u32, u32)>);
    sync_fn!(pub fn set_window_position(&self, left: i32, top: i32) -> Result<()>);
    sync_fn!(pub fn set_window_size(&self, width: u32, height: u32) -> Result<()>);
    sync_fn!(pub fn minimize(&self) -> Result<()>);
    sync_fn!(pub fn maximize(&self) -> Result<()>);
    sync_fn!(pub fn fullscreen(&self) -> Result<()>);
}

// ── network control ───────────────────────────────────────────────────

impl SyncPage {
    sync_fn!(pub fn set_blocked_urls(&self, urls: &[&str]) -> Result<()>);
    sync_fn!(pub fn set_offline(&self, offline: bool) -> Result<()>);
    sync_fn!(pub fn clear_cache(&self) -> Result<()>);
    sync_fn!(pub fn disable_images(&self) -> Result<()>);

    pub fn links(&self) -> Result<Vec<String>> {
        self.rt().block_on(self.page().links())
    }

    pub fn images(&self) -> Result<Vec<String>> {
        self.rt().block_on(self.page().images())
    }
}

// ── device / emulation ────────────────────────────────────────────────

impl SyncPage {
    sync_fn!(pub fn set_location_and_reload(&self, lat: f64, lng: f64) -> Result<()>);
    sync_fn!(pub fn set_device_scale(&self, scale: f64) -> Result<()>);
    sync_fn!(pub fn set_touch(&self, enabled: bool) -> Result<()>);
}

// ── intercept ─────────────────────────────────────────────────────────

impl SyncPage {
    sync_fn!(pub fn enable_intercept(&self, url_pattern: &str) -> Result<InterceptGuard>);
}

// ── performance ───────────────────────────────────────────────────────

impl SyncPage {
    sync_fn!(pub fn performance_metrics(&self) -> Result<Vec<(String, f64)>>);
    sync_fn!(pub fn page_timing(&self) -> Result<std::collections::HashMap<String, f64>>);
}

// ── DOM / CSS ─────────────────────────────────────────────────────────

impl SyncPage {
    sync_fn!(pub fn dom_snapshot(&self) -> Result<serde_json::Value>);
    sync_fn!(pub fn inject_css(&self, css: &str) -> Result<String>);
    sync_fn!(pub fn remove_css(&self, id: &str) -> Result<()>);
    sync_fn!(pub fn get_content_type(&self) -> Result<String>);
}

// ── file chooser ──────────────────────────────────────────────────────

impl SyncPage {
    sync_fn!(pub fn wait_file_chooser(&self, timeout_secs: u64) -> Result<FileChooserInfo>);

    pub fn set_file_chooser(&self, enabled: bool) {
        self.rt().block_on(self.page().set_file_chooser(enabled));
    }
}

// ── audio ─────────────────────────────────────────────────────────────

impl SyncPage {
    sync_fn!(pub fn mute(&self) -> Result<()>);
    sync_fn!(pub fn unmute(&self) -> Result<()>);
}

// ── load strategy ─────────────────────────────────────────────────────

impl SyncPage {
    pub fn set_load_strategy(&mut self, strategy: &str) {
        self.inner.set_load_strategy(strategy);
    }

    pub fn load_strategy(&self) -> &str {
        self.page().load_strategy()
    }
}

// ── user data dir ─────────────────────────────────────────────────────

impl SyncPage {
    pub fn user_data_dir(&self) -> Option<&PathBuf> {
        self.page().user_data_dir()
    }
}

// ── navigation helpers ────────────────────────────────────────────────

impl SyncPage {
    sync_fn!(pub fn get_and_wait(&self, url: &str, timeout_secs: u64) -> Result<()>);
    sync_fn!(pub fn safe_back(&self) -> Result<()>);
    sync_fn!(pub fn safe_forward(&self) -> Result<()>);
    sync_fn!(pub fn safe_refresh(&self) -> Result<()>);
}

// ── Agent API (AI-friendly) ───────────────────────────────────────────

impl SyncPage {
    sync_fn!(pub fn interactive_elements(&self) -> Result<Vec<InteractiveElement>>);
    sync_fn!(pub fn page_summary(&self) -> Result<PageSummary>);
    sync_fn!(pub fn page_snapshot(&self) -> Result<PageSnapshot>);

    pub fn smart_click(&self, target: &str) -> ActionAttempt {
        self.rt().block_on(self.page().smart_click(target))
    }

    pub fn smart_fill(&self, field: &str, value: &str) -> ActionAttempt {
        self.rt().block_on(self.page().smart_fill(field, value))
    }
}

// ── listen / network monitoring ───────────────────────────────────────

impl SyncPage {
    sync_fn!(pub fn listen_start(&self) -> Result<()>);
    sync_fn!(pub fn listen_stop(&self) -> Result<()>);

    pub fn get_packets(&self, url_pattern: &str) -> Vec<RequestInfo> {
        self.page().get_packets(url_pattern)
    }

    pub fn get_responses(&self, url_pattern: &str) -> Vec<ResponseInfo> {
        self.page().get_responses(url_pattern)
    }
}

// ── action chain ──────────────────────────────────────────────────────

impl SyncPage {
    pub fn actions(&self) -> ActionChain<'_> {
        self.page().actions()
    }
}
