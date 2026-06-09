//! Session-only mode - pure HTTP, no browser needed
use rpage::SessionPage;

fn main() -> rpage::Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    let mut page = SessionPage::new()?;

    rt.block_on(async {
        let html = page.get("https://example.com").await?;
        println!("Got {} bytes", html.len());

        // Parse elements
        let title = page.title();
        println!("Title: {title:?}");

        // Find elements
        if let Ok(h1) = page.ele("h1") {
            println!("H1 text: {}", h1.text());
        }

        // Find all paragraphs
        let paras = page.eles("p")?;
        println!("Found {} paragraphs", paras.len());

        Ok::<(), rpage::Error>(())
    })?;

    Ok(())
}
