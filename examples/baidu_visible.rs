//! 非无头模式演示 — 弹出真实 Chrome 窗口

use rpage::WebPage;

#[tokio::main]
async fn main() -> rpage::Result<()> {
    println!("🚀 启动浏览器（非无头模式）...");
    let mut page = WebPage::new().await?;

    println!("📡 导航到百度...");
    page.get("https://www.baidu.com").await?;
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let title = page.title().await?;
    println!("📄 标题: {title}");

    println!("⌨️  输入: rust教程");
    let search_box = page.ele("#kw").await?;
    search_box
        .js("this.value = 'rust教程'; this.dispatchEvent(new Event('input', {bubbles: true}));")
        .await?;

    println!("🖱️  点击搜索...");
    page.ele("#su").await?.click().await?;

    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    let title = page.title().await?;
    println!("📄 标题: {title}");

    let results = page.eles("h3").await?;
    println!("\n📋 搜索结果 ({} 条):", results.len());
    for (i, r) in results.iter().enumerate() {
        let text = r.text();
        if !text.is_empty() {
            println!("  {}. {}", i + 1, text);
        }
    }

    Ok(())
}
