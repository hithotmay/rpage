# rpage 🦀🌐

**English** | [简体中文](README.zh-CN.md)

> A Rust browser-automation library inspired by [DrissionPage](https://github.com/g1879/DrissionPage) — CDP browser control + HTTP session + shared cookies, in one crate.

`rpage` drives a real Chrome/Chromium over the DevTools Protocol *and* speaks
plain HTTP (reqwest) in the same API, with cookies shared between the two. It
aims for the ergonomics of DrissionPage with parity-or-better coverage of
Playwright's most useful features (response mocking, semantic locators,
actionability waits).

```toml
[dependencies]
rpage = "1"
```

## Why rpage

- **One call to start** — `WebPage::new()` detects Chrome, launches it, connects over CDP.
- **No automation fingerprint** — never passes `--enable-automation`; behaves like a hand-opened browser.
- **Dual mode** — browser (CDP) and HTTP session in one API, with cookie sync and `to_session()`/`to_chromium()` switching.
- **Smart waiting** — `get()` waits for load; `ele()`/`eles()` auto-retry; click waits for actionability.
- **Sync *and* async** — full async API, plus a zero-`await` `SyncPage` wrapper.
- **Unicode-perfect input** — `fill()` types Chinese/any Unicode via CDP `insertText`.
- **Rich locators** — CSS, XPath, text/attr operators (`=` `:` `^` `$`), same-element `@@`/`@|`, chains.
- **Network superpowers** — capture request/response bodies, mock responses (`route.fulfill`), intercept/redirect/block, raw CDP passthrough.

## Quick start

```rust
use rpage::prelude::*;

#[tokio::main]
async fn main() -> rpage::Result<()> {
    let page = WebPage::new().await?;                 // launch Chrome
    page.get("https://example.com").await?;           // navigate (auto-waits)
    page.ele("#search").await?.fill("rust 教程").await?; // type (Unicode OK)
    page.ele("#submit").await?.click().await?;        // waits for actionability
    for text in page.eles("h3").await?.texts() {      // auto-retry locate
        println!("{text}");
    }
    Ok(())
}
```

### Chaining

```rust
page.goto("https://example.com").await?
    .type_text("#search", "query").await?
    .click_ele("#submit").await?;
```

### Sync API — no `await`, no `#[tokio::main]`

```rust
use rpage::sync::SyncPage;

fn main() -> rpage::Result<()> {
    let p = SyncPage::connect("http://127.0.0.1:9222")?; // takes over the active tab
    p.get("https://example.com")?;
    p.type_text("input[name=q]", "rust")?;
    p.click_ele("#submit")?;
    println!("{} — {} cookies", p.title()?, p.cookies()?.len());
    Ok(())
}
```

## Locator syntax

Text/attribute operators follow DrissionPage: `=` exact, `:` contains, `^` starts-with, `$` ends-with.

| Syntax | Meaning |
|--------|---------|
| `#id`, `.class`, `tag`, `css:…` | CSS |
| `xpath:…` | XPath |
| `@class=btn` / `@class:btn` / `@class^btn` / `@class$btn` | attribute exact / contains / prefix / suffix |
| `text=Login` / `text:Login` / `text^Log` / `text$in` | text exact / contains / prefix / suffix |
| `tag:div@@class=x@@text:y` | **same-element AND** (all conditions) |
| `@|class=a@|class=b` | **same-element OR** (any condition) |
| `a@@@b@@@c` | descendant chain (rpage extension) |

Non-CSS locators fall back to a JS `document.evaluate` XPath query automatically.

## Capabilities

Below is a tour of the high-value APIs. The **full API (580+ methods) is on
[docs.rs/rpage](https://docs.rs/rpage)**.

### Elements

```rust
let el = page.ele("#item").await?;
el.click().await?;                    // CDP click, JS fallback, waits actionable
el.fill("text").await?;               // clear + type (Unicode)
el.right_click().await?; el.double_click().await?;
el.drag_to(&target).await?;           // or drag_to_offset(dx, dy)
el.select("Option").await?;           // <select>
el.upload_file("/path").await?;
el.is_visible().await; el.is_in_viewport().await; el.is_alive().await; // states
let parent = el.parent().await?;      // parent/first_child/next/prev/children
el.wait_for_visible().await?;         // + hidden/enabled/clickable/stale/text/attribute
```

### Semantic locators (Playwright `get_by_*`)

```rust
page.get_by_text("Login").await?;
page.get_by_role("button").await?;        // explicit + implicit ARIA roles
page.get_by_label("Username").await?;     // aria-label / <label for> / wrapping
page.get_by_placeholder("Search").await?;
page.get_by_test_id("submit").await?;     // data-testid
```

### Network capture & mocking

```rust
// Raw CDP passthrough for anything not wrapped:
page.run_cdp("Page.getLayoutMetrics", serde_json::json!({})).await?;

// Capture full request+response+body (DrissionPage listen.wait/steps):
page.listen_start().await?;
page.ele("#load").await?.click().await?;
let pkt = page.wait_data_packet("/api/data", 10).await?;      // wait for one
println!("{} {} -> {}", pkt.status, pkt.url, pkt.body_text());
for p in page.data_packets("/api/").await { /* all matches */ }

// Mock a response without hitting the network (Playwright route.fulfill):
let guard = page.enable_intercept("*://*/api/*").await?;
page.click_ele("#load").await?;
for req in guard.paused_requests() {
    guard.fulfill_json(req.request_id.as_ref(), r#"{"mock":true}"#).await?;
    // or: continue_request(id, redirect) / fail_request(id) /
    //     fulfill_request(id, status, &headers, body)
}
guard.disable().await?;
```

### Static-snapshot elements (DrissionPage `s_ele`)

```rust
let rows = page.s_eles("table tr").await?;   // parse current HTML once, no live CDP
for row in &rows { println!("{}", row.text()); }
```

### Dual mode, cookies, lifecycle

```rust
page.to_session().await?;             // browser → HTTP
let resp = page.session_get(url).await?;
page.to_chromium().await?;            // HTTP → browser
page.save_cookies_to_file("c.json").await?;
page.quit().await?;
```

### More (see docs.rs)

Tabs, scrolling, screenshots/PDF, iframes, alerts, downloads, console &
WebSocket capture, performance metrics, init scripts & CSS injection, device
emulation (DPR/touch/geolocation/timezone), window management, `ActionChain`,
stealth, and an **Agent interface** (`interactive_elements()`,
`page_snapshot()`, `smart_click()`, `smart_fill()`) for LLM-driven automation.

## Configuration

```rust
use rpage::{WebPage, WebPageOptions, ChromiumOptions};

let page = WebPage::with_options(WebPageOptions {
    chromium: ChromiumOptions::builder()
        .headless(true)
        .debug_port(19222)
        .proxy("http://127.0.0.1:7890")
        .build(),
    ..Default::default()
}).await?;
```

Set `RPAGE_CHROME_PATH` to point at a specific Chrome/Chromium binary.

## Requirements

- A locally installed Chrome or Chromium.
- Rust 1.75+ (2021 edition).

## Stability

`rpage` follows [SemVer](https://semver.org/). Starting at **1.0**, breaking
API changes will only land in a new major version.

## License

MIT
