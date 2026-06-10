//! Element abstraction over CDP and parsed-HTML elements
//!
//! Elements carry an optional reference to the CDP page, enabling
//! async interactions (click, input, etc.) in Chromium mode.

use chromiumoxide::Page;
use scraper::Selector;

use crate::error::{Error, Result};
use crate::locator::{parse_locator, Locator};

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
#[allow(dead_code)]
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

    /// True if element has non-hidden style.
    pub fn is_displayed(&self) -> bool {
        !self.html.contains("display:none")
            && !self.html.contains("display: none")
            && !self.html.contains("hidden")
    }

    /// True if not disabled.
    pub fn is_enabled(&self) -> bool {
        !self.attrs.iter().any(|(k, _)| k == "disabled")
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
        let selector = locator_to_query(locator)?;
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
        let escaped = text.replace('\\', "\\\\").replace('\'', "\\'");
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
                 desc.set.call(this, '{}'); \
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
                    "function() {{ return getComputedStyle(this).getPropertyValue('{}'); }}",
                    property.replace('\'', "\\'")
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
        let escaped = text.replace('\\', "\\\\").replace('\'', "\\'");
        let js = format!(
            "(function() {{ \
               var opts = this.options; \
               for (var i = 0; i < opts.length; i++) {{ \
                 if (opts[i].text === '{}') {{ \
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
        let escaped = value.replace('\\', "\\\\").replace('\'', "\\'");
        let js = format!(
            "(function() {{ \
               var opts = this.options; \
               for (var i = 0; i < opts.length; i++) {{ \
                 if (opts[i].value === '{}') {{ \
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
    pub async fn upload_file(&self, path: &str) -> Result<()> {
        let escaped = path.replace('\\', "\\\\").replace('\'', "\\'");
        let js = format!(
            "(function() {{ \
               var input = this; \
               var dt = new DataTransfer(); \
               dt.items.add(new File([''], '{}')); \
               input.files = dt.files; \
               input.dispatchEvent(new Event('change', {{bubbles: true}})); \
             }}).call(this);",
            escaped
        );
        self.js(&js).await
    }

    /// Submit the form this element belongs to.
    pub async fn submit(&self) -> Result<()> {
        self.js("this.form ? this.form.submit() : this.submit()")
            .await
    }

    /// Set an attribute on this element.
    pub async fn set_attr(&self, name: &str, value: &str) -> Result<()> {
        let escaped_name = name.replace('\\', "\\\\").replace('\'', "\\'");
        let escaped_value = value.replace('\\', "\\\\").replace('\'', "\\'");
        let js = format!("this.setAttribute('{}', '{}')", escaped_name, escaped_value);
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
            let selector = locator_to_query(locator)?;
            format!(
                "(function(){{ var el = document.querySelector('{}'); if(!el) return; (function(){{ {} }}).call(el); }})()",
                selector.replace('\\', "\\\\").replace('\'', "\\'"),
                script,
            )
        } else {
            // XPath-based locator: use document.evaluate()
            let xpath = locator
                .to_xpath()
                .ok_or_else(|| Error::InvalidLocator("cannot convert to XPath".into()))?;
            format!(
                "(function(){{ var result = document.evaluate('{}', document, null, XPathResult.FIRST_ORDERED_NODE_TYPE, null); var el = result.singleNodeValue; if(!el) return; (function(){{ {} }}).call(el); }})()",
                xpath.replace('\\', "\\\\").replace('\'', "\\'"),
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
}

// ── Helpers ──────────────────────────────────────────────────

/// Convert Locator to a CSS query selector for use in page.evaluate.
fn locator_to_query(locator: &Locator) -> Result<String> {
    match locator {
        Locator::Css(sel) => Ok(sel.clone()),
        Locator::XPath(xp) => Ok(format!("xpath:{xp}")),
        Locator::Text(t) => Ok(format!("xpath://*[text()='{}']", t.replace('\'', "\\'"))),
        Locator::TextContains(t) => Ok(format!(
            "xpath://*[contains(text(),'{}')]",
            t.replace('\'', "\\'")
        )),
        Locator::AttrEquals { attr, value } => Ok(format!(
            "xpath://*[@{}='{}']",
            attr,
            value.replace('\'', "\\'")
        )),
        Locator::AttrContains { attr, value } => Ok(format!(
            "xpath://*[contains(@{},'{}')]",
            attr,
            value.replace('\'', "\\'")
        )),
        Locator::Chain(locators) => locators
            .last()
            .ok_or_else(|| Error::InvalidLocator("empty chain".into()))
            .and_then(locator_to_query),
    }
}

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
