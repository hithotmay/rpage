//! SyncPage demo — no #[tokio::main], no .await, pure synchronous style.
//!
//! Usage:
//!   1. Launch Chrome with remote debugging:
//!      chrome --remote-debugging-port=9222
//!   2. Run this example:
//!      cargo run --example sync_demo
//!
//! The example will connect to your existing browser and operate on the
//! currently active tab — no new tabs are opened.

use rpage::sync_page::SyncPage;

fn main() -> rpage::Result<()> {
    println!("=== SyncPage Demo ===\n");

    // Connect to existing Chrome debug port
    // Picks up the currently active (visible) tab automatically.
    let page = SyncPage::connect("http://127.0.0.1:9222")?;
    println!("✓ Connected to active tab");

    // Navigate
    page.get("https://www.example.com")?;
    println!("✓ Navigated to example.com");

    // Page info
    let title = page.title()?;
    let url = page.url()?;
    println!("  Title: {title}");
    println!("  URL:   {url}");

    // Find element
    let h1 = page.ele("tag:h1")?;
    println!("  H1:    {}", h1.text());

    // Get all links
    let links = page.links()?;
    println!("  Links: {} found", links.len());
    for link in links.iter().take(3) {
        println!("    - {link}");
    }

    // Screenshot
    page.screenshot("sync_demo_screenshot.png")?;
    println!("✓ Screenshot saved");

    // JS execution
    let result = page.execute("document.querySelectorAll('p').length")?;
    println!("  Paragraph count: {result}");

    // Scroll
    page.scroll_to_bottom()?;
    page.scroll_to_top()?;
    println!("✓ Scroll test passed");

    // Cookie
    let cookies = page.cookies()?;
    println!("  Cookies: {} found", cookies.len());

    // Window bounds
    let (x, y, w, h) = page.get_window_bounds()?;
    println!("  Window: ({x},{y}) {w}x{h}");

    // Interactive elements (Agent API)
    let elements = page.interactive_elements()?;
    println!("  Interactive elements: {}", elements.len());

    // Page snapshot (Agent API)
    let snapshot = page.page_snapshot()?;
    println!("  Page snapshot: {} interactive, {} chars visible text",
        snapshot.interactive_elements.len(),
        snapshot.visible_text.len().min(100));

    println!("\n=== All Done ===");
    Ok(())
}
