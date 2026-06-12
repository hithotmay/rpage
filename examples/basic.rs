//! Basic WebPage usage - launch browser and navigate
use rpage::WebPage;

#[tokio::main]
async fn main() -> rpage::Result<()> {

    let page = WebPage::new().await?;
    // let page = ChromiumPage::connect("http://localhost:9222").await?;
    for _i in 1..2{
        page.activate_tab("Example").await?;
        // page.get("https://example.com").await?;

        let title = page.title().await?;
        let url = page.url().await?;
        let html = page.html().await?;

        println!("Title: {title}");
        println!("URL: {url}");
        println!("html: {html}");
        page.activate_tab("bili").await?;
        // page.ele("text=Pull requests").await?; // 操作 GitHub 标签
        let title = page.title().await?;
        let url = page.url().await?;
        println!("Title: {title}");
        println!("URL: {url}");
        // 切换到另一个标签
        page.activate_tab_by_url("https://www.runoob.com/").await?;
        let title = page.title().await?;
        let url = page.url().await?;
        println!("Title: {title}");
        println!("URL: {url}");
        page.click_ele("text=用户笔记").await?;
    }
    Ok(())
}
