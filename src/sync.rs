//! rpage 同步封装 — 零 await，像 DrissionPage 一样用
//!
//! ```
//! use rpage::sync::SyncPage;
//!
//! let page = SyncPage::connect("http://127.0.0.1:9222").unwrap();
//! page.get("https://example.com").unwrap();
//! let title = page.title().unwrap();
//! let el = page.ele("h1").unwrap();
//! println!("{}", el.text());
//! el.click().unwrap();
//! ```

use crate::element::Element;
use crate::chromium_page::{ChromiumPage, CookieInfo, FrameContext, InterceptGuard, PdfOptions};
use crate::error::{Error, Result};
use crate::agent::{ActionAttempt, InteractiveElement, PageSnapshot, PageSummary};
use crate::download::DownloadInfo;
use crate::wait::WaitOptions;
use std::time::Duration;

/// 同步 Page — 内部持有 tokio runtime，所有方法零 await
pub struct SyncPage {
    inner: ChromiumPage,
    rt: tokio::runtime::Runtime,
}

impl SyncPage {
    // ── 构造 ──────────────────────────────────────────

    /// 启动浏览器并接管（同步）
    pub fn new() -> Result<Self> {
        let rt = tokio::runtime::Runtime::new().map_err(|e| Error::Browser(format!("runtime: {e}")))?;
        let inner = rt.block_on(ChromiumPage::new())?;
        Ok(Self { inner, rt })
    }

    /// 用自定义选项启动浏览器（同步）
    pub fn with_options(opts: crate::config::ChromiumOptions) -> Result<Self> {
        let rt = tokio::runtime::Runtime::new().map_err(|e| Error::Browser(format!("runtime: {e}")))?;
        let inner = rt.block_on(ChromiumPage::with_options(opts))?;
        Ok(Self { inner, rt })
    }

    /// 连接已运行的 Chrome（同步）
    pub fn connect(debug_url: &str) -> Result<Self> {
        let rt = tokio::runtime::Runtime::new().map_err(|e| Error::Browser(format!("runtime: {e}")))?;
        let inner = rt.block_on(ChromiumPage::connect(debug_url))?;
        Ok(Self { inner, rt })
    }

    /// 连接已运行的 Chrome（带选项）
    pub fn connect_with_opts(debug_url: &str, opts: crate::config::ChromiumOptions) -> Result<Self> {
        let rt = tokio::runtime::Runtime::new().map_err(|e| Error::Browser(format!("runtime: {e}")))?;
        let inner = rt.block_on(ChromiumPage::connect_with_opts(debug_url, opts))?;
        Ok(Self { inner, rt })
    }

    // ── 内部辅助 ──────────────────────────────────────

    #[inline]
    fn rt(&self) -> &tokio::runtime::Runtime { &self.rt }

    // ── 导航 ──────────────────────────────────────────

    pub fn get(&self, url: &str) -> Result<()> { self.rt().block_on(self.inner.get(url)) }
    pub fn goto(&self, url: &str) -> Result<&Self> { self.rt().block_on(self.inner.goto(url))?; Ok(self) }
    pub fn refresh(&self) -> Result<()> { self.rt().block_on(self.inner.refresh()) }
    pub fn back(&self) -> Result<()> { self.rt().block_on(self.inner.back()) }
    pub fn forward(&self) -> Result<()> { self.rt().block_on(self.inner.forward()) }
    pub fn get_and_wait(&self, url: &str, timeout_secs: u64) -> Result<()> {
        self.rt().block_on(self.inner.get_and_wait(url, timeout_secs))
    }

    // ── 页面信息 ──────────────────────────────────────

    pub fn title(&self) -> Result<String> { self.rt().block_on(self.inner.title()) }
    pub fn url(&self) -> Result<String> { self.rt().block_on(self.inner.url()) }
    pub fn current_url(&self) -> Result<String> { self.rt().block_on(self.inner.current_url()) }
    pub fn current_title(&self) -> Result<String> { self.rt().block_on(self.inner.current_title()) }
    pub fn html(&self) -> Result<String> { self.rt().block_on(self.inner.html()) }
    pub fn page_source(&self) -> Result<String> { self.rt().block_on(self.inner.page_source()) }
    pub fn content_type(&self) -> Result<String> { self.rt().block_on(self.inner.get_content_type()) }

    // ── 元素查找 ──────────────────────────────────────

    pub fn ele(&self, selector: &str) -> Result<SyncElement> {
        let el = self.rt().block_on(self.inner.ele(selector))?;
        Ok(SyncElement { inner: el, rt: self.rt().handle().clone() })
    }

    pub fn eles(&self, selector: &str) -> Result<Vec<SyncElement>> {
        let els = self.rt().block_on(self.inner.eles(selector))?;
        let handle = self.rt().handle().clone();
        Ok(els.into_iter().map(|e| SyncElement { inner: e, rt: handle.clone() }).collect())
    }

    pub fn ele_or_none(&self, selector: &str) -> Option<SyncElement> {
        self.rt().block_on(self.inner.ele_or_none(selector))
            .map(|e| SyncElement { inner: e, rt: self.rt().handle().clone() })
    }

    pub fn exists(&self, selector: &str) -> bool { self.rt().block_on(self.inner.exists(selector)) }
    pub fn count(&self, selector: &str) -> usize { self.rt().block_on(self.inner.count(selector)) }
    pub fn ele_count(&self, selector: &str) -> Result<usize> { self.rt().block_on(self.inner.ele_count(selector)) }

    // ── 快捷操作（链式返回 &Self）──────────────────────

    pub fn click_ele(&self, selector: &str) -> Result<&Self> { self.rt().block_on(self.inner.click_ele(selector))?; Ok(self) }
    pub fn type_text(&self, selector: &str, text: &str) -> Result<&Self> { self.rt().block_on(self.inner.type_text(selector, text))?; Ok(self) }
    pub fn input_text(&self, selector: &str, text: &str) -> Result<&Self> { self.rt().block_on(self.inner.input_text(selector, text))?; Ok(self) }
    pub fn hover_ele(&self, selector: &str) -> Result<&Self> { self.rt().block_on(self.inner.hover_ele(selector))?; Ok(self) }
    pub fn scroll_to_ele(&self, selector: &str) -> Result<&Self> { self.rt().block_on(self.inner.scroll_to_ele(selector))?; Ok(self) }
    pub fn get_text(&self, selector: &str) -> Result<String> { self.rt().block_on(self.inner.get_text(selector)) }
    pub fn get_attr(&self, selector: &str, attr: &str) -> Result<Option<String>> { self.rt().block_on(self.inner.get_attr(selector, attr)) }

    // ── JS 执行 ───────────────────────────────────────

    pub fn execute(&self, js: &str) -> Result<serde_json::Value> { self.rt().block_on(self.inner.execute(js)) }
    pub fn run_async_js(&self, expr: &str) -> Result<serde_json::Value> { self.rt().block_on(self.inner.run_async_js(expr)) }
    pub fn run_js_with_args(&self, expr: &str, args: serde_json::Value) -> Result<serde_json::Value> {
        self.rt().block_on(self.inner.run_js_with_args(expr, args))
    }
    pub fn evaluate_on_new_document(&self, js: &str) -> Result<()> { self.rt().block_on(self.inner.evaluate_on_new_document(js)) }
    pub fn add_init_script(&self, name: &str, js: &str) -> Result<()> { self.rt().block_on(self.inner.add_init_script(name, js)) }
    pub fn remove_init_script(&self, name: &str) -> Result<()> { self.rt().block_on(self.inner.remove_init_script(name)) }

    // ── 等待 ──────────────────────────────────────────

    pub fn wait_ele(&self, selector: &str, timeout_secs: u64) -> Result<SyncElement> {
        let el = self.rt().block_on(self.inner.wait_ele(selector, timeout_secs))?;
        Ok(SyncElement { inner: el, rt: self.rt().handle().clone() })
    }
    pub fn wait_ele_hidden(&self, selector: &str, timeout_secs: u64) -> Result<()> { self.rt().block_on(self.inner.wait_ele_hidden(selector, timeout_secs)) }
    pub fn wait_ele_deleted(&self, selector: &str, timeout_secs: u64) -> Result<()> { self.rt().block_on(self.inner.wait_ele_deleted(selector, timeout_secs)) }
    pub fn wait_title_contains(&self, text: &str, timeout_secs: u64) -> Result<()> { self.rt().block_on(self.inner.wait_title_contains(text, timeout_secs)) }
    pub fn wait_url_contains(&self, text: &str, timeout_secs: u64) -> Result<()> { self.rt().block_on(self.inner.wait_url_contains(text, timeout_secs)) }
    pub fn wait_url_is(&self, expected: &str, timeout_secs: u64) -> Result<()> { self.rt().block_on(self.inner.wait_url_is(expected, timeout_secs)) }
    pub fn wait_title_is(&self, expected: &str, timeout_secs: u64) -> Result<()> { self.rt().block_on(self.inner.wait_title_is(expected, timeout_secs)) }
    pub fn wait_js(&self, expr: &str, timeout_secs: u64) -> Result<()> { self.rt().block_on(self.inner.wait_js(expr, timeout_secs)) }
    pub fn wait_for_navigation(&self, expected_url: &str, timeout_secs: u64) -> Result<()> { self.rt().block_on(self.inner.wait_for_navigation(expected_url, std::time::Duration::from_secs(timeout_secs))) }
    pub fn wait_new_tab(&self, timeout_secs: u64) -> Result<()> { self.rt().block_on(self.inner.wait_new_tab(timeout_secs)) }
    pub fn wait_download(&self, timeout_secs: u64) -> Result<DownloadInfo> { self.rt().block_on(self.inner.wait_download(timeout_secs)) }

    // ── 截图 / PDF ────────────────────────────────────

    pub fn screenshot_bytes(&self) -> Result<Vec<u8>> { self.rt().block_on(self.inner.screenshot_bytes()) }
    pub fn screenshot(&self, path: &str) -> Result<()> { self.rt().block_on(self.inner.screenshot(path)) }
    pub fn pdf(&self, path: &str) -> Result<()> { self.rt().block_on(self.inner.pdf(path)) }
    pub fn pdf_bytes(&self, opts: PdfOptions) -> Result<Vec<u8>> { self.rt().block_on(self.inner.pdf_bytes(opts)) }
    pub fn pdf_to_file(&self, path: &str, opts: PdfOptions) -> Result<()> { self.rt().block_on(self.inner.pdf_to_file(path, opts)) }

    // ── Cookie ────────────────────────────────────────

    pub fn cookies(&self) -> Result<Vec<CookieInfo>> { self.rt().block_on(self.inner.cookies()) }
    pub fn set_cookie(&self, cookie: CookieInfo) -> Result<()> { self.rt().block_on(self.inner.set_cookie(cookie)) }
    pub fn delete_cookie(&self, name: &str) -> Result<()> { self.rt().block_on(self.inner.delete_cookie(name)) }
    pub fn clear_cookies(&self) -> Result<()> { self.rt().block_on(self.inner.clear_cookies()) }
    pub fn share_cookies_to(&self, other: &SyncPage) -> Result<()> { self.rt().block_on(self.inner.share_cookies_to(&other.inner)) }

    // ── 标签页 ────────────────────────────────────────

    pub fn tabs(&self) -> Result<Vec<SyncPage>> {
        // tabs() 返回 Vec<Page> (chromiumoxide Page)，不是 ChromiumPage
        // 暂时返回标题列表更实用
        let titles = self.rt().block_on(self.inner.tab_titles())?;
        // 无法直接构造 SyncPage from Page，提供便捷方法
        Err(Error::Browser(format!("共 {} 个标签: {:?}", titles.len(), titles)))
    }
    pub fn tab_titles(&self) -> Result<Vec<String>> { self.rt().block_on(self.inner.tab_titles()) }
    pub fn tab_urls(&self) -> Result<Vec<String>> { self.rt().block_on(self.inner.tab_urls()) }
    pub fn switch_to_tab(&self, index: usize) -> Result<()> { self.rt().block_on(self.inner.switch_to_tab(index)) }
    pub fn close_tab(&self, index: usize) -> Result<()> { self.rt().block_on(self.inner.close_tab(index)) }
    pub fn new_tab(&self) -> Result<()> { self.rt().block_on(self.inner.new_tab())?; Ok(()) }
    pub fn get_tab_by_title(&self, title: &str) -> Result<usize> { self.rt().block_on(self.inner.get_tab_by_title(title)) }
    pub fn get_tab_by_url(&self, url: &str) -> Result<usize> { self.rt().block_on(self.inner.get_tab_by_url(url)) }

    // ── 滚动 ──────────────────────────────────────────

    pub fn scroll_to(&self, x: u32, y: u32) -> Result<()> { self.rt().block_on(self.inner.scroll_to(x, y)) }
    pub fn scroll_to_top(&self) -> Result<()> { self.rt().block_on(self.inner.scroll_to_top()) }
    pub fn scroll_to_bottom(&self) -> Result<()> { self.rt().block_on(self.inner.scroll_to_bottom()) }
    pub fn scroll_up(&self, pixels: u32) -> Result<()> { self.rt().block_on(self.inner.scroll_up(pixels)) }
    pub fn scroll_down(&self, pixels: u32) -> Result<()> { self.rt().block_on(self.inner.scroll_down(pixels)) }
    pub fn scroll_by(&self, x: i64, y: i64) -> Result<()> { self.rt().block_on(self.inner.scroll_by(x, y)) }
    pub fn smooth_scroll(&self, x: i64, y: i64, duration_ms: u64) -> Result<()> { self.rt().block_on(self.inner.smooth_scroll(x, y, duration_ms)) }

    // ── 键盘 ──────────────────────────────────────────

    pub fn press(&self, key: &str) -> Result<()> { self.rt().block_on(self.inner.press(key)) }
    pub fn keys(&self, text: &str) -> Result<()> { self.rt().block_on(self.inner.keys(text)) }

    // ── 网络 ──────────────────────────────────────────

    pub fn set_extra_headers(&self, headers: std::collections::HashMap<String, String>) -> Result<()> { self.rt().block_on(self.inner.set_extra_headers(headers)) }
    pub fn set_user_agent(&self, ua: &str) -> Result<()> { self.rt().block_on(self.inner.set_user_agent(ua)) }
    pub fn set_proxy_auth(&self, user: &str, pass: &str) -> Result<()> { self.rt().block_on(self.inner.set_proxy_auth(user, pass)) }
    pub fn set_blocked_urls(&self, urls: &[&str]) -> Result<()> { self.rt().block_on(self.inner.set_blocked_urls(urls)) }
    pub fn set_offline(&self, offline: bool) -> Result<()> { self.rt().block_on(self.inner.set_offline(offline)) }
    pub fn clear_cache(&self) -> Result<()> { self.rt().block_on(self.inner.clear_cache()) }
    pub fn links(&self) -> Result<Vec<String>> { self.rt().block_on(self.inner.links()) }
    pub fn images(&self) -> Result<Vec<String>> { self.rt().block_on(self.inner.images()) }
    pub fn disable_images(&self) -> Result<()> { self.rt().block_on(self.inner.disable_images()) }
    pub fn download(&self, url: &str, timeout_secs: u64) -> Result<DownloadInfo> { self.rt().block_on(self.inner.download(url, timeout_secs)) }

    // ── 视口 / 窗口 / 设备 ────────────────────────────

    pub fn set_viewport(&self, w: u32, h: u32) -> Result<()> { self.rt().block_on(self.inner.set_viewport(w, h)) }
    pub fn set_device_scale(&self, scale: f64) -> Result<()> { self.rt().block_on(self.inner.set_device_scale(scale)) }
    pub fn set_touch(&self, enabled: bool) -> Result<()> { self.rt().block_on(self.inner.set_touch(enabled)) }
    pub fn set_geolocation(&self, lat: f64, lng: f64) -> Result<()> { self.rt().block_on(self.inner.set_geolocation(lat, lng)) }
    pub fn set_timezone(&self, tz: &str) -> Result<()> { self.rt().block_on(self.inner.set_timezone(tz)) }
    pub fn set_window_position(&self, left: i32, top: i32) -> Result<()> { self.rt().block_on(self.inner.set_window_position(left, top)) }
    pub fn set_window_size(&self, w: u32, h: u32) -> Result<()> { self.rt().block_on(self.inner.set_window_size(w, h)) }
    pub fn get_window_bounds(&self) -> Result<(i32, i32, u32, u32)> { self.rt().block_on(self.inner.get_window_bounds()) }
    pub fn minimize(&self) -> Result<()> { self.rt().block_on(self.inner.minimize()) }
    pub fn maximize(&self) -> Result<()> { self.rt().block_on(self.inner.maximize()) }
    pub fn fullscreen(&self) -> Result<()> { self.rt().block_on(self.inner.fullscreen()) }
    pub fn emulate_device(&self, width: u32, height: u32, ua: &str, scale: f64, touch: bool) -> Result<()> { self.rt().block_on(self.inner.emulate_device(width, height, ua, scale, touch)) }

    // ── 剪贴板 ────────────────────────────────────────

    pub fn clipboard_read(&self) -> Result<String> { self.rt().block_on(self.inner.clipboard_read()) }
    pub fn clipboard_write(&self, text: &str) -> Result<()> { self.rt().block_on(self.inner.clipboard_write(text)) }

    // ── 文本操作 ──────────────────────────────────────

    pub fn select_all_text(&self) -> Result<()> { self.rt().block_on(self.inner.select_all_text()) }
    pub fn copy_text(&self) -> Result<()> { self.rt().block_on(self.inner.copy_text()) }
    pub fn paste_text(&self) -> Result<()> { self.rt().block_on(self.inner.paste_text()) }
    pub fn find_text(&self, text: &str) -> Result<bool> { self.rt().block_on(self.inner.find_text(text)) }

    // ── 弹窗 ──────────────────────────────────────────

    pub fn handle_alert(&self, accept: bool, text: Option<&str>) -> Result<()> { self.rt().block_on(self.inner.handle_alert(accept, text)) }
    pub fn accept_alert(&self) -> Result<()> { self.rt().block_on(self.inner.accept_alert()) }
    pub fn dismiss_alert(&self) -> Result<()> { self.rt().block_on(self.inner.dismiss_alert()) }
    pub fn accept_prompt(&self, text: &str) -> Result<()> { self.rt().block_on(self.inner.accept_prompt(text)) }

    // ── CSS / 样式 ────────────────────────────────────

    pub fn inject_css(&self, css: &str) -> Result<String> { self.rt().block_on(self.inner.inject_css(css)) }
    pub fn remove_css(&self, id: &str) -> Result<()> { self.rt().block_on(self.inner.remove_css(id)) }

    // ── iframe ────────────────────────────────────────

    pub fn frame_html(&self, selector: &str) -> Result<String> { self.rt().block_on(self.inner.frame_html(selector)) }
    pub fn frame_execute(&self, selector: &str, js: &str) -> Result<serde_json::Value> { self.rt().block_on(self.inner.frame_execute(selector, js)) }
    pub fn enter_frame(&self, selector: &str) -> Result<FrameContext> { self.rt().block_on(self.inner.enter_frame(selector)) }

    // ── 性能 ──────────────────────────────────────────

    pub fn performance_metrics(&self) -> Result<Vec<(String, f64)>> { self.rt().block_on(self.inner.performance_metrics()) }
    pub fn page_timing(&self) -> Result<std::collections::HashMap<String, f64>> { self.rt().block_on(self.inner.page_timing()) }
    pub fn dom_snapshot(&self) -> Result<serde_json::Value> { self.rt().block_on(self.inner.dom_snapshot()) }

    // ── 权限 / 设备 ───────────────────────────────────

    pub fn grant_permissions(&self, origin: &str, perms: Vec<String>) -> Result<()> { self.rt().block_on(self.inner.grant_permissions(origin, perms)) }
    pub fn reset_permissions(&self) -> Result<()> { self.rt().block_on(self.inner.reset_permissions()) }
    pub fn mute(&self) -> Result<()> { self.rt().block_on(self.inner.mute()) }
    pub fn unmute(&self) -> Result<()> { self.rt().block_on(self.inner.unmute()) }

    // ── 下载 ──────────────────────────────────────────

    pub fn set_download_file_name(&self, name: &str) -> Result<()> { self.rt().block_on(self.inner.set_download_file_name(name)) }
    pub fn wait_for_download_file(&self, dir: &str, timeout_secs: u64) -> Result<DownloadInfo> { self.rt().block_on(self.inner.wait_for_download_file(dir, timeout_secs)) }
    pub fn set_file_chooser(&self, enabled: bool) { self.rt().block_on(self.inner.set_file_chooser(enabled)) }
    pub fn wait_file_chooser(&self, timeout_secs: u64) -> Result<crate::chromium_page::FileChooserInfo> { self.rt().block_on(self.inner.wait_file_chooser(timeout_secs)) }

    // ── 网络/位置 ─────────────────────────────────────

    pub fn set_location_and_reload(&self, lat: f64, lng: f64) -> Result<()> { self.rt().block_on(self.inner.set_location_and_reload(lat, lng)) }

    // ── Agent API ─────────────────────────────────────

    pub fn interactive_elements(&self) -> Result<Vec<InteractiveElement>> { self.rt().block_on(self.inner.interactive_elements()) }
    pub fn page_summary(&self) -> Result<PageSummary> { self.rt().block_on(self.inner.page_summary()) }
    pub fn page_snapshot(&self) -> Result<PageSnapshot> { self.rt().block_on(self.inner.page_snapshot()) }
    pub fn smart_click(&self, target: &str) -> ActionAttempt { self.rt().block_on(self.inner.smart_click(target)) }
    pub fn smart_fill(&self, field: &str, value: &str) -> ActionAttempt { self.rt().block_on(self.inner.smart_fill(field, value)) }

    // ── 生命周期 ──────────────────────────────────────

    pub fn sleep(&self, dur: Duration) { self.rt().block_on(self.inner.sleep(dur)) }
    pub fn close(&self) -> Result<()> { self.rt().block_on(self.inner.close()) }
    pub fn quit(&self) -> Result<()> { self.rt().block_on(self.inner.quit()) }
    // reconnect() 需要 &mut self，与 SyncPage 的 &self 模式冲突
    // 如需重连请直接重建 SyncPage::connect()
    pub fn clone_session(&self) -> Result<SyncPage> {
        let inner = self.rt().block_on(self.inner.clone_session())?;
        Ok(SyncPage { inner, rt: tokio::runtime::Runtime::new().map_err(|e| Error::Browser(format!("runtime: {e}")))? })
    }

    pub fn is_connected(&self) -> bool { self.inner.is_connected() }
    pub fn debug_url(&self) -> &str { self.inner.debug_url() }

    // ── 拦截 ──────────────────────────────────────────

    pub fn enable_intercept(&self, pattern: &str) -> Result<InterceptGuard> { self.rt().block_on(self.inner.enable_intercept(pattern)) }

    // ── 刷新元素 ──────────────────────────────────────

    pub fn refresh_ele(&self, el: &SyncElement) -> Result<SyncElement> {
        let refreshed = self.rt().block_on(self.inner.refresh_ele(&el.inner))?;
        Ok(SyncElement { inner: refreshed, rt: self.rt().handle().clone() })
    }
}

/// 同步 Element — 内部持有 tokio handle，所有方法零 await
pub struct SyncElement {
    inner: Element,
    rt: tokio::runtime::Handle,
}

impl SyncElement {
    // ── 同步方法（直接委托）────────────────────────────

    pub fn tag(&self) -> &str { self.inner.tag() }
    pub fn text(&self) -> &str { self.inner.text() }
    pub fn html(&self) -> &str { self.inner.html() }
    pub fn attr(&self, name: &str) -> Option<&str> { self.inner.attr(name) }
    pub fn attrs(&self) -> &[(String, String)] { self.inner.attrs() }
    pub fn is_displayed(&self) -> bool { self.inner.is_displayed() }
    pub fn is_enabled(&self) -> bool { self.inner.is_enabled() }
    pub fn is_cdp(&self) -> bool { self.inner.is_cdp() }

    // ── 异步方法（block_on 包装）────────────────────────

    pub fn click(&self) -> Result<()> { self.rt.block_on(self.inner.click()) }
    pub fn input(&self, text: &str) -> Result<()> { self.rt.block_on(self.inner.input(text)) }
    pub fn fill(&self, text: &str) -> Result<()> { self.rt.block_on(self.inner.fill(text)) }
    pub fn clear(&self) -> Result<()> { self.rt.block_on(self.inner.clear()) }
    pub fn hover(&self) -> Result<()> { self.rt.block_on(self.inner.hover()) }
    pub fn scroll_into_view(&self) -> Result<()> { self.rt.block_on(self.inner.scroll_into_view()) }
    pub fn press_key(&self, key: &str) -> Result<()> { self.rt.block_on(self.inner.press_key(key)) }
    pub fn right_click(&self) -> Result<()> { self.rt.block_on(self.inner.right_click()) }
    pub fn double_click(&self) -> Result<()> { self.rt.block_on(self.inner.double_click()) }
    pub fn submit(&self) -> Result<()> { self.rt.block_on(self.inner.submit()) }
    pub fn check(&self) -> Result<()> { self.rt.block_on(self.inner.check()) }
    pub fn uncheck(&self) -> Result<()> { self.rt.block_on(self.inner.uncheck()) }
    pub fn focus(&self) -> Result<()> { self.rt.block_on(self.inner.focus()) }
    pub fn blur(&self) -> Result<()> { self.rt.block_on(self.inner.blur()) }

    pub fn value(&self) -> Result<String> { self.rt.block_on(self.inner.value()) }
    pub fn rect(&self) -> Result<(f64, f64, f64, f64)> { self.rt.block_on(self.inner.rect()) }
    pub fn bounding_box(&self) -> Result<(f64, f64, f64, f64)> { self.rt.block_on(self.inner.bounding_box()) }
    pub fn is_selected(&self) -> Result<bool> { self.rt.block_on(self.inner.is_selected()) }
    pub fn is_visible(&self) -> bool { self.rt.block_on(self.inner.is_visible()) }
    pub fn style(&self, prop: &str) -> Result<String> { self.rt.block_on(self.inner.style(prop)) }

    pub fn select(&self, text: &str) -> Result<()> { self.rt.block_on(self.inner.select(text)) }
    pub fn select_by_value(&self, val: &str) -> Result<()> { self.rt.block_on(self.inner.select_by_value(val)) }
    pub fn select_option(&self, val: &str) -> Result<()> { self.rt.block_on(self.inner.select_option(val)) }
    pub fn select_text(&self) -> Result<()> { self.rt.block_on(self.inner.select_text()) }
    pub fn upload_file(&self, path: &str) -> Result<()> { self.rt.block_on(self.inner.upload_file(path)) }
    pub fn upload_files(&self, paths: &[&str]) -> Result<()> { self.rt.block_on(self.inner.upload_files(paths)) }

    pub fn screenshot(&self, path: &str) -> Result<()> { self.rt.block_on(self.inner.screenshot(path)) }
    pub fn screenshot_bytes(&self) -> Result<Vec<u8>> { self.rt.block_on(self.inner.screenshot_bytes()) }

    pub fn drag_to(&self, target: &SyncElement) -> Result<()> { self.rt.block_on(self.inner.drag_to(&target.inner)) }
    pub fn drag_to_offset(&self, x: f64, y: f64) -> Result<()> { self.rt.block_on(self.inner.drag_to_offset(x, y)) }

    pub fn set_attr(&self, name: &str, val: &str) -> Result<()> { self.rt.block_on(self.inner.set_attr(name, val)) }
    pub fn set_style(&self, prop: &str, val: &str) -> Result<()> { self.rt.block_on(self.inner.set_style(prop, val)) }
    pub fn add_class(&self, class: &str) -> Result<()> { self.rt.block_on(self.inner.add_class(class)) }
    pub fn remove_class(&self, class: &str) -> Result<()> { self.rt.block_on(self.inner.remove_class(class)) }
    pub fn has_class(&self, class: &str) -> Result<bool> { self.rt.block_on(self.inner.has_class(class)) }

    pub fn parent(&self) -> Result<SyncElement> {
        let el = self.rt.block_on(self.inner.parent())?;
        Ok(SyncElement { inner: el, rt: self.rt.clone() })
    }
    pub fn first_child(&self) -> Result<SyncElement> {
        let el = self.rt.block_on(self.inner.first_child())?;
        Ok(SyncElement { inner: el, rt: self.rt.clone() })
    }
    pub fn next(&self) -> Result<SyncElement> {
        let el = self.rt.block_on(self.inner.next())?;
        Ok(SyncElement { inner: el, rt: self.rt.clone() })
    }
    pub fn prev(&self) -> Result<SyncElement> {
        let el = self.rt.block_on(self.inner.prev())?;
        Ok(SyncElement { inner: el, rt: self.rt.clone() })
    }
    pub fn scroll_to_top(&self) -> Result<()> { self.rt.block_on(self.inner.scroll_to_top()) }

    pub fn js(&self, script: &str) -> Result<()> { self.rt.block_on(self.inner.js(script)) }

    pub fn shadow_ele(&self, selector: &str) -> Result<SyncElement> {
        let el = self.rt.block_on(self.inner.shadow_ele(selector))?;
        Ok(SyncElement { inner: el, rt: self.rt.clone() })
    }
    pub fn shadow_eles(&self, selector: &str) -> Result<Vec<SyncElement>> {
        let els = self.rt.block_on(self.inner.shadow_eles(selector))?;
        Ok(els.into_iter().map(|e| SyncElement { inner: e, rt: self.rt.clone() }).collect())
    }

    // ── 等待 ──────────────────────────────────────────

    pub fn wait_for_visible(&self) -> Result<()> { self.rt.block_on(self.inner.wait_for_visible()) }
    pub fn wait_for_visible_with_timeout(&self, timeout: Duration) -> Result<()> { self.rt.block_on(self.inner.wait_for_visible_with_timeout(timeout)) }
    pub fn wait_for_hidden(&self) -> Result<()> { self.rt.block_on(self.inner.wait_for_hidden()) }
    pub fn wait_for_hidden_with_timeout(&self, timeout: Duration) -> Result<()> { self.rt.block_on(self.inner.wait_for_hidden_with_timeout(timeout)) }
    pub fn wait_for_enabled(&self) -> Result<()> { self.rt.block_on(self.inner.wait_for_enabled()) }
    pub fn wait_for_enabled_with_timeout(&self, timeout: Duration) -> Result<()> { self.rt.block_on(self.inner.wait_for_enabled_with_timeout(timeout)) }

    // ── 子元素查找 ─────────────────────────────────────

    pub fn ele(&self, selector: &str) -> Result<SyncElement> {
        let el = self.inner.ele(selector)?;
        Ok(SyncElement { inner: el, rt: self.rt.clone() })
    }
    pub fn eles(&self, selector: &str) -> Result<Vec<SyncElement>> {
        let els = self.inner.eles(selector)?;
        Ok(els.into_iter().map(|e| SyncElement { inner: e, rt: self.rt.clone() }).collect())
    }
}
