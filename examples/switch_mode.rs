//! WebPage mode switching - browser <-> session with cookie sync
use rpage::WebPage;

#[tokio::main]
async fn main() -> rpage::Result<()> {
    // Start in browser mode
    let mut page = WebPage::new().await?;
    page.get("https://example.com").await?;

    let title = page.title().await?;
    println!("[Chromium] Title: {title}");
    println!("Mode: {}", page.mode());

    // Switch to session mode (cookies auto-synced)
    page.to_session().await?;
    println!("Mode: {}", page.mode());

    // Switch back to browser mode
    page.to_chromium().await?;
    println!("Mode: {}", page.mode());

    Ok(())
}
