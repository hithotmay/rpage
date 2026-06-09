//! 一个函数启动浏览器 + 百度搜索 — 零自动化标记，永不触发验证码
//!
//! `WebPage::new()` 自动：检测 Chrome → 启动子进程 → 等待端口就绪 → CDP 连接

use rpage::WebPage;

#[tokio::main]
async fn main() -> rpage::Result<()> {
    println!("🚀 一个函数启动浏览器...");
    let mut page = WebPage::new().await?;

    // ── 百度搜索 ──────────────────────────────────────
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
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    println!("🖱️  点击搜索...");
    page.ele("#su").await?.click().await?;

    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    let title = page.title().await?;
    println!("📄 标题: {title}");

    let results = page.eles("h3").await?;
    println!("\n📋 百度搜索结果 ({} 条):", results.len());
    for (i, r) in results.iter().enumerate() {
        let text = r.text();
        if !text.is_empty() {
            println!("  {}. {}", i + 1, text);
        }
    }

    page.screenshot("screenshot_baidu.png").await?;
    println!("\n📸 截图: screenshot_baidu.png");

    Ok(())
}
