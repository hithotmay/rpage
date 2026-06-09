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
    pub async fn fill(&self, text: &str) -> Result<()> {
        // Use JS directly — most reliable, works with all characters including Chinese
        let escaped = text.replace('\\', "\\\\").replace('\'', "\\'");
        let js = format!(
            "(function() {{ \
               this.focus(); \
               var nativeInputValueSetter = Object.getOwnPropertyDescriptor(\
                 window.HTMLInputElement.prototype, 'value').set;\
               nativeInputValueSetter.call(this, '{}'); \
               this.dispatchEvent(new Event('input', {{bubbles: true}})); \
               this.dispatchEvent(new Event('change', {{bubbles: true}})); \
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

    /// Execute JavaScript with `this` bound to this element.
    pub async fn js(&self, script: &str) -> Result<()> {
        let page = self
            .page
            .as_ref()
            .ok_or(Error::Browser("js requires Chromium mode".into()))?;

        let locator = self
            .locator
            .as_ref()
            .ok_or(Error::Browser("no locator for element".into()))?;
        let selector = locator_to_query(locator)?;

        let wrapped = format!(
            "(function(){{ var el = document.querySelector('{}'); if(!el) return; (function(){{ {} }}).call(el); }})()",
            selector.replace('\\', "\\\\").replace('\'', "\\'"),
            script,
        );
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
