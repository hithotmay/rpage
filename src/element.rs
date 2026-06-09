//! Element abstraction over CDP and parsed-HTML elements

use crate::error::{Error, Result};
use crate::locator::{parse_locator, Locator};

/// Identifier for the page this element belongs to
#[derive(Debug, Clone, PartialEq)]
pub enum PageId {
    /// CDP-based browser page
    Cdp(String),
    /// Session-based (parsed HTML)
    Session,
}

/// An element found on a page.
///
/// Stores enough metadata to re-resolve the element if it becomes stale
/// (e.g. after a page navigation or DOM mutation).
#[derive(Debug, Clone)]
pub struct Element {
    /// Which page this element belongs to
    page_id: PageId,
    /// The locator used to find this element (for re-resolve)
    locator: Option<Locator>,
    /// Outer HTML snapshot
    html: String,
    /// Tag name
    tag: String,
    /// Text content
    text: String,
    /// Attributes
    attrs: Vec<(String, String)>,
}

impl Element {
    /// Create a new element from parsed data
    pub fn new(
        page_id: PageId,
        locator: Option<Locator>,
        html: String,
        tag: String,
        text: String,
        attrs: Vec<(String, String)>,
    ) -> Self {
        Self {
            page_id,
            locator,
            html,
            tag,
            text,
            attrs,
        }
    }

    /// Create an element with CDP data (extracted from chromiumoxide Element)
    pub fn with_cdp_data(
        page_id: impl Into<String>,
        html: String,
        tag: String,
        text: String,
        attrs: Vec<(String, String)>,
        locator: Option<Locator>,
    ) -> Self {
        Self {
            page_id: PageId::Cdp(page_id.into()),
            locator,
            html,
            tag,
            text,
            attrs,
        }
    }

    /// The locator that was used to find this element
    pub fn locator(&self) -> Option<&Locator> {
        self.locator.as_ref()
    }

    /// Tag name
    pub fn tag(&self) -> &str {
        &self.tag
    }

    /// Text content
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Inner / outer HTML
    pub fn html(&self) -> &str {
        &self.html
    }

    /// Get an attribute value by name
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }

    /// All attributes
    pub fn attrs(&self) -> &[(String, String)] {
        &self.attrs
    }

    /// The page ID
    pub fn page_id(&self) -> &PageId {
        &self.page_id
    }

    /// Check if this is a CDP-backed element
    pub fn is_cdp(&self) -> bool {
        matches!(self.page_id, PageId::Cdp(_))
    }

    /// Find a child element by locator string (works on cached HTML)
    pub fn ele(&self, locator_str: &str) -> Result<Element> {
        let locator = parse_locator(locator_str)?;
        // Parse the cached HTML and search within it
        let doc = scraper::Html::parse_document(&self.html);
        match &locator {
            Locator::Css(sel) => {
                let selector = scraper::Selector::parse(sel)
                    .map_err(|e| Error::InvalidLocator(e.to_string()))?;
                doc.select(&selector)
                    .next()
                    .map(|el| from_scraper_element(&el, Some(locator.clone())))
                    .ok_or_else(|| {
                        Error::ElementNotFound(format!("sub-element not found: {locator_str}"))
                    })
            }
            _ => {
                // For non-CSS locators, search in cached HTML via xpath conversion
                let xpath = locator
                    .to_xpath()
                    .ok_or_else(|| Error::InvalidLocator("cannot convert to XPath".into()))?;
                let package = sxd_document::parser::parse(&self.html)
                    .map_err(|e| Error::InvalidLocator(e.to_string()))?;
                let document = package.as_document();
                let value = sxd_xpath::evaluate_xpath(&document, &xpath)
                    .map_err(|e| Error::InvalidLocator(format!("XPath error: {e}")))?;
                let nodes = match value {
                    sxd_xpath::Value::Nodeset(ns) => ns,
                    _ => return Err(Error::ElementNotFound("XPath returned non-nodeset".into())),
                };
                let node = nodes.iter().next().ok_or_else(|| {
                    Error::ElementNotFound(format!("sub-element not found: {locator_str}"))
                })?;
                let node_str = node.string_value();
                Ok(Element::new(
                    self.page_id.clone(),
                    Some(locator),
                    node_str,
                    String::new(),
                    String::new(),
                    Vec::new(),
                ))
            }
        }
    }

    /// Find all child elements matching the locator
    pub fn eles(&self, locator_str: &str) -> Result<Vec<Element>> {
        let locator = parse_locator(locator_str)?;
        let doc = scraper::Html::parse_document(&self.html);
        match &locator {
            Locator::Css(sel) => {
                let selector = scraper::Selector::parse(sel)
                    .map_err(|e| Error::InvalidLocator(e.to_string()))?;
                Ok(doc
                    .select(&selector)
                    .map(|el| from_scraper_element(&el, Some(locator.clone())))
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
                    .map_err(|e| Error::InvalidLocator(format!("XPath error: {e}")))?;
                let nodes = match value {
                    sxd_xpath::Value::Nodeset(ns) => ns,
                    _ => return Err(Error::ElementNotFound("XPath returned non-nodeset".into())),
                };
                Ok(nodes
                    .iter()
                    .map(|node| {
                        let node_str = node.string_value();
                        Element::new(
                            self.page_id.clone(),
                            Some(locator.clone()),
                            node_str,
                            String::new(),
                            String::new(),
                            Vec::new(),
                        )
                    })
                    .collect())
            }
        }
    }

    /// Check if element is likely displayed (has non-empty dimensions)
    pub fn is_displayed(&self) -> bool {
        !self.html.contains("display:none") && !self.html.contains("display: none")
    }

    /// Check if element is enabled (not disabled)
    pub fn is_enabled(&self) -> bool {
        !self.attrs.iter().any(|(k, _)| k == "disabled")
    }
}

/// Build an Element from a scraper::ElementRef
pub(crate) fn from_scraper_element(el: &scraper::ElementRef, locator: Option<Locator>) -> Element {
    let tag = el.value().name().to_string();
    let attrs: Vec<(String, String)> = el
        .value()
        .attrs()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let text = el.text().collect::<Vec<_>>().join("");
    let html = el.html();
    Element::new(PageId::Session, locator, html, tag, text, attrs)
}
