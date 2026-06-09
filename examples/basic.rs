//! Basic WebPage usage - launch browser and navigate
use rpage::WebPage;

#[tokio::main]
async fn main() -> rpage::Result<()> {
    let page = WebPage::new().await?;
    page.get("https://example.com").await?;

    let title = page.title().await?;
    let url = page.url().await?;
    println!("Title: {title}");
    println!("URL: {url}");

    Ok(())
}
