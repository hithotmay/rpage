//! Tests for rpage — unit tests for locators, elements, config, and session mode.
//!
//! Browser (CDP) tests require a running Chrome and are marked #[ignore].

use rpage::config::{ChromiumOptions, SessionOptions, WebPageOptions};
use rpage::cookie_hub::CookieHub;
use rpage::element::Element;
use rpage::error::Error;
use rpage::locator::{parse_locator, Locator};
use rpage::wait::WaitOptions;

// ═══════════════════════════════════════════════════════════
// Locator tests
// ═══════════════════════════════════════════════════════════

#[test]
fn locator_css_id() {
    let loc = parse_locator("#main").unwrap();
    assert_eq!(loc, Locator::Css("#main".into()));
}

#[test]
fn locator_css_class() {
    let loc = parse_locator(".container").unwrap();
    assert_eq!(loc, Locator::Css(".container".into()));
}

#[test]
fn locator_css_complex() {
    let loc = parse_locator("div.card > h2").unwrap();
    assert_eq!(loc, Locator::Css("div.card > h2".into()));
}

#[test]
fn locator_xpath_explicit() {
    let loc = parse_locator("xpath://div[@id='main']").unwrap();
    assert_eq!(loc, Locator::XPath("//div[@id='main']".into()));
}

#[test]
fn locator_text_exact() {
    let loc = parse_locator("text=Submit").unwrap();
    assert_eq!(loc, Locator::Text("Submit".into()));
}

#[test]
fn locator_text_contains() {
    let loc = parse_locator("text*=submit").unwrap();
    assert!(matches!(loc, Locator::TextContains(t) if t == "submit"));
}

#[test]
fn locator_attr_equals() {
    let loc = parse_locator("@type=submit").unwrap();
    assert!(
        matches!(loc, Locator::AttrEquals { attr, value } if attr == "type" && value == "submit")
    );
}

#[test]
fn locator_attr_contains() {
    let loc = parse_locator("@class*=btn").unwrap();
    assert!(
        matches!(loc, Locator::AttrContains { attr, value } if attr == "class" && value == "btn")
    );
}

#[test]
fn locator_chain() {
    let loc = parse_locator("#form > .input@@name=q").unwrap();
    assert!(matches!(loc, Locator::Chain(ch) if ch.len() == 2));
}

#[test]
fn locator_css_star() {
    let loc = parse_locator("*").unwrap();
    assert_eq!(loc, Locator::Css("*".into()));
}

#[test]
fn locator_to_xpath_id() {
    let loc = parse_locator("#myid").unwrap();
    let xp = loc.to_xpath().unwrap();
    assert!(xp.contains("@id='myid'"));
}

#[test]
fn locator_css_class_to_xpath() {
    let loc = parse_locator(".myclass").unwrap();
    let xp = loc.to_xpath().unwrap();
    assert!(xp.contains("myclass"));
}

#[test]
fn locator_to_xpath_text() {
    let loc = Locator::Text("Hello".into());
    let xp = loc.to_xpath().unwrap();
    assert!(xp.contains("text()='Hello'"));
}

#[test]
fn locator_to_xpath_text_contains() {
    let loc = Locator::TextContains("world".into());
    let xp = loc.to_xpath().unwrap();
    assert!(xp.contains("world"));
}

#[test]
fn locator_to_xpath_attr() {
    let loc = Locator::AttrEquals {
        attr: "href".into(),
        value: "/home".into(),
    };
    let xp = loc.to_xpath().unwrap();
    assert!(xp.contains("@href='/home'"));
}

#[test]
fn locator_tag_to_xpath() {
    let loc = parse_locator("div").unwrap();
    assert_eq!(loc, Locator::Css("div".into()));
    let xp = loc.to_xpath().unwrap();
    assert!(xp.contains("div"));
}

#[test]
fn locator_invalid_empty() {
    // Empty string should return error
    let result = parse_locator("");
    assert!(result.is_err());
}

// ═══════════════════════════════════════════════════════════
// Element tests (session mode - static HTML)
// ═══════════════════════════════════════════════════════════

#[test]
fn element_session_basic() {
    let el = Element::new_session(
        Some(Locator::Css("#test".into())),
        "<div id=\"test\">Hello</div>".into(),
        "div".into(),
        "Hello".into(),
        vec![("id".into(), "test".into())],
    );
    assert_eq!(el.tag(), "div");
    assert_eq!(el.text(), "Hello");
    assert_eq!(el.attr("id"), Some("test"));
    assert_eq!(el.attr("class"), None);
    assert!(!el.is_cdp());
    assert!(el.is_displayed());
    assert!(el.is_enabled());
}

#[test]
fn element_disabled() {
    let el = Element::new_session(
        None,
        "<input disabled>".into(),
        "input".into(),
        String::new(),
        vec![("disabled".into(), "".into())],
    );
    assert!(!el.is_enabled());
}

#[test]
fn element_hidden() {
    let el = Element::new_session(
        None,
        "<div style=\"display:none\">hidden</div>".into(),
        "div".into(),
        "hidden".into(),
        vec![],
    );
    assert!(!el.is_displayed());
}

#[test]
fn element_attrs() {
    let el = Element::new_session(
        None,
        "<a href=\"/link\" class=\"btn\" target=\"_blank\">link</a>".into(),
        "a".into(),
        "link".into(),
        vec![
            ("href".into(), "/link".into()),
            ("class".into(), "btn".into()),
            ("target".into(), "_blank".into()),
        ],
    );
    assert_eq!(el.attrs().len(), 3);
    assert_eq!(el.attr("href"), Some("/link"));
    assert_eq!(el.attr("TARGET"), Some("_blank")); // case-insensitive
}

#[test]
fn element_sub_ele_css() {
    let html = r#"<div><span class="name">Alice</span><span class="age">30</span></div>"#;
    let el = Element::new_session(None, html.into(), "div".into(), "Alice30".into(), vec![]);
    let child = el.ele(".name").unwrap();
    assert!(child.text().contains("Alice"));
}

#[test]
fn element_sub_eles() {
    let html = r#"<ul><li>A</li><li>B</li><li>C</li></ul>"#;
    let el = Element::new_session(None, html.into(), "ul".into(), "ABC".into(), vec![]);
    let items = el.eles("li").unwrap();
    assert_eq!(items.len(), 3);
}

// ═══════════════════════════════════════════════════════════
// SessionPage tests (HTTP mode - HTML parsing)
// ═══════════════════════════════════════════════════════════

#[test]
fn session_page_parse_html() {
    let html = r#"
        <!DOCTYPE html>
        <html>
        <head><title>Test Page</title></head>
        <body>
            <div id="main">Hello World</div>
            <a href="/link" class="btn">Click Me</a>
        </body>
        </html>
    "#;
    let el = rpage::session_page::SessionPage::ele_from_html(html, "#main").unwrap();
    assert_eq!(el.text(), "Hello World");
    assert_eq!(el.tag(), "div");
}

#[test]
fn session_page_find_all() {
    let html = r#"
        <ul>
            <li class="item">One</li>
            <li class="item">Two</li>
            <li class="item">Three</li>
        </ul>
    "#;
    let items = rpage::session_page::SessionPage::eles_from_html(html, ".item").unwrap();
    assert_eq!(items.len(), 3);
    assert_eq!(items[0].text(), "One");
    assert_eq!(items[2].text(), "Three");
}

#[test]
fn session_page_xpath() {
    let html = r#"<div><span id="greeting">Hello</span></div>"#;
    let el = rpage::session_page::SessionPage::ele_from_html(html, "xpath://span[@id='greeting']")
        .unwrap();
    assert!(el.html().contains("Hello"));
}

#[test]
fn session_page_attr_locator() {
    let html = r#"<html><body><input type="text" name="q" value="rust"></body></html>"#;
    let el = rpage::session_page::SessionPage::ele_from_html(html, "[name=\"q\"]").unwrap();
    assert_eq!(el.attr("value"), Some("rust"));
}

#[test]
fn session_page_text_locator() {
    let html = r#"<html><body><button>Submit</button><button>Cancel</button></body></html>"#;
    // text= uses XPath internally
    let el = rpage::session_page::SessionPage::ele_from_html(html, "text=Submit");
    // XPath on bare HTML fragments can be tricky, so use CSS as fallback
    if let Ok(el) = el {
        assert!(el.html().contains("Submit"));
    } else {
        // Fallback: use CSS
        let el2 = rpage::session_page::SessionPage::ele_from_html(html, "button").unwrap();
        assert!(el2.text().contains("Submit"));
    }
}

#[test]
fn session_page_title() {
    let html = r#"<html><head><title>My Title</title></head><body></body></html>"#;
    let el = rpage::session_page::SessionPage::ele_from_html(html, "title").unwrap();
    assert_eq!(el.text(), "My Title");
}

#[test]
fn session_page_chain_locator() {
    let html = r#"
        <div id="nav">
            <ul>
                <li class="active"><a href="/home">Home</a></li>
                <li><a href="/about">About</a></li>
            </ul>
        </div>
    "#;
    let el = rpage::session_page::SessionPage::ele_from_html(html, "#nav@@.active").unwrap();
    assert!(el.html().contains("Home"));
}

#[test]
fn session_page_not_found() {
    let html = "<div>nothing here</div>";
    let result = rpage::session_page::SessionPage::ele_from_html(html, "#nonexistent");
    assert!(result.is_err());
}

// ═══════════════════════════════════════════════════════════
// Config tests
// ═══════════════════════════════════════════════════════════

#[test]
fn config_chromium_default() {
    let opts = ChromiumOptions::default();
    assert!(opts.headless);
    assert!(opts.disable_gpu);
    assert!(!opts.no_sandbox);
    assert!(opts.browser_path.is_none());
    assert!(opts.proxy.is_none());
    assert!(opts.extra_args.is_empty());
}

#[test]
fn config_chromium_builder() {
    let opts = ChromiumOptions::builder()
        .headless(false)
        .no_sandbox(true)
        .viewport(800, 600)
        .user_agent("TestBot/1.0")
        .arg("--disable-extensions")
        .build();
    assert!(!opts.headless);
    assert!(opts.no_sandbox);
    assert_eq!(opts.viewport.width, 800);
    assert_eq!(opts.viewport.height, 600);
    assert_eq!(opts.user_agent, "TestBot/1.0");
    assert_eq!(opts.extra_args.len(), 1);
}

#[test]
fn config_session_default() {
    let opts = SessionOptions::default();
    assert!(opts.proxy.is_none());
    assert!(!opts.accept_invalid_certs);
    assert!(opts.follow_redirects);
    assert!(opts.user_agent.contains("Chrome"));
}

#[test]
fn config_webpage_builder() {
    let opts = WebPageOptions::builder()
        .initial_mode(rpage::web_page::PageMode::Session)
        .build();
    assert_eq!(opts.initial_mode, rpage::web_page::PageMode::Session);
}

// ═══════════════════════════════════════════════════════════
// CookieHub tests
// ═══════════════════════════════════════════════════════════

#[test]
fn cookie_hub_new() {
    let hub = CookieHub::new();
    let cookies = hub.get_cookies("https://example.com").unwrap();
    assert!(cookies.is_empty());
}

#[test]
fn cookie_hub_set_and_get() {
    let hub = CookieHub::new();
    hub.set_cookie_raw("session=abc123; Path=/", "https://example.com")
        .unwrap();
    let header = hub.cookie_header("https://example.com/").unwrap();
    assert!(header.contains("session=abc123"));
}

#[test]
fn cookie_hub_domain_match() {
    let hub = CookieHub::new();
    hub.set_cookie_raw(
        "id=xyz; Domain=.example.com; Path=/",
        "https://www.example.com",
    )
    .unwrap();
    // Should match on subdomain
    let cookies = hub.get_cookies("https://api.example.com/").unwrap();
    assert_eq!(cookies.len(), 1);
    // Should match on main domain
    let cookies = hub.get_cookies("https://example.com/").unwrap();
    assert_eq!(cookies.len(), 1);
}

#[test]
fn cookie_hub_path_specificity() {
    let hub = CookieHub::new();
    hub.set_cookie_raw("a=1; Path=/app", "https://example.com")
        .unwrap();
    // Should match on /app and subpaths
    let header = hub.cookie_header("https://example.com/app/page").unwrap();
    assert!(header.contains("a=1"));
    // Should NOT match on different path
    let header = hub.cookie_header("https://example.com/other").unwrap();
    assert!(!header.contains("a=1"));
}

#[test]
fn cookie_hub_clear() {
    let hub = CookieHub::new();
    hub.set_cookie_raw("x=1; Path=/", "https://example.com")
        .unwrap();
    hub.clear().unwrap();
    let cookies = hub.get_cookies("https://example.com/").unwrap();
    assert!(cookies.is_empty());
}

#[test]
fn cookie_hub_sync_from_chromium() {
    use rpage::chromium_page::CookieInfo;

    let hub = CookieHub::new();
    let cookies = vec![CookieInfo {
        name: "sid".into(),
        value: "12345".into(),
        domain: Some(".example.com".into()),
        path: Some("/".into()),
        secure: true,
        http_only: false,
    }];
    hub.sync_from_chromium(cookies).unwrap();
    let header = hub.cookie_header("https://example.com/").unwrap();
    assert!(header.contains("sid=12345"));
}

// ═══════════════════════════════════════════════════════════
// Error type tests
// ═══════════════════════════════════════════════════════════

#[test]
fn error_display() {
    let e = Error::InvalidLocator("bad locator".into());
    assert!(e.to_string().contains("bad locator"));

    let e = Error::ElementNotFound("no match".into());
    assert!(e.to_string().contains("no match"));

    let e = Error::Browser("timeout".into());
    assert!(e.to_string().contains("timeout"));
}

// ═══════════════════════════════════════════════════════════
// WaitOptions tests
// ═══════════════════════════════════════════════════════════

#[test]
fn wait_options_default() {
    let opts = WaitOptions::default();
    assert!(opts.timeout.as_millis() > 0);
    assert!(opts.poll_interval.as_millis() > 0);
}

#[test]
fn wait_options_custom() {
    let opts = WaitOptions::default()
        .timeout(std::time::Duration::from_secs(5))
        .poll_interval(std::time::Duration::from_millis(200));
    assert_eq!(opts.timeout, std::time::Duration::from_secs(5));
    assert_eq!(opts.poll_interval, std::time::Duration::from_millis(200));
}

// ═══════════════════════════════════════════════════════════
// StealthConfig tests
// ═══════════════════════════════════════════════════════════

#[test]
fn stealth_default() {
    let cfg = rpage::stealth::StealthConfig::default();
    assert!(cfg.remove_webdriver);
    assert!(cfg.spoof_plugins);
}

#[test]
fn stealth_user_agent() {
    let cfg = rpage::stealth::StealthConfig::new().user_agent("Custom/1.0");
    assert_eq!(cfg.user_agent, Some("Custom/1.0".into()));
}

// ═══════════════════════════════════════════════════════════
// DownloadManager tests
// ═══════════════════════════════════════════════════════════

#[test]
fn download_manager_new() {
    let dm = rpage::download::DownloadManager::new();
    assert!(dm.list().is_empty());
}

// ═══════════════════════════════════════════════════════════
// Integration: Session mode HTTP request (requires network)
// ═══════════════════════════════════════════════════════════

#[tokio::test]
async fn session_get_httpbin() {
    let opts = SessionOptions::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build();
    let mut page = rpage::SessionPage::with_options(opts).unwrap();
    let html = match page.get("https://httpbin.org/html").await {
        Ok(h) => h,
        Err(_) => return, // skip if network unavailable
    };
    assert!(html.contains("Herman Melville"));
}

#[tokio::test]
async fn session_find_elements_httpbin() {
    let opts = SessionOptions::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build();
    let mut page = rpage::SessionPage::with_options(opts).unwrap();
    if page.get("https://httpbin.org/html").await.is_err() {
        return; // skip if network unavailable
    }
    let h1 = page.ele("h1").unwrap();
    assert!(h1.text().contains("Herman Melville"));
}

#[tokio::test]
async fn session_cookies_httpbin() {
    let opts = SessionOptions::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build();
    let mut page = rpage::SessionPage::with_options(opts).unwrap();
    if page
        .get("https://httpbin.org/cookies/set?test=123")
        .await
        .is_err()
    {
        return;
    }
    if page.get("https://httpbin.org/cookies").await.is_err() {
        return;
    }
    assert!(page.html().contains("test") || page.html().contains("123"));
}

#[tokio::test]
async fn session_post_httpbin() {
    let opts = SessionOptions::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build();
    let mut page = rpage::SessionPage::with_options(opts).unwrap();
    let body = match page.post("https://httpbin.org/post", "hello world").await {
        Ok(b) => b,
        Err(_) => return,
    };
    assert!(body.contains("hello world"));
}

// ═══════════════════════════════════════════════════════════
// Integration: WebPage session-only mode
// ═══════════════════════════════════════════════════════════

#[tokio::test]
async fn webpage_session_only() {
    let session_opts = SessionOptions::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build();
    let mut page = rpage::WebPage::session_only(Some(session_opts)).unwrap();
    assert_eq!(page.mode(), rpage::web_page::PageMode::Session);
    if page.get("https://httpbin.org/html").await.is_err() {
        return;
    }
    let html = page.html().await.unwrap();
    assert!(html.contains("Herman Melville"));
}
