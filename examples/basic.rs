//! Basic WebPage usage - launch browser and navigate
use rpage::WebPage;

#[tokio::main]
async fn main() -> rpage::Result<()> {
    let page = WebPage::new().await?;
    let page = ChromiumPage::connect("http://localhost:9222").await?;
    page.get("https://example.com").await?;

    let title = page.title().await?;
    let url = page.url().await?;
    println!("Title: {title}");
    println!("URL: {url}");

    page.activate_tab("GitHub").await?;
    page.ele("text=Pull requests").await?; // 操作 GitHub 标签

    // 切换到另一个标签
    page.activate_tab_by_url("stackoverflow.com").await?;
    Ok(())
}
