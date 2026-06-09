//! Locator parsing and matching
//!
//! Supported locator syntax:
//! - `#id` → Css("#id")
//! - `.class` → Css(".class")
//! - `tag` → Css("tag")
//! - `css:xxx` → Css("xxx")
//! - `xpath:xxx` → XPath("xxx")
//! - `text=xxx` → Text("xxx")
//! - `text*=xxx` → TextContains("xxx")
//! - `@attr=val` → AttrEquals("attr", "val")
//! - `@attr*=val` → AttrContains("attr", "val")
//! - `tag:form@@text=Login` → Chain([Css("form"), Text("Login")])

use crate::error::{Error, Result};

/// Locator strategy for finding elements
#[derive(Debug, Clone, PartialEq)]
pub enum Locator {
    /// CSS selector
    Css(String),
    /// XPath expression
    XPath(String),
    /// Exact text match
    Text(String),
    /// Text contains match
    TextContains(String),
    /// Attribute equals value
    AttrEquals { attr: String, value: String },
    /// Attribute contains value
    AttrContains { attr: String, value: String },
    /// Chained locators (narrow down step by step)
    Chain(Vec<Locator>),
}

impl Locator {
    /// Convert this locator to a CSS selector if possible
    pub fn to_css(&self) -> Option<String> {
        match self {
            Locator::Css(s) => Some(s.clone()),
            Locator::Text(_t) => {
                // XPath only, no direct CSS equivalent for exact text
                None
            }
            Locator::TextContains(_) => None,
            Locator::XPath(_) => None,
            Locator::AttrEquals { .. } => None,
            Locator::AttrContains { .. } => None,
            Locator::Chain(_) => None,
        }
    }

    /// Convert to an XPath expression
    pub fn to_xpath(&self) -> Option<String> {
        match self {
            Locator::XPath(x) => Some(x.clone()),
            Locator::Css(sel) => {
                // Basic CSS to XPath: prepend //
                let mut xpath = String::from("//");
                // Simple conversion for common cases
                if let Some(id) = sel.strip_prefix('#') {
                    xpath.push_str(&format!("*[@id='{id}']"));
                } else if let Some(cls) = sel.strip_prefix('.') {
                    xpath.push_str(&format!(
                        "*[contains(concat(' ',normalize-space(@class),' '),' {cls} ')]"
                    ));
                } else {
                    xpath.push_str(sel);
                }
                Some(xpath)
            }
            Locator::Text(t) => Some(format!("//*[text()='{}']", t.replace('\'', "\\'"))),
            Locator::TextContains(t) => Some(format!(
                "//*[contains(text(),'{}')]",
                t.replace('\'', "\\'")
            )),
            Locator::AttrEquals { attr, value } => {
                Some(format!("//*[@{}='{}']", attr, value.replace('\'', "\\'")))
            }
            Locator::AttrContains { attr, value } => Some(format!(
                "//*[contains(@{},'{}')]",
                attr,
                value.replace('\'', "\\'")
            )),
            Locator::Chain(locators) => {
                // Build a combined XPath from chain
                let mut parts = Vec::new();
                for loc in locators {
                    if let Some(xp) = loc.to_xpath() {
                        parts.push(xp);
                    } else {
                        return None;
                    }
                }
                Some(parts.join(" | "))
            }
        }
    }

    /// Check if this is a pure CSS locator
    pub fn is_css(&self) -> bool {
        matches!(self, Locator::Css(_))
    }

    /// Check if this requires XPath
    pub fn is_xpath(&self) -> bool {
        matches!(
            self,
            Locator::XPath(_)
                | Locator::Text(_)
                | Locator::TextContains(_)
                | Locator::AttrEquals { .. }
                | Locator::AttrContains { .. }
        )
    }
}

/// Parse a locator string into a Locator enum
pub fn parse_locator(input: &str) -> Result<Locator> {
    let input = input.trim();
    if input.is_empty() {
        return Err(Error::InvalidLocator("empty locator string".into()));
    }

    // Check for chain separator: @@@ or @@
    if input.contains("@@@") {
        let parts: Vec<&str> = input.split("@@@").collect();
        if parts.len() < 2 {
            return Err(Error::InvalidLocator(format!(
                "invalid chain locator: {input}"
            )));
        }
        let locators: Vec<Locator> = parts
            .iter()
            .map(|p| parse_single_locator(p.trim()))
            .collect::<Result<Vec<_>>>()?;
        return Ok(Locator::Chain(locators));
    }

    // Check for tag:xxx@@text=yyy pattern (2-part chain)
    if input.contains("@@") && !input.starts_with('@') {
        let parts: Vec<&str> = input.splitn(2, "@@").collect();
        if parts.len() == 2 {
            let first = parse_single_locator(parts[0].trim())?;
            let second = parse_single_locator(parts[1].trim())?;
            return Ok(Locator::Chain(vec![first, second]));
        }
    }

    parse_single_locator(input)
}

/// Parse a single (non-chained) locator
fn parse_single_locator(input: &str) -> Result<Locator> {
    let input = input.trim();
    if input.is_empty() {
        return Err(Error::InvalidLocator("empty locator segment".into()));
    }

    // xpath:xxx
    if let Some(rest) = input.strip_prefix("xpath:") {
        if rest.is_empty() {
            return Err(Error::InvalidLocator(
                "xpath: requires an expression".into(),
            ));
        }
        return Ok(Locator::XPath(rest.to_string()));
    }

    // css:xxx
    if let Some(rest) = input.strip_prefix("css:") {
        if rest.is_empty() {
            return Err(Error::InvalidLocator("css: requires a selector".into()));
        }
        return Ok(Locator::Css(rest.to_string()));
    }

    // text*=xxx (must check before text=)
    if let Some(rest) = input.strip_prefix("text*=") {
        if rest.is_empty() {
            return Err(Error::InvalidLocator("text*= requires a value".into()));
        }
        return Ok(Locator::TextContains(rest.to_string()));
    }

    // text=xxx
    if let Some(rest) = input.strip_prefix("text=") {
        if rest.is_empty() {
            return Err(Error::InvalidLocator("text= requires a value".into()));
        }
        return Ok(Locator::Text(rest.to_string()));
    }

    // @attr*=val (must check before @attr=val)
    if input.starts_with('@') && input.contains("*=") {
        let rest = &input[1..]; // remove leading @
        if let Some(pos) = rest.find("*=") {
            let attr = &rest[..pos];
            let value = &rest[pos + 2..];
            if attr.is_empty() {
                return Err(Error::InvalidLocator(
                    "@attr*=val requires attr name".into(),
                ));
            }
            return Ok(Locator::AttrContains {
                attr: attr.to_string(),
                value: value.to_string(),
            });
        }
    }

    // @attr=val
    if input.starts_with('@') && input.contains('=') {
        let rest = &input[1..]; // remove leading @
        if let Some(pos) = rest.find('=') {
            let attr = &rest[..pos];
            let value = &rest[pos + 1..];
            if attr.is_empty() {
                return Err(Error::InvalidLocator("@attr=val requires attr name".into()));
            }
            return Ok(Locator::AttrEquals {
                attr: attr.to_string(),
                value: value.to_string(),
            });
        }
    }

    // tag:xxx → treat "xxx" as CSS tag selector
    if let Some(rest) = input.strip_prefix("tag:") {
        if rest.is_empty() {
            return Err(Error::InvalidLocator("tag: requires a tag name".into()));
        }
        return Ok(Locator::Css(rest.to_string()));
    }

    // #id → CSS selector
    if input.starts_with('#') {
        return Ok(Locator::Css(input.to_string()));
    }

    // .class → CSS selector
    if input.starts_with('.') {
        return Ok(Locator::Css(input.to_string()));
    }

    // [attr=val] → CSS selector
    if input.starts_with('[') {
        return Ok(Locator::Css(input.to_string()));
    }

    // Plain tag name (letters only, no special chars)
    if input
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Ok(Locator::Css(input.to_string()));
    }

    // Fallback: treat as CSS selector
    Ok(Locator::Css(input.to_string()))
}

/// Trait for converting types into Locator
pub trait IntoLocator {
    fn to_locator(&self) -> Result<Locator>;
}

impl IntoLocator for &str {
    fn to_locator(&self) -> Result<Locator> {
        parse_locator(self)
    }
}

impl IntoLocator for String {
    fn to_locator(&self) -> Result<Locator> {
        parse_locator(self)
    }
}

impl IntoLocator for Locator {
    fn to_locator(&self) -> Result<Locator> {
        Ok(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_css_id() {
        let loc = parse_locator("#myid").unwrap();
        assert_eq!(loc, Locator::Css("#myid".to_string()));
    }

    #[test]
    fn test_css_class() {
        let loc = parse_locator(".myclass").unwrap();
        assert_eq!(loc, Locator::Css(".myclass".to_string()));
    }

    #[test]
    fn test_tag() {
        let loc = parse_locator("div").unwrap();
        assert_eq!(loc, Locator::Css("div".to_string()));
    }

    #[test]
    fn test_css_prefix() {
        let loc = parse_locator("css:div.container > p").unwrap();
        assert_eq!(loc, Locator::Css("div.container > p".to_string()));
    }

    #[test]
    fn test_xpath_prefix() {
        let loc = parse_locator("xpath://div[@id='foo']").unwrap();
        assert_eq!(loc, Locator::XPath("//div[@id='foo']".to_string()));
    }

    #[test]
    fn test_text_exact() {
        let loc = parse_locator("text=Login").unwrap();
        assert_eq!(loc, Locator::Text("Login".to_string()));
    }

    #[test]
    fn test_text_contains() {
        let loc = parse_locator("text*=Log").unwrap();
        assert_eq!(loc, Locator::TextContains("Log".to_string()));
    }

    #[test]
    fn test_attr_equals() {
        let loc = parse_locator("@name=login").unwrap();
        assert_eq!(
            loc,
            Locator::AttrEquals {
                attr: "name".to_string(),
                value: "login".to_string()
            }
        );
    }

    #[test]
    fn test_attr_contains() {
        let loc = parse_locator("@name*=log").unwrap();
        assert_eq!(
            loc,
            Locator::AttrContains {
                attr: "name".to_string(),
                value: "log".to_string()
            }
        );
    }

    #[test]
    fn test_chain_double_at() {
        let loc = parse_locator("tag:form@@text=Login").unwrap();
        match loc {
            Locator::Chain(parts) => {
                assert_eq!(parts.len(), 2);
                assert_eq!(parts[0], Locator::Css("form".to_string()));
                assert_eq!(parts[1], Locator::Text("Login".to_string()));
            }
            _ => panic!("expected Chain"),
        }
    }

    #[test]
    fn test_chain_triple_at() {
        let loc = parse_locator("tag:form@@@name=login").unwrap();
        match loc {
            Locator::Chain(parts) => {
                assert_eq!(parts.len(), 2);
            }
            _ => panic!("expected Chain"),
        }
    }
}
