//! Element abstraction over CDP and parsed-HTML elements
//!
//! Elements carry an optional reference to the CDP page, enabling
//! async interactions (click, input, etc.) in Chromium mode.

use std::time::Duration;

use chromiumoxide::Page;
use scraper::Selector;

use crate::error::{Error, Result};
use crate::locator::{locator_to_selector, parse_locator, Locator};
use crate::wait::WaitOptions;

// ── Page identity ────────────────────────────────────────────

/// Identifies which backing store an element comes from.
#[derive(Debug, Clone)]
pub enum PageRef {
    /// CDP browser page (cloneable Arc-wrapped chromiumoxide::Page)
    Cdp(Page),
    /// Session mode (pure HTML, no live connection)
    Session,
}

// ── Element ──────────────────────────────────────────────────

/// An element found on a page.
///
/// In Chromium mode the element holds a clone of the CDP `Page` so it can
/// perform live interactions (click, type, evaluate JS).
/// In Session mode the element is a snapshot of parsed HTML.
#[derive(Debug, Clone)]
pub struct Element {
    /// Live page reference (CDP only)
    page: Option<Page>,
    /// Which backing store
    page_ref: PageRef,
    /// Locator used to find this element (for re-resolve)
    locator: Option<Locator>,
    /// CDP remote-object id (for direct CDP calls in Chromium mode)
    object_id: Option<String>,
    /// Outer HTML
    html: String,
    /// Tag name (lowercase)
    tag: String,
    /// Text content
    text: String,
    /// Attributes
    attrs: Vec<(String, String)>,
}

impl Element {
    // ── Constructors ─────────────────────────────────────────

    /// Create a CDP-backed element with a live page reference.
    pub fn new_cdp(
        page: Page,
        object_id: String,
        locator: Option<Locator>,
        html: String,
        tag: String,
        text: String,
        attrs: Vec<(String, String)>,
    ) -> Self {
        Self {
            page: Some(page.clone()),
            page_ref: PageRef::Cdp(page),
            locator,
            object_id: Some(object_id),
            html,
            tag,
            text,
            attrs,
        }
    }

    /// Create a session-backed element (static HTML snapshot).
    pub fn new_session(
        locator: Option<Locator>,
        html: String,
        tag: String,
        text: String,
        attrs: Vec<(String, String)>,
    ) -> Self {
        Self {
            page: None,
            page_ref: PageRef::Session,
            locator,
            object_id: None,
            html,
            tag,
            text,
            attrs,
        }
    }

    // ── Synchronous accessors ────────────────────────────────

    /// Tag name (lowercase).
    pub fn tag(&self) -> &str {
        &self.tag
    }

    /// Visible text content.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Outer HTML.
    pub fn html(&self) -> &str {
        &self.html
    }

    /// Get an attribute by name.
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
    }

    /// All attributes as `&[(String, String)]`.
    pub fn attrs(&self) -> &[(String, String)] {
        &self.attrs
    }

    /// Which page this element belongs to.
    pub fn page_ref(&self) -> &PageRef {
        &self.page_ref
    }

    /// True if backed by a live CDP page.
    pub fn is_cdp(&self) -> bool {
        self.page.is_some()
    }

    /// True if element has non-hidden style (synchronous, checks cached HTML).
    ///
    /// For a more accurate async check, use [`is_visible()`](Element::is_visible).
    pub fn is_displayed(&self) -> bool {
        let html = self.html.to_lowercase();
        if html.contains("display:none")
            || html.contains("display: none")
            || html.contains("hidden")
        {
            return false;
        }
        // Additional checks for elements with CDP backing
        if self.object_id.is_some()
            && (html.contains("visibility:hidden") || html.contains("visibility: hidden"))
        {
            return false;
        }
        !html.is_empty()
    }

    /// True if not disabled.
    pub fn is_enabled(&self) -> bool {
        !self.attrs.iter().any(|(k, _)| k == "disabled")
    }

    /// Check if element is actually visible (async, uses getBoundingClientRect).
    ///
    /// Uses JavaScript `offsetWidth`, `offsetHeight`, and `getClientRects()` to
    /// determine real visibility. Falls back to [`is_displayed()`](Element::is_displayed)
    /// for non-CDP elements.
    pub async fn is_visible(&self) -> bool {
        let page = match self.page.as_ref() {
            Some(p) => p,
            None => return self.is_displayed(),
        };
        if let Some(ref oid) = self.object_id {
            if !oid.is_empty() {
                use chromiumoxide::cdp::js_protocol::runtime::CallFunctionOnParams;
                // Robust visibility: checks display, visibility, opacity, hidden attr,
                // AND geometry (offsetWidth/Height, getClientRects).
                // In headless mode geometry may be 0, so CSS-based checks are primary.
                let fn_decl = "function(){\
                    if(!this||!this.isConnected) return false;\
                    var s=getComputedStyle(this);\
                    if(s.display==='none') return false;\
                    if(s.visibility==='collapse'||s.visibility==='hidden') return false;\
                    if(parseFloat(s.opacity)<=0) return false;\
                    if(this.hasAttribute('hidden')) return false;\
                    if(this.offsetWidth||this.offsetHeight||this.getClientRects().length) return true;\
                    return s.display!=='none';\
                }";
                let params = match CallFunctionOnParams::builder()
                    .object_id(oid.clone())
                    .function_declaration(fn_decl)
                    .return_by_value(true)
                    .build()
                {
                    Ok(p) => p,
                    Err(_) => return self.is_displayed(),
                };
                match page.execute(params).await {
                    Ok(result) => result
                        .result
                        .result
                        .value
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    Err(_) => self.is_displayed(),
                }
            } else {
                self.is_displayed()
            }
        } else {
            self.is_displayed()
        }
    }

    /// The locator used to find this element.
    pub fn locator(&self) -> Option<&Locator> {
        self.locator.as_ref()
    }

    // ── Internal: get CDP element by re-resolving ────────────

    /// Re-resolve this element in the live page via its locator.
    async fn cdp_element(&self) -> Result<chromiumoxide::Element> {
        let page = self
            .page
            .as_ref()
            .ok_or(Error::Browser("requires Chromium mode".into()))?;
        let locator = self
            .locator
            .as_ref()
            .ok_or(Error::Browser("no locator for element".into()))?;
        let selector = locator_to_selector(locator)?;
        page.find_element(&selector)
            .await
            .map_err(|e| Error::Browser(format!("re-resolve element: {e}")))
    }

    // ── Async interactions (CDP only) ────────────────────────

    /// Click this element. Falls back to JS click if CDP click fails.
    pub async fn click(&self) -> Result<()> {
        let cdp_el = self.cdp_element().await?;
        // Try scroll + CDP click first
        if cdp_el.scroll_into_view().await.is_ok() && cdp_el.click().await.is_ok() {
            return Ok(());
        }
        // Fallback: JS click — works even when element is "not visible" to CDP
        self.js("this.click()").await?;
        Ok(())
    }

    /// Type text into this element using CDP Input.insertText.
    /// Supports Chinese and all Unicode characters.
    /// Appends to existing value.
    pub async fn input(&self, text: &str) -> Result<()> {
        let cdp_el = self.cdp_element().await?;
        cdp_el
            .scroll_into_view()
            .await
            .map_err(|e| Error::Browser(format!("scroll: {e}")))?;
        cdp_el
            .click()
            .await
            .map_err(|e| Error::Browser(format!("focus: {e}")))?;
        cdp_el
            .type_str(text)
            .await
            .map_err(|e| Error::Browser(format!("type: {e}")))?;
        Ok(())
    }

    /// Clear the value and type new text (清空后输入).
    /// This is the most common operation for form fields.
    /// Supports both `<input>` and `<textarea>` elements.
    pub async fn fill(&self, text: &str) -> Result<()> {
        // Use JS directly — most reliable, works with all characters including Chinese.
        // Use the element's own prototype chain so it works for both input and textarea.
        let escaped = serde_json::to_string(text).unwrap();
        let js = format!(
            "(function() {{ \
               this.focus(); \
               var proto = Object.getPrototypeOf(this); \
               var desc = null; \
               while (proto) {{ \
                 desc = Object.getOwnPropertyDescriptor(proto, 'value'); \
                 if (desc) break; \
                 proto = Object.getPrototypeOf(proto); \
               }} \
               if (desc && desc.set) {{ \
                 desc.set.call(this, {}); \
                 this.dispatchEvent(new Event('input', {{bubbles: true}})); \
                 this.dispatchEvent(new Event('change', {{bubbles: true}})); \
               }} \
             }}).call(this);",
            escaped
        );
        self.js(&js).await?;
        Ok(())
    }

    /// Clear the value of this element.
    pub async fn clear(&self) -> Result<()> {
        let cdp_el = self.cdp_element().await?;
        cdp_el
            .click()
            .await
            .map_err(|e| Error::Browser(format!("focus: {e}")))?;
        // Ctrl+A then Delete
        let page = self
            .page
            .as_ref()
            .ok_or(Error::Browser("requires Chromium mode".into()))?;
        use chromiumoxide::cdp::browser_protocol::input::{
            DispatchKeyEventParams, DispatchKeyEventType,
        };
        let select_all = DispatchKeyEventParams::builder()
            .r#type(DispatchKeyEventType::KeyDown)
            .key("a")
            .code("KeyA")
            .windows_virtual_key_code(0x41)
            .modifiers(2)
            .build()
            .unwrap();
        page.execute(select_all)
            .await
            .map_err(|e| Error::Browser(format!("select all: {e}")))?;
        let del = DispatchKeyEventParams::builder()
            .r#type(DispatchKeyEventType::KeyDown)
            .key("Delete")
            .code("Delete")
            .windows_virtual_key_code(0x2E)
            .build()
            .unwrap();
        page.execute(del)
            .await
            .map_err(|e| Error::Browser(format!("delete: {e}")))?;
        Ok(())
    }

    /// Hover over this element (move mouse to element center).
    pub async fn hover(&self) -> Result<()> {
        let cdp_el = self.cdp_element().await?;
        cdp_el
            .scroll_into_view()
            .await
            .map_err(|e| Error::Browser(format!("scroll: {e}")))?;
        cdp_el
            .hover()
            .await
            .map_err(|e| Error::Browser(format!("hover: {e}")))?;
        Ok(())
    }

    /// Scroll this element into view.
    pub async fn scroll_into_view(&self) -> Result<()> {
        let cdp_el = self.cdp_element().await?;
        cdp_el
            .scroll_into_view()
            .await
            .map_err(|e| Error::Browser(format!("scroll: {e}")))?;
        Ok(())
    }

    /// Press a keyboard key (e.g. "Enter", "Tab", "Escape").
    pub async fn press_key(&self, key: &str) -> Result<()> {
        let page = self
            .page
            .as_ref()
            .ok_or(Error::Browser("requires Chromium mode".into()))?;
        let cdp_el = self.cdp_element().await?;
        // Focus the element first
        cdp_el
            .click()
            .await
            .map_err(|e| Error::Browser(format!("focus: {e}")))?;
        // Press the key
        cdp_el
            .press_key(key)
            .await
            .map_err(|e| Error::Browser(format!("press_key: {e}")))?;
        // Just consume the page reference to avoid unused warning
        let _ = page;
        Ok(())
    }

    /// Get the current value of input/textarea/select elements.
    pub async fn value(&self) -> Result<String> {
        let cdp_el = self.cdp_element().await?;
        cdp_el
            .string_property("value")
            .await
            .map(|v| v.unwrap_or_default())
            .map_err(|e| Error::Browser(format!("value: {e}")))
    }

    /// Get the element's bounding rectangle (x, y, width, height).
    pub async fn rect(&self) -> Result<(f64, f64, f64, f64)> {
        let cdp_el = self.cdp_element().await?;
        let bb = cdp_el
            .bounding_box()
            .await
            .map_err(|e| Error::Browser(format!("bounding_box: {e}")))?;
        Ok((bb.x, bb.y, bb.width, bb.height))
    }

    /// Whether this element is selected (checkbox/radio/option).
    pub async fn is_selected(&self) -> Result<bool> {
        let cdp_el = self.cdp_element().await?;
        Ok(cdp_el
            .string_property("checked")
            .await
            .ok()
            .flatten()
            .is_some()
            || cdp_el
                .string_property("selected")
                .await
                .ok()
                .flatten()
                .is_some())
    }

    /// Get a computed CSS property value.
    pub async fn style(&self, property: &str) -> Result<String> {
        let page = self
            .page
            .as_ref()
            .ok_or(Error::Browser("requires Chromium mode".into()))?;
        if let Some(ref oid) = self.object_id {
            if !oid.is_empty() {
                use chromiumoxide::cdp::js_protocol::runtime::CallFunctionOnParams;
                let function_decl = format!(
                    "function() {{ return getComputedStyle(this).getPropertyValue({}); }}",
                    serde_json::to_string(property).unwrap()
                );
                let params = CallFunctionOnParams::builder()
                    .object_id(oid.clone())
                    .function_declaration(function_decl)
                    .await_promise(false)
                    .return_by_value(true)
                    .build()
                    .map_err(|e| Error::Browser(format!("build: {e}")))?;
                let result = page
                    .execute(params)
                    .await
                    .map_err(|e| Error::Browser(format!("style: {e}")))?;
                if let Some(val) = result.result.result.value {
                    Ok(val.as_str().unwrap_or("").to_string())
                } else {
                    Ok(String::new())
                }
            } else {
                Ok(String::new())
            }
        } else {
            Ok(String::new())
        }
    }

    /// Select an option in a `<select>` element by visible text.
    pub async fn select(&self, text: &str) -> Result<()> {
        let escaped = serde_json::to_string(text).unwrap();
        let js = format!(
            "(function() {{ \
               var opts = this.options; \
               for (var i = 0; i < opts.length; i++) {{ \
                 if (opts[i].text === {}) {{ \
                   this.selectedIndex = i; \
                   opts[i].selected = true; \
                   this.dispatchEvent(new Event('change', {{bubbles: true}})); \
                   return; \
                 }} \
               }} \
             }}).call(this);",
            escaped
        );
        self.js(&js).await
    }

    /// Select an option by its value attribute.
    pub async fn select_by_value(&self, value: &str) -> Result<()> {
        let escaped = serde_json::to_string(value).unwrap();
        let js = format!(
            "(function() {{ \
               var opts = this.options; \
               for (var i = 0; i < opts.length; i++) {{ \
                 if (opts[i].value === {}) {{ \
                   this.selectedIndex = i; \
                   opts[i].selected = true; \
                   this.dispatchEvent(new Event('change', {{bubbles: true}})); \
                   return; \
                 }} \
               }} \
             }}).call(this);",
            escaped
        );
        self.js(&js).await
    }

    /// Upload a file to an `<input type="file">` element.
    ///
    /// Uses CDP `DOM.setFileInputFiles` which reliably bypasses browser
    /// security restrictions (JS DataTransfer approach is blocked by most
    /// browsers for programmatic file assignment).
    pub async fn upload_file(&self, path: &str) -> Result<()> {
        let page = self
            .page
            .as_ref()
            .ok_or(Error::Browser("upload_file requires Chromium mode".into()))?;
        let cdp_el = self.cdp_element().await?;
        use chromiumoxide::cdp::browser_protocol::dom::SetFileInputFilesParams;
        let params = SetFileInputFilesParams::builder()
            .file(path)
            .object_id(cdp_el.remote_object_id.clone())
            .build()
            .map_err(|e| Error::Browser(format!("build setFileInputFiles: {e}")))?;
        page.execute(params)
            .await
            .map_err(|e| Error::Browser(format!("setFileInputFiles: {e}")))?;
        Ok(())
    }

    /// Right-click this element (fires `contextmenu` event).
    pub async fn right_click(&self) -> Result<()> {
        self.js("this.dispatchEvent(new MouseEvent('contextmenu', {bubbles: true}))")
            .await
    }

    /// Double-click this element (fires `dblclick` event).
    pub async fn double_click(&self) -> Result<()> {
        self.js("this.dispatchEvent(new MouseEvent('dblclick', {bubbles: true}))")
            .await
    }

    /// Take a screenshot of just this element and save to `path` as PNG.
    pub async fn screenshot(&self, path: &str) -> Result<()> {
        let cdp_el = self.cdp_element().await?;
        let _ = cdp_el.scroll_into_view().await;
        let page = self
            .page
            .as_ref()
            .ok_or_else(|| Error::Browser("screenshot requires Chromium mode".into()))?;
        let bbox = cdp_el
            .bounding_box()
            .await
            .map_err(|e| Error::Browser(format!("bbox: {e}")))?;
        use chromiumoxide::cdp::browser_protocol::page::Viewport;
        use chromiumoxide::page::ScreenshotParams;
        let clip = Viewport {
            x: bbox.x,
            y: bbox.y,
            width: bbox.width,
            height: bbox.height,
            scale: 1.0,
        };
        let params = ScreenshotParams::builder().clip(clip).build();
        let bytes = page
            .screenshot(params)
            .await
            .map_err(|e| Error::Browser(format!("screenshot: {e}")))?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    /// Submit the form this element belongs to.
    pub async fn submit(&self) -> Result<()> {
        self.js("this.form ? this.form.submit() : this.submit()")
            .await
    }

    /// Drag this element to a target element.
    pub async fn drag_to(&self, target: &Element) -> Result<()> {
        let src = self.cdp_element().await?;
        let tgt = target.cdp_element().await?;
        let _ = src.scroll_into_view().await;

        let src_bbox = src
            .bounding_box()
            .await
            .map_err(|e| Error::Browser(format!("src bbox: {e}")))?;
        let tgt_bbox = tgt
            .bounding_box()
            .await
            .map_err(|e| Error::Browser(format!("tgt bbox: {e}")))?;

        let src_x = src_bbox.x + src_bbox.width / 2.0;
        let src_y = src_bbox.y + src_bbox.height / 2.0;
        let tgt_x = tgt_bbox.x + tgt_bbox.width / 2.0;
        let tgt_y = tgt_bbox.y + tgt_bbox.height / 2.0;

        let page = self
            .page
            .as_ref()
            .ok_or_else(|| Error::Browser("drag_to requires Chromium mode".into()))?;

        use chromiumoxide::cdp::browser_protocol::input::{
            DispatchMouseEventParams, DispatchMouseEventType, MouseButton,
        };

        // Move to source
        let move_to_src = DispatchMouseEventParams::builder()
            .r#type(DispatchMouseEventType::MouseMoved)
            .x(src_x)
            .y(src_y)
            .build()
            .map_err(|e| Error::Browser(format!("build: {e}")))?;
        page.execute(move_to_src).await.ok();

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Press at source
        let press = DispatchMouseEventParams::builder()
            .r#type(DispatchMouseEventType::MousePressed)
            .x(src_x)
            .y(src_y)
            .button(MouseButton::Left)
            .click_count(1)
            .build()
            .map_err(|e| Error::Browser(format!("build: {e}")))?;
        page.execute(press).await.ok();

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Move to target
        let move_to_tgt = DispatchMouseEventParams::builder()
            .r#type(DispatchMouseEventType::MouseMoved)
            .x(tgt_x)
            .y(tgt_y)
            .build()
            .map_err(|e| Error::Browser(format!("build: {e}")))?;
        page.execute(move_to_tgt).await.ok();

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Release at target
        let release = DispatchMouseEventParams::builder()
            .r#type(DispatchMouseEventType::MouseReleased)
            .x(tgt_x)
            .y(tgt_y)
            .button(MouseButton::Left)
            .click_count(1)
            .build()
            .map_err(|e| Error::Browser(format!("build: {e}")))?;
        page.execute(release).await.ok();

        Ok(())
    }

    /// Drag this element by an offset (relative movement).
    pub async fn drag_to_offset(&self, offset_x: f64, offset_y: f64) -> Result<()> {
        let src = self.cdp_element().await?;
        let _ = src.scroll_into_view().await;

        let src_bbox = src
            .bounding_box()
            .await
            .map_err(|e| Error::Browser(format!("src bbox: {e}")))?;

        let src_x = src_bbox.x + src_bbox.width / 2.0;
        let src_y = src_bbox.y + src_bbox.height / 2.0;
        let tgt_x = src_x + offset_x;
        let tgt_y = src_y + offset_y;

        let page = self
            .page
            .as_ref()
            .ok_or_else(|| Error::Browser("drag_to_offset requires Chromium mode".into()))?;

        use chromiumoxide::cdp::browser_protocol::input::{
            DispatchMouseEventParams, DispatchMouseEventType, MouseButton,
        };

        // Move to source
        let move_to_src = DispatchMouseEventParams::builder()
            .r#type(DispatchMouseEventType::MouseMoved)
            .x(src_x)
            .y(src_y)
            .build()
            .map_err(|e| Error::Browser(format!("build: {e}")))?;
        page.execute(move_to_src).await.ok();

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Press at source
        let press = DispatchMouseEventParams::builder()
            .r#type(DispatchMouseEventType::MousePressed)
            .x(src_x)
            .y(src_y)
            .button(MouseButton::Left)
            .click_count(1)
            .build()
            .map_err(|e| Error::Browser(format!("build: {e}")))?;
        page.execute(press).await.ok();

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Move to target
        let move_to_tgt = DispatchMouseEventParams::builder()
            .r#type(DispatchMouseEventType::MouseMoved)
            .x(tgt_x)
            .y(tgt_y)
            .build()
            .map_err(|e| Error::Browser(format!("build: {e}")))?;
        page.execute(move_to_tgt).await.ok();

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Release at target
        let release = DispatchMouseEventParams::builder()
            .r#type(DispatchMouseEventType::MouseReleased)
            .x(tgt_x)
            .y(tgt_y)
            .button(MouseButton::Left)
            .click_count(1)
            .build()
            .map_err(|e| Error::Browser(format!("build: {e}")))?;
        page.execute(release).await.ok();

        Ok(())
    }

    /// Double-click this element via CDP mouse events (more realistic than JS event).
    pub async fn double_click_cdp(&self) -> Result<()> {
        let cdp_el = self.cdp_element().await?;
        cdp_el
            .scroll_into_view()
            .await
            .map_err(|e| Error::Browser(format!("scroll: {e}")))?;

        let bbox = cdp_el
            .bounding_box()
            .await
            .map_err(|e| Error::Browser(format!("bbox: {e}")))?;

        let page = self
            .page
            .as_ref()
            .ok_or_else(|| Error::Browser("double_click_cdp requires Chromium mode".into()))?;

        use chromiumoxide::cdp::browser_protocol::input::{
            DispatchMouseEventParams, DispatchMouseEventType, MouseButton,
        };

        let x = bbox.x + bbox.width / 2.0;
        let y = bbox.y + bbox.height / 2.0;

        let press = DispatchMouseEventParams::builder()
            .r#type(DispatchMouseEventType::MousePressed)
            .x(x)
            .y(y)
            .button(MouseButton::Left)
            .click_count(2)
            .build()
            .map_err(|e| Error::Browser(format!("build: {e}")))?;
        page.execute(press).await.ok();

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let release = DispatchMouseEventParams::builder()
            .r#type(DispatchMouseEventType::MouseReleased)
            .x(x)
            .y(y)
            .button(MouseButton::Left)
            .click_count(2)
            .build()
            .map_err(|e| Error::Browser(format!("build: {e}")))?;
        page.execute(release).await.ok();

        Ok(())
    }

    /// Right-click (context-click) this element via CDP mouse events.
    pub async fn right_click_cdp(&self) -> Result<()> {
        let cdp_el = self.cdp_element().await?;
        cdp_el
            .scroll_into_view()
            .await
            .map_err(|e| Error::Browser(format!("scroll: {e}")))?;

        let bbox = cdp_el
            .bounding_box()
            .await
            .map_err(|e| Error::Browser(format!("bbox: {e}")))?;

        let page = self
            .page
            .as_ref()
            .ok_or_else(|| Error::Browser("right_click_cdp requires Chromium mode".into()))?;

        use chromiumoxide::cdp::browser_protocol::input::{
            DispatchMouseEventParams, DispatchMouseEventType, MouseButton,
        };

        let x = bbox.x + bbox.width / 2.0;
        let y = bbox.y + bbox.height / 2.0;

        let press = DispatchMouseEventParams::builder()
            .r#type(DispatchMouseEventType::MousePressed)
            .x(x)
            .y(y)
            .button(MouseButton::Right)
            .click_count(1)
            .build()
            .map_err(|e| Error::Browser(format!("build: {e}")))?;
        page.execute(press).await.ok();

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let release = DispatchMouseEventParams::builder()
            .r#type(DispatchMouseEventType::MouseReleased)
            .x(x)
            .y(y)
            .button(MouseButton::Right)
            .click_count(1)
            .build()
            .map_err(|e| Error::Browser(format!("build: {e}")))?;
        page.execute(release).await.ok();

        Ok(())
    }

    /// Check this element (for checkboxes and radio buttons).
    pub async fn check(&self) -> Result<()> {
        self.js("if (!this.checked) { this.click(); }").await
    }

    /// Uncheck this element (for checkboxes).
    pub async fn uncheck(&self) -> Result<()> {
        self.js("if (this.checked) { this.click(); }").await
    }

    /// Select an `<option>` element or select by value/text in a `<select>`.
    pub async fn select_option(&self, value: &str) -> Result<()> {
        let escaped = serde_json::to_string(value).unwrap();
        let js = format!(
            "(function() {{ \
               if (this.tagName === 'OPTION') {{ \
                 this.selected = true; \
                 this.parentElement.dispatchEvent(new Event('change', {{bubbles: true}})); \
               }} else if (this.tagName === 'SELECT') {{ \
                 for (var i = 0; i < this.options.length; i++) {{ \
                   var opt = this.options[i]; \
                   if (opt.value === {} || opt.text === {}) {{ \
                     opt.selected = true; \
                     this.dispatchEvent(new Event('change', {{bubbles: true}})); \
                     return; \
                   }} \
                 }} \
               }} \
             }}).call(this);",
            escaped, escaped
        );
        self.js(&js).await
    }

    /// Focus this element.
    pub async fn focus(&self) -> Result<()> {
        self.js("this.focus()").await
    }

    /// Blur (unfocus) this element.
    pub async fn blur(&self) -> Result<()> {
        self.js("this.blur()").await
    }

    /// Get the bounding box (x, y, width, height) of this element.
    pub async fn bounding_box(&self) -> Result<(f64, f64, f64, f64)> {
        let cdp_el = self.cdp_element().await?;
        match cdp_el.bounding_box().await {
            Ok(bbox) => Ok((bbox.x, bbox.y, bbox.width, bbox.height)),
            Err(_) => {
                // Fallback: use getBoundingClientRect via CDP CallFunctionOn
                let page = self.page.as_ref()
                    .ok_or_else(|| Error::Browser("bounding_box: no page ref".into()))?;
                let oid = self.object_id.clone()
                    .ok_or_else(|| Error::Browser("bounding_box: no object_id".into()))?;
                use chromiumoxide::cdp::js_protocol::runtime::CallFunctionOnParams;
                let params = CallFunctionOnParams::builder()
                    .object_id(oid)
                    .function_declaration(
                        "function(){var r=this.getBoundingClientRect();return [r.x,r.y,r.width,r.height]}"
                    )
                    .return_by_value(true)
                    .build()
                    .map_err(|e| Error::Browser(format!("bounding_box: {e}")))?;
                let result = page.execute(params).await
                    .map_err(|e| Error::Browser(format!("bounding_box js: {e}")))?;
                let arr = result.result.result.value
                    .ok_or_else(|| Error::Browser("bounding_box: no value".into()))?;
                let vals: Vec<f64> = arr.as_array()
                    .ok_or_else(|| Error::Browser("bounding_box: not array".into()))?
                    .iter().filter_map(|v| v.as_f64()).collect();
                if vals.len() >= 4 {
                    Ok((vals[0], vals[1], vals[2], vals[3]))
                } else {
                    Err(Error::Browser("bounding_box: insufficient values".into()))
                }
            }
        }
    }

    /// Select all text in this element (focus + select).
    pub async fn select_text(&self) -> Result<()> {
        self.js("this.focus(); this.select()").await
    }

    /// Upload multiple files to an `<input type="file">` element.
    pub async fn upload_files(&self, file_paths: &[&str]) -> Result<()> {
        let page = self
            .page
            .as_ref()
            .ok_or_else(|| Error::Browser("upload_files requires Chromium mode".into()))?;
        let cdp_el = self.cdp_element().await?;
        use chromiumoxide::cdp::browser_protocol::dom::SetFileInputFilesParams;
        let files: Vec<String> = file_paths.iter().map(|s| s.to_string()).collect();
        let params = SetFileInputFilesParams::builder()
            .files(files)
            .object_id(cdp_el.remote_object_id.clone())
            .build()
            .map_err(|e| Error::Browser(format!("build setFileInputFiles: {e}")))?;
        page.execute(params)
            .await
            .map_err(|e| Error::Browser(format!("setFileInputFiles: {e}")))?;
        Ok(())
    }

    /// Take a screenshot of just this element and return raw PNG bytes.
    pub async fn screenshot_bytes(&self) -> Result<Vec<u8>> {
        let cdp_el = self.cdp_element().await?;
        cdp_el
            .scroll_into_view()
            .await
            .map_err(|e| Error::Browser(format!("scroll: {e}")))?;

        let page = self
            .page
            .as_ref()
            .ok_or_else(|| Error::Browser("screenshot_bytes requires Chromium mode".into()))?;

        let bbox = cdp_el
            .bounding_box()
            .await
            .map_err(|e| Error::Browser(format!("bbox: {e}")))?;

        use chromiumoxide::cdp::browser_protocol::page::Viewport;
        use chromiumoxide::page::ScreenshotParams;

        let clip = Viewport {
            x: bbox.x,
            y: bbox.y,
            width: bbox.width,
            height: bbox.height,
            scale: 1.0,
        };
        let params = ScreenshotParams::builder().clip(clip).build();
        let bytes = page
            .screenshot(params)
            .await
            .map_err(|e| Error::Browser(format!("screenshot_bytes: {e}")))?;
        Ok(bytes)
    }

    /// Scroll this element to bring it into view at the top of its scrollable container.
    pub async fn scroll_to_top(&self) -> Result<()> {
        self.js("this.scrollIntoView({behavior: 'instant', block: 'start'})")
            .await
    }

    /// Set an attribute on this element.
    pub async fn set_attr(&self, name: &str, value: &str) -> Result<()> {
        let escaped_name = serde_json::to_string(name).unwrap();
        let escaped_value = serde_json::to_string(value).unwrap();
        let js = format!("this.setAttribute({}, {})", escaped_name, escaped_value);
        self.js(&js).await
    }

    /// Get the parent element.
    pub async fn parent(&self) -> Result<Element> {
        self.js_element("this.parentElement").await
    }

    /// Get the first child element.
    pub async fn first_child(&self) -> Result<Element> {
        self.js_element("this.firstElementChild").await
    }

    /// Get the next sibling element.
    pub async fn next(&self) -> Result<Element> {
        self.js_element("this.nextElementSibling").await
    }

    /// Get the previous sibling element.
    pub async fn prev(&self) -> Result<Element> {
        self.js_element("this.previousElementSibling").await
    }

    /// Helper: evaluate a JS expression that returns an element node,
    /// parse its outerHTML, and return a new `Element`.
    async fn js_element(&self, js_expr: &str) -> Result<Element> {
        let page = self
            .page
            .as_ref()
            .ok_or(Error::Browser("requires Chromium mode".into()))?;
        if let Some(ref oid) = self.object_id {
            if !oid.is_empty() {
                use chromiumoxide::cdp::js_protocol::runtime::CallFunctionOnParams;
                let fn_decl = format!(
                    "function() {{ var el = {}; return el ? el.outerHTML : null; }}",
                    js_expr
                );
                let params = CallFunctionOnParams::builder()
                    .object_id(oid.clone())
                    .function_declaration(fn_decl)
                    .return_by_value(true)
                    .build()
                    .map_err(|e| Error::Browser(format!("build: {e}")))?;
                let result = page
                    .execute(params)
                    .await
                    .map_err(|e| Error::Browser(format!("js_element: {e}")))?;
                let html_str = result
                    .result
                    .result
                    .value
                    .and_then(|v| v.as_str().map(String::from))
                    .ok_or_else(|| Error::ElementNotFound("no element returned".into()))?;
                let doc = scraper::Html::parse_document(&html_str);
                let sel = Selector::parse("*").unwrap();
                let el_ref = doc
                    .select(&sel)
                    .next()
                    .ok_or_else(|| Error::ElementNotFound("parse element".into()))?;
                Ok(from_scraper_element(&el_ref, None, self.page.clone()))
            } else {
                Err(Error::ElementNotFound("no object_id".into()))
            }
        } else {
            Err(Error::ElementNotFound("no CDP".into()))
        }
    }

    /// Execute JavaScript with `this` bound to this element.
    pub async fn js(&self, script: &str) -> Result<()> {
        let page = self
            .page
            .as_ref()
            .ok_or(Error::Browser("js requires Chromium mode".into()))?;

        // If we have an object_id, use CDP Runtime.callFunctionOn for reliability
        if let Some(ref oid) = self.object_id {
            if !oid.is_empty() {
                use chromiumoxide::cdp::js_protocol::runtime::CallFunctionOnParams;
                let function_decl = format!("function() {{ {script} }}");
                let params = CallFunctionOnParams::builder()
                    .object_id(oid.clone())
                    .function_declaration(function_decl)
                    .await_promise(true)
                    .build()
                    .map_err(|e| Error::Browser(format!("build callFunctionOn: {e}")))?;
                page.execute(params)
                    .await
                    .map_err(|e| Error::Browser(format!("element js: {e}")))?;
                return Ok(());
            }
        }

        // Fallback: use locator-based query
        let locator = self
            .locator
            .as_ref()
            .ok_or(Error::Browser("no locator for element".into()))?;

        let wrapped = if locator.is_css() {
            let selector = locator_to_selector(locator)?;
            format!(
                "(function(){{ var el = document.querySelector({}); if(!el) return; (function(){{ {} }}).call(el); }})()",
                serde_json::to_string(&selector).unwrap(),
                script,
            )
        } else {
            // XPath-based locator: use document.evaluate()
            let xpath = locator
                .to_xpath()
                .ok_or_else(|| Error::InvalidLocator("cannot convert to XPath".into()))?;
            format!(
                "(function(){{ var result = document.evaluate({}, document, null, XPathResult.FIRST_ORDERED_NODE_TYPE, null); var el = result.singleNodeValue; if(!el) return; (function(){{ {} }}).call(el); }})()",
                serde_json::to_string(&xpath).unwrap(),
                script,
            )
        };
        page.evaluate(wrapped.as_str())
            .await
            .map_err(|e| Error::Browser(format!("element js: {e}")))?;
        Ok(())
    }

    // ── Sub-element queries (cached HTML) ────────────────────

    /// Find a child element by locator (operates on cached HTML).
    pub fn ele(&self, locator_str: &str) -> Result<Element> {
        let locator = parse_locator(locator_str)?;
        let doc = scraper::Html::parse_document(&self.html);

        match &locator {
            Locator::Css(sel) => {
                let selector =
                    Selector::parse(sel).map_err(|e| Error::InvalidLocator(e.to_string()))?;
                doc.select(&selector)
                    .next()
                    .map(|el| from_scraper_element(&el, Some(locator), self.page.clone()))
                    .ok_or_else(|| Error::ElementNotFound(format!("sub-element: {locator_str}")))
            }
            _ => {
                let xpath = locator
                    .to_xpath()
                    .ok_or_else(|| Error::InvalidLocator("cannot convert to XPath".into()))?;
                let package = sxd_document::parser::parse(&self.html)
                    .map_err(|e| Error::InvalidLocator(e.to_string()))?;
                let document = package.as_document();
                let value = sxd_xpath::evaluate_xpath(&document, &xpath)
                    .map_err(|e| Error::InvalidLocator(format!("XPath: {e}")))?;
                let nodes = match value {
                    sxd_xpath::Value::Nodeset(ns) => ns,
                    _ => return Err(Error::ElementNotFound("XPath: non-nodeset".into())),
                };
                let node = nodes
                    .iter()
                    .next()
                    .ok_or_else(|| Error::ElementNotFound(format!("sub-element: {locator_str}")))?;
                Ok(Element::new_session(
                    Some(locator),
                    node.string_value(),
                    String::new(),
                    String::new(),
                    Vec::new(),
                ))
            }
        }
    }

    /// Find all child elements matching the locator.
    pub fn eles(&self, locator_str: &str) -> Result<Vec<Element>> {
        let locator = parse_locator(locator_str)?;
        let doc = scraper::Html::parse_document(&self.html);

        match &locator {
            Locator::Css(sel) => {
                let selector =
                    Selector::parse(sel).map_err(|e| Error::InvalidLocator(e.to_string()))?;
                Ok(doc
                    .select(&selector)
                    .map(|el| from_scraper_element(&el, Some(locator.clone()), self.page.clone()))
                    .collect())
            }
            _ => {
                let xpath = locator
                    .to_xpath()
                    .ok_or_else(|| Error::InvalidLocator("cannot convert to XPath".into()))?;
                let package = sxd_document::parser::parse(&self.html)
                    .map_err(|e| Error::InvalidLocator(e.to_string()))?;
                let document = package.as_document();
                let value = sxd_xpath::evaluate_xpath(&document, &xpath)
                    .map_err(|e| Error::InvalidLocator(format!("XPath: {e}")))?;
                let nodes = match value {
                    sxd_xpath::Value::Nodeset(ns) => ns,
                    _ => return Ok(Vec::new()),
                };
                Ok(nodes
                    .iter()
                    .map(|node| {
                        Element::new_session(
                            Some(locator.clone()),
                            node.string_value(),
                            String::new(),
                            String::new(),
                            Vec::new(),
                        )
                    })
                    .collect())
            }
        }
    }

    // ── Shadow DOM piercing (CDP only) ────────────────────────

    /// Find an element inside this element's Shadow DOM.
    ///
    /// Usage: `element.shadow_ele(".inner")` — penetrates this element's
    /// shadowRoot and runs `querySelector(".inner")`.
    ///
    /// For multi-level piercing use `>>>` separator:
    /// `element.shadow_ele(".mid >>> .inner")`
    pub async fn shadow_ele(&self, selector: &str) -> Result<Element> {
        let page = self
            .page
            .as_ref()
            .ok_or(Error::Browser("shadow_ele requires Chromium mode".into()))?;
        let oid = self
            .object_id
            .as_deref()
            .ok_or(Error::Browser("no object_id for element".into()))?;

        let parts: Vec<&str> = selector.split(">>>").map(|s| s.trim()).collect();
        if parts.is_empty() {
            return Err(Error::InvalidLocator("empty selector".into()));
        }

        // Build JS: start from this.shadowRoot, then drill down
        let first_sel = serde_json::to_string(parts[0]).unwrap();
        let js = if parts.len() == 1 {
            format!(
                "function() {{ \
                   if (!this.shadowRoot) return null; \
                   return this.shadowRoot.querySelector({sel}); \
                 }}",
                sel = first_sel
            )
        } else {
            let mut body = String::from(
                "if (!this.shadowRoot) return null; \
                 var cur = this.shadowRoot;",
            );
            let inner_sels: Vec<String> = parts
                .iter()
                .map(|s| serde_json::to_string(s).unwrap())
                .collect();
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
            format!("function() {{ {body} }}")
        };

        // Simpler approach: combine into one function
        let full_fn = {
            let inner_fn = js
                .trim_start_matches("function() {")
                .trim_end_matches('}')
                .trim();
            format!(
                "function() {{ \
                   (function() {{ {inner_fn} }}).call(this); \
                   var el = (function() {{ {inner_fn} }}).call(this); \
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
                 }}"
            )
        };

        use chromiumoxide::cdp::js_protocol::runtime::CallFunctionOnParams;
        let params = CallFunctionOnParams::builder()
            .object_id(oid.to_string())
            .function_declaration(full_fn)
            .return_by_value(true)
            .build()
            .map_err(|e| Error::Browser(format!("build: {e}")))?;

        let result = page
            .execute(params)
            .await
            .map_err(|e| Error::Browser(format!("shadow_ele: {e}")))?;

        let json_str = result
            .result
            .result
            .value
            .and_then(|v| v.as_str().map(String::from))
            .ok_or_else(|| Error::ElementNotFound("shadow element not found".into()))?;

        let data: serde_json::Value = serde_json::from_str(&json_str)
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

        Ok(Element::new_cdp(
            page.clone(),
            String::new(),
            Some(Locator::Css(selector.to_string())),
            html,
            tag,
            text,
            attrs,
        ))
    }

    /// Find all elements inside this element's Shadow DOM.
    ///
    /// Usage: `element.shadow_eles(".inner")`
    pub async fn shadow_eles(&self, selector: &str) -> Result<Vec<Element>> {
        let page = self
            .page
            .as_ref()
            .ok_or(Error::Browser("shadow_eles requires Chromium mode".into()))?;
        let oid = self
            .object_id
            .as_deref()
            .ok_or(Error::Browser("no object_id for element".into()))?;

        let parts: Vec<&str> = selector.split(">>>").map(|s| s.trim()).collect();
        if parts.is_empty() {
            return Err(Error::InvalidLocator("empty selector".into()));
        }

        // Build JS for querySelectorAll
        let inner_sels: Vec<String> = parts
            .iter()
            .map(|s| serde_json::to_string(s).unwrap())
            .collect();

        let query_body = if inner_sels.len() == 1 {
            format!(
                "if (!this.shadowRoot) return []; \
                 return this.shadowRoot.querySelectorAll({sel});",
                sel = &inner_sels[0]
            )
        } else {
            let mut body =
                String::from("if (!this.shadowRoot) return []; var cur = this.shadowRoot;");
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
            body
        };

        let full_fn = format!(
            "function() {{ \
               var els = (function() {{ {query_body} }}).call(this); \
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
             }}"
        );

        use chromiumoxide::cdp::js_protocol::runtime::CallFunctionOnParams;
        let params = CallFunctionOnParams::builder()
            .object_id(oid.to_string())
            .function_declaration(full_fn)
            .return_by_value(true)
            .build()
            .map_err(|e| Error::Browser(format!("build: {e}")))?;

        let result = page
            .execute(params)
            .await
            .map_err(|e| Error::Browser(format!("shadow_eles: {e}")))?;

        let json_str = result
            .result
            .result
            .value
            .and_then(|v| v.as_str().map(String::from))
            .unwrap_or_else(|| "[]".to_string());

        let items: Vec<serde_json::Value> = serde_json::from_str(&json_str)
            .map_err(|e| Error::Browser(format!("parse shadow results: {e}")))?;

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
                page.clone(),
                String::new(),
                Some(Locator::Css(selector.to_string())),
                html,
                tag,
                text,
                attrs,
            ));
        }

        Ok(elements)
    }

    // ── Wait methods (CDP only) ──────────────────────────────

    /// Internal helper: re-query the element from the live page via CDP and
    /// return a fresh `Element` with updated state.
    async fn requery(&self) -> Result<Element> {
        let page = self
            .page
            .as_ref()
            .ok_or(Error::Browser("wait requires Chromium mode".into()))?;
        let locator = self
            .locator
            .as_ref()
            .ok_or(Error::Browser("no locator for element".into()))?;
        let selector = locator_to_selector(locator)?;

        let cdp_el = page
            .find_element(&selector)
            .await
            .map_err(|e| Error::Browser(format!("re-query: {e}")))?;

        let html = cdp_el.outer_html().await.ok().flatten().unwrap_or_default();
        let text = cdp_el.inner_text().await.ok().flatten().unwrap_or_default();
        let tag = cdp_el
            .string_property("tagName")
            .await
            .ok()
            .flatten()
            .unwrap_or_default()
            .to_lowercase();
        let attrs: Vec<(String, String)> = cdp_el
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
        let object_id: String = cdp_el.remote_object_id.clone().into();

        Ok(Element::new_cdp(
            page.clone(),
            object_id,
            Some(locator.clone()),
            html,
            tag,
            text,
            attrs,
        ))
    }

    /// Wait until this element is visible on the page.
    ///
    /// Polls by re-querying the element and checking visibility via JavaScript.
    /// Uses default timeout (10s) and poll interval (200ms).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Timeout`] if the element is not visible within the timeout.
    /// Returns [`Error::Browser`] if not in CDP mode or no locator is available.
    ///
    /// # Example
    ///
    /// ```ignore
    /// element.wait_for_visible().await?;
    /// element.wait_for_visible_with_timeout(Duration::from_secs(30)).await?;
    /// ```
    pub async fn wait_for_visible(&self) -> Result<()> {
        self.wait_for_visible_with_options(WaitOptions::default()).await
    }

    /// Wait until this element is visible, with a custom timeout.
    pub async fn wait_for_visible_with_timeout(&self, timeout: Duration) -> Result<()> {
        self.wait_for_visible_with_options(WaitOptions::default().timeout(timeout))
            .await
    }

    /// Wait until this element is visible, with full [`WaitOptions`] control.
    pub async fn wait_for_visible_with_options(&self, opts: WaitOptions) -> Result<()> {
        let start = std::time::Instant::now();
        loop {
            match self.requery().await {
                Ok(el) => {
                    if el.is_visible().await {
                        return Ok(());
                    }
                }
                Err(_) => {
                    // Element not found yet — keep waiting
                }
            }
            if start.elapsed() >= opts.timeout {
                return Err(Error::Timeout(format!(
                    "Timed out waiting for element to be visible ({:?})",
                    opts.timeout
                )));
            }
            tokio::time::sleep(opts.poll_interval).await;
        }
    }

    /// Wait until this element is hidden or removed from the DOM.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Timeout`] if the element is still visible within the timeout.
    pub async fn wait_for_hidden(&self) -> Result<()> {
        self.wait_for_hidden_with_options(WaitOptions::default()).await
    }

    /// Wait until this element is hidden, with a custom timeout.
    pub async fn wait_for_hidden_with_timeout(&self, timeout: Duration) -> Result<()> {
        self.wait_for_hidden_with_options(WaitOptions::default().timeout(timeout))
            .await
    }

    /// Wait until this element is hidden, with full [`WaitOptions`] control.
    pub async fn wait_for_hidden_with_options(&self, opts: WaitOptions) -> Result<()> {
        let start = std::time::Instant::now();
        loop {
            match self.requery().await {
                Ok(el) => {
                    if !el.is_visible().await {
                        return Ok(());
                    }
                }
                Err(_) => {
                    // Element not found in DOM → considered hidden
                    return Ok(());
                }
            }
            if start.elapsed() >= opts.timeout {
                return Err(Error::Timeout(format!(
                    "Timed out waiting for element to be hidden ({:?})",
                    opts.timeout
                )));
            }
            tokio::time::sleep(opts.poll_interval).await;
        }
    }

    /// Wait until this element is enabled (not disabled).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Timeout`] if the element remains disabled within the timeout.
    pub async fn wait_for_enabled(&self) -> Result<()> {
        self.wait_for_enabled_with_options(WaitOptions::default()).await
    }

    /// Wait until this element is enabled, with a custom timeout.
    pub async fn wait_for_enabled_with_timeout(&self, timeout: Duration) -> Result<()> {
        self.wait_for_enabled_with_options(WaitOptions::default().timeout(timeout))
            .await
    }

    /// Wait until this element is enabled, with full [`WaitOptions`] control.
    pub async fn wait_for_enabled_with_options(&self, opts: WaitOptions) -> Result<()> {
        let start = std::time::Instant::now();
        loop {
            match self.requery().await {
                Ok(el) => {
                    if el.is_enabled() {
                        return Ok(());
                    }
                }
                Err(_) => {
                    // Element not found — keep waiting
                }
            }
            if start.elapsed() >= opts.timeout {
                return Err(Error::Timeout(format!(
                    "Timed out waiting for element to be enabled ({:?})",
                    opts.timeout
                )));
            }
            tokio::time::sleep(opts.poll_interval).await;
        }
    }

    /// Wait until this element's text content contains the given substring.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Timeout`] if the text does not appear within the timeout.
    pub async fn wait_for_text(&self, text: &str) -> Result<()> {
        self.wait_for_text_with_options(text, WaitOptions::default())
            .await
    }

    /// Wait until this element's text contains the given substring, with a custom timeout.
    pub async fn wait_for_text_with_timeout(&self, text: &str, timeout: Duration) -> Result<()> {
        self.wait_for_text_with_options(text, WaitOptions::default().timeout(timeout))
            .await
    }

    /// Wait until this element's text contains the given substring, with full [`WaitOptions`].
    pub async fn wait_for_text_with_options(
        &self,
        text: &str,
        opts: WaitOptions,
    ) -> Result<()> {
        let start = std::time::Instant::now();
        loop {
            match self.requery().await {
                Ok(el) => {
                    if el.text().contains(text) {
                        return Ok(());
                    }
                }
                Err(_) => {
                    // Element not found — keep waiting
                }
            }
            if start.elapsed() >= opts.timeout {
                return Err(Error::Timeout(format!(
                    "Timed out waiting for element text to contain {:?} ({:?})",
                    text, opts.timeout
                )));
            }
            tokio::time::sleep(opts.poll_interval).await;
        }
    }

    /// Wait until this element has an attribute with the exact given value.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Timeout`] if the attribute value does not match within the timeout.
    pub async fn wait_for_attribute(&self, name: &str, value: &str) -> Result<()> {
        self.wait_for_attribute_with_options(name, value, WaitOptions::default())
            .await
    }

    /// Wait until this element has an attribute with the exact given value, with a custom timeout.
    pub async fn wait_for_attribute_with_timeout(
        &self,
        name: &str,
        value: &str,
        timeout: Duration,
    ) -> Result<()> {
        self.wait_for_attribute_with_options(name, value, WaitOptions::default().timeout(timeout))
            .await
    }

    /// Wait until this element has an attribute with the exact given value, with full [`WaitOptions`].
    pub async fn wait_for_attribute_with_options(
        &self,
        name: &str,
        value: &str,
        opts: WaitOptions,
    ) -> Result<()> {
        let start = std::time::Instant::now();
        loop {
            match self.requery().await {
                Ok(el) => {
                    if el.attr(name) == Some(value) {
                        return Ok(());
                    }
                }
                Err(_) => {
                    // Element not found — keep waiting
                }
            }
            if start.elapsed() >= opts.timeout {
                return Err(Error::Timeout(format!(
                    "Timed out waiting for element attribute {:?}={:?} ({:?})",
                    name, value, opts.timeout
                )));
            }
            tokio::time::sleep(opts.poll_interval).await;
        }
    }

    /// Wait until this element is removed from the DOM (stale).
    ///
    /// A stale element is one whose locator no longer resolves in the live page.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Timeout`] if the element is still present within the timeout.
    pub async fn wait_for_stale(&self) -> Result<()> {
        self.wait_for_stale_with_options(WaitOptions::default()).await
    }

    /// Wait until this element is stale, with a custom timeout.
    pub async fn wait_for_stale_with_timeout(&self, timeout: Duration) -> Result<()> {
        self.wait_for_stale_with_options(WaitOptions::default().timeout(timeout))
            .await
    }

    /// Wait until this element is stale, with full [`WaitOptions`] control.
    pub async fn wait_for_stale_with_options(&self, opts: WaitOptions) -> Result<()> {
        let start = std::time::Instant::now();
        loop {
            match self.requery().await {
                Ok(_) => {
                    // Element still exists — keep waiting
                }
                Err(_) => {
                    // Element gone from DOM → stale
                    return Ok(());
                }
            }
            if start.elapsed() >= opts.timeout {
                return Err(Error::Timeout(format!(
                    "Timed out waiting for element to become stale ({:?})",
                    opts.timeout
                )));
            }
            tokio::time::sleep(opts.poll_interval).await;
        }
    }

    /// Wait until this element is both visible and enabled (clickable).
    ///
    /// # Errors
    ///
    /// Returns [`Error::Timeout`] if the element is not clickable within the timeout.
    pub async fn wait_for_clickable(&self) -> Result<()> {
        self.wait_for_clickable_with_options(WaitOptions::default())
            .await
    }

    /// Wait until this element is clickable, with a custom timeout.
    pub async fn wait_for_clickable_with_timeout(&self, timeout: Duration) -> Result<()> {
        self.wait_for_clickable_with_options(WaitOptions::default().timeout(timeout))
            .await
    }

    /// Wait until this element is clickable, with full [`WaitOptions`] control.
    pub async fn wait_for_clickable_with_options(&self, opts: WaitOptions) -> Result<()> {
        let start = std::time::Instant::now();
        loop {
            match self.requery().await {
                Ok(el) => {
                    if el.is_visible().await && el.is_enabled() {
                        return Ok(());
                    }
                }
                Err(_) => {
                    // Element not found — keep waiting
                }
            }
            if start.elapsed() >= opts.timeout {
                return Err(Error::Timeout(format!(
                    "Timed out waiting for element to be clickable ({:?})",
                    opts.timeout
                )));
            }
            tokio::time::sleep(opts.poll_interval).await;
        }
    }

    /// Wait until this element's text content exactly matches the given string.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Timeout`] if the text does not match within the timeout.
    pub async fn wait_for_text_eq(&self, text: &str) -> Result<()> {
        self.wait_for_text_eq_with_options(text, WaitOptions::default())
            .await
    }

    /// Wait until this element's text exactly matches, with a custom timeout.
    pub async fn wait_for_text_eq_with_timeout(&self, text: &str, timeout: Duration) -> Result<()> {
        self.wait_for_text_eq_with_options(text, WaitOptions::default().timeout(timeout))
            .await
    }

    /// Wait until this element's text exactly matches, with full [`WaitOptions`].
    pub async fn wait_for_text_eq_with_options(
        &self,
        text: &str,
        opts: WaitOptions,
    ) -> Result<()> {
        let start = std::time::Instant::now();
        loop {
            match self.requery().await {
                Ok(el) => {
                    if el.text() == text {
                        return Ok(());
                    }
                }
                Err(_) => {
                    // Element not found — keep waiting
                }
            }
            if start.elapsed() >= opts.timeout {
                return Err(Error::Timeout(format!(
                    "Timed out waiting for element text to equal {:?} ({:?})",
                    text, opts.timeout
                )));
            }
            tokio::time::sleep(opts.poll_interval).await;
        }
    }

    /// Wait for an attribute to contain a substring.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Timeout`] if the attribute doesn't contain the value within the timeout.
    pub async fn wait_for_attribute_contains(&self, name: &str, value: &str) -> Result<()> {
        self.wait_for_attribute_contains_with_options(name, value, WaitOptions::default())
            .await
    }

    /// Wait for an attribute to contain a substring, with a custom timeout.
    pub async fn wait_for_attribute_contains_with_timeout(
        &self,
        name: &str,
        value: &str,
        timeout: Duration,
    ) -> Result<()> {
        self.wait_for_attribute_contains_with_options(
            name,
            value,
            WaitOptions::default().timeout(timeout),
        )
        .await
    }

    /// Wait for an attribute to contain a substring, with full [`WaitOptions`].
    pub async fn wait_for_attribute_contains_with_options(
        &self,
        name: &str,
        value: &str,
        opts: WaitOptions,
    ) -> Result<()> {
        let start = std::time::Instant::now();
        loop {
            match self.requery().await {
                Ok(el) => {
                    if el.attr(name).is_some_and(|v| v.contains(value)) {
                        return Ok(());
                    }
                }
                Err(_) => {
                    // Element not found — keep waiting
                }
            }
            if start.elapsed() >= opts.timeout {
                return Err(Error::Timeout(format!(
                    "Timed out waiting for element attribute {:?} to contain {:?} ({:?})",
                    name, value, opts.timeout
                )));
            }
            tokio::time::sleep(opts.poll_interval).await;
        }
    }
}

// ── Helpers ──────────────────────────────────────────────────

/// Build a session-mode Element from a scraper::ElementRef.
pub(crate) fn from_scraper_element(
    el: &scraper::ElementRef,
    locator: Option<Locator>,
    page: Option<Page>,
) -> Element {
    let tag = el.value().name().to_string();
    let attrs: Vec<(String, String)> = el
        .value()
        .attrs()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let text = el.text().collect::<Vec<_>>().join("");
    let html = el.html();

    if let Some(p) = page {
        Element::new_cdp(
            p,
            String::new(), // no object_id for scraper-sourced
            locator,
            html,
            tag,
            text,
            attrs,
        )
    } else {
        Element::new_session(locator, html, tag, text, attrs)
    }
}

// ── Batch operations on Vec<Element> ──────────────────────

/// Extension trait for batch operations on `Vec<Element>`.
pub trait ElementBatch {
    /// Get text content of all elements.
    fn texts(&self) -> Vec<&str>;
    /// Get a specific attribute from all elements.
    fn attr_values(&self, name: &str) -> Vec<Option<&str>>;
    /// Get all matching elements that are displayed.
    fn displayed(&self) -> Vec<&Element>;
}

impl ElementBatch for [Element] {
    fn texts(&self) -> Vec<&str> {
        self.iter().map(|e| e.text()).collect()
    }
    fn attr_values(&self, name: &str) -> Vec<Option<&str>> {
        self.iter().map(|e| e.attr(name)).collect()
    }
    fn displayed(&self) -> Vec<&Element> {
        self.iter().filter(|e| e.is_displayed()).collect()
    }
}
