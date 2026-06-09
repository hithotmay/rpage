//! SessionPage - pure HTTP mode using reqwest

use std::sync::Arc;

use scraper::{Html, Selector};
use tracing::debug;

use crate::config::SessionOptions;
use crate::cookie_hub::CookieHub;
use crate::element::{from_scraper_element, Element};
use crate::error::{Error, Result};
use crate::locator::Locator;

/// SessionPage wraps reqwest for pure HTTP request mode.
///
/// No browser is launched. Sends HTTP requests and parses the HTML.
#[allow(dead_code)]
pub struct SessionPage {
    client: reqwest::Client,
    cookie_hub: Arc<CookieHub>,
    current_html: String,
    document: Option<Html>,
    current_url: Option<String>,
    opts: SessionOptions,
}

impl SessionPage {
    /// Create with default options.
    pub fn new() -> Result<Self> {
        Self::with_options(SessionOptions::default())
    }

    /// Create with custom options.
    pub fn with_options(opts: SessionOptions) -> Result<Self> {
        let cookie_hub = Arc::new(CookieHub::new());
        let client = Self::build_client(&opts, &cookie_hub)?;
        Ok(Self {
            client,
            cookie_hub,
            current_html: String::new(),
            document: None,
            current_url: None,
            opts,
        })
    }

    /// Create sharing an existing CookieHub (used by WebPage).
    pub(crate) fn with_cookie_hub(
        cookie_hub: Arc<CookieHub>,
        opts: SessionOptions,
    ) -> Result<Self> {
        let client = Self::build_client(&opts, &cookie_hub)?;
        Ok(Self {
            client,
            cookie_hub,
            current_html: String::new(),
            document: None,
            current_url: None,
            opts,
        })
    }

    fn build_client(opts: &SessionOptions, cookie_hub: &Arc<CookieHub>) -> Result<reqwest::Client> {
        let store = cookie_hub.store().clone();
        let mut builder = reqwest::Client::builder()
            .timeout(opts.timeout)
            .cookie_provider(store)
            .user_agent(&opts.user_agent);

        if let Some(ref proxy) = opts.proxy {
            let proxy = reqwest::Proxy::all(proxy).map_err(|e| Error::Config(e.to_string()))?;
            builder = builder.proxy(proxy);
        }
        if opts.accept_invalid_certs {
            builder = builder.danger_accept_invalid_certs(true);
        }

        builder.build().map_err(Error::Reqwest)
    }

    // ── Accessors ────────────────────────────────────────────

    pub fn cookie_hub(&self) -> &Arc<CookieHub> {
        &self.cookie_hub
    }

    /// Cached HTML.
    pub fn html(&self) -> &str {
        &self.current_html
    }

    /// Page title (from parsed HTML).
    pub fn title(&self) -> Option<String> {
        self.document
            .as_ref()
            .and_then(|doc| {
                let sel = Selector::parse("title").ok()?;
                doc.select(&sel).next()
            })
            .map(|el| el.text().collect::<Vec<_>>().join(""))
    }

    /// Current URL.
    pub fn url(&self) -> Option<&str> {
        self.current_url.as_deref()
    }

    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    // ── HTTP methods ─────────────────────────────────────────

    /// GET request, cache the response.
    pub async fn get(&mut self, url: &str) -> Result<String> {
        debug!("GET {url}");
        let resp = self.client.get(url).send().await?;
        let status = resp.status();
        debug!("Response status: {status}");

        let text = resp.text().await?;
        self.current_html = text;
        self.document = Some(Html::parse_document(&self.current_html));
        self.current_url = Some(url.to_string());
        Ok(self.current_html.clone())
    }

    /// POST request with plain text body.
    pub async fn post(&mut self, url: &str, body: impl Into<reqwest::Body>) -> Result<String> {
        debug!("POST {url}");
        let resp = self.client.post(url).body(body).send().await?;
        let text = resp.text().await?;
        self.current_html = text.clone();
        self.document = Some(Html::parse_document(&self.current_html));
        self.current_url = Some(url.to_string());
        Ok(text)
    }

    /// POST JSON.
    pub async fn post_json(
        &mut self,
        url: &str,
        json: &serde_json::Value,
    ) -> Result<reqwest::Response> {
        debug!("POST (json) {url}");
        let resp = self.client.post(url).json(json).send().await?;
        Ok(resp)
    }

    /// Raw GET without caching.
    pub async fn get_raw(&self, url: &str) -> Result<reqwest::Response> {
        let resp = self.client.get(url).send().await?;
        Ok(resp)
    }

    // ── Element queries ──────────────────────────────────────

    /// Find first matching element.
    pub fn ele(&self, locator_str: &str) -> Result<Element> {
        let locator = crate::locator::parse_locator(locator_str)?;
        let doc = self
            .document
            .as_ref()
            .ok_or_else(|| Error::ElementNotFound("no page loaded".into()))?;
        find_element_in_doc(doc, &locator)
    }

    /// Find all matching elements.
    pub fn eles(&self, locator_str: &str) -> Result<Vec<Element>> {
        let locator = crate::locator::parse_locator(locator_str)?;
        let doc = self
            .document
            .as_ref()
            .ok_or_else(|| Error::ElementNotFound("no page loaded".into()))?;
        find_elements_in_doc(doc, &locator)
    }

    /// Find element in arbitrary HTML string.
    pub fn ele_from_html(html: &str, locator_str: &str) -> Result<Element> {
        let locator = crate::locator::parse_locator(locator_str)?;
        let doc = Html::parse_document(html);
        find_element_in_doc(&doc, &locator)
    }

    /// Find all elements in arbitrary HTML string.
    pub fn eles_from_html(html: &str, locator_str: &str) -> Result<Vec<Element>> {
        let locator = crate::locator::parse_locator(locator_str)?;
        let doc = Html::parse_document(html);
        find_elements_in_doc(&doc, &locator)
    }
}

// ── Internal helpers ─────────────────────────────────────────

fn find_element_in_doc(doc: &Html, locator: &Locator) -> Result<Element> {
    match locator {
        Locator::Css(sel) => {
            let selector =
                Selector::parse(sel).map_err(|e| Error::InvalidLocator(e.to_string()))?;
            doc.select(&selector)
                .next()
                .map(|el| from_scraper_element(&el, Some(locator.clone()), None))
                .ok_or_else(|| Error::ElementNotFound(format!("no match: {sel}")))
        }
        Locator::XPath(_)
        | Locator::Text(_)
        | Locator::TextContains(_)
        | Locator::AttrEquals { .. }
        | Locator::AttrContains { .. } => {
            let xpath = locator
                .to_xpath()
                .ok_or_else(|| Error::InvalidLocator("cannot convert to XPath".into()))?;
            find_by_xpath(&doc.html(), &xpath, Some(locator.clone()))
        }
        Locator::Chain(locators) => {
            let mut current_html = doc.html();
            let mut result: Option<Element> = None;
            for sub in locators {
                let sub_doc = Html::parse_document(&current_html);
                let el = find_element_in_doc(&sub_doc, sub)?;
                current_html = el.html().to_string();
                result = Some(el);
            }
            result.ok_or_else(|| Error::ElementNotFound("chain: no result".into()))
        }
    }
}

fn find_elements_in_doc(doc: &Html, locator: &Locator) -> Result<Vec<Element>> {
    match locator {
        Locator::Css(sel) => {
            let selector =
                Selector::parse(sel).map_err(|e| Error::InvalidLocator(e.to_string()))?;
            Ok(doc
                .select(&selector)
                .map(|el| from_scraper_element(&el, Some(locator.clone()), None))
                .collect())
        }
        Locator::XPath(_)
        | Locator::Text(_)
        | Locator::TextContains(_)
        | Locator::AttrEquals { .. }
        | Locator::AttrContains { .. } => {
            let xpath = locator
                .to_xpath()
                .ok_or_else(|| Error::InvalidLocator("cannot convert to XPath".into()))?;
            find_all_by_xpath(&doc.html(), &xpath, Some(locator.clone()))
        }
        Locator::Chain(locators) => {
            let mut current_html = doc.html();
            let mut chain = locators.iter().peekable();
            while let Some(sub) = chain.next() {
                if chain.peek().is_none() {
                    let sub_doc = Html::parse_document(&current_html);
                    return find_elements_in_doc(&sub_doc, sub);
                }
                let sub_doc = Html::parse_document(&current_html);
                let el = find_element_in_doc(&sub_doc, sub)?;
                current_html = el.html().to_string();
            }
            Ok(Vec::new())
        }
    }
}

fn find_by_xpath(html: &str, xpath_expr: &str, locator: Option<Locator>) -> Result<Element> {
    let package =
        sxd_document::parser::parse(html).map_err(|e| Error::InvalidLocator(e.to_string()))?;
    let document = package.as_document();
    let value = sxd_xpath::evaluate_xpath(&document, xpath_expr)
        .map_err(|e| Error::InvalidLocator(format!("XPath: {e}")))?;
    let nodes = match value {
        sxd_xpath::Value::Nodeset(ns) => ns,
        _ => return Err(Error::ElementNotFound("XPath: non-nodeset".into())),
    };
    let node = nodes
        .iter()
        .next()
        .ok_or_else(|| Error::ElementNotFound(format!("no match: {xpath_expr}")))?;
    Ok(Element::new_session(
        locator,
        node.string_value(),
        String::new(),
        String::new(),
        Vec::new(),
    ))
}

fn find_all_by_xpath(
    html: &str,
    xpath_expr: &str,
    locator: Option<Locator>,
) -> Result<Vec<Element>> {
    let package =
        sxd_document::parser::parse(html).map_err(|e| Error::InvalidLocator(e.to_string()))?;
    let document = package.as_document();
    let value = sxd_xpath::evaluate_xpath(&document, xpath_expr)
        .map_err(|e| Error::InvalidLocator(format!("XPath: {e}")))?;
    let nodes = match value {
        sxd_xpath::Value::Nodeset(ns) => ns,
        _ => return Ok(Vec::new()),
    };
    Ok(nodes
        .iter()
        .map(|node| {
            Element::new_session(
                locator.clone(),
                node.string_value(),
                String::new(),
                String::new(),
                Vec::new(),
            )
        })
        .collect())
}
