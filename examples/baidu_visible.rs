//! 实战演示：用 rpage API 非无头搜索（弹出真实 Chrome 窗口）
//!
//! rpage 内置反检测：覆盖 navigator.webdriver、伪造 plugins/languages

use rpage::{ChromiumOptions, WebPage};

#[tokio::main]
async fn main() -> rpage::Result<()> {
    println!("🚀 启动浏览器（rpage API，非无头模式）...");

    let opts = rpage::config::WebPageOptions::builder()
        .chromium(
            ChromiumOptions::builder()
                .headless(false)
                .viewport(1280, 800)
                .no_sandbox(true)
                .build(),
        )
        .build();

    let mut page = WebPage::with_options(opts).await?;

    println!("📡 导航到百度...");
    page.get("https://www.baidu.com").await?;
    let title = page.title().await?;
    println!("✅ 已打开: {title}");

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    println!("⌨️  搜索框输入: rust教程");
    let search_box = page.ele("#kw").await?;
    search_box.input("rust教程").await?;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    println!("🖱️  点击搜索...");
    let btn = page.ele("#su").await?;
    btn.click().await?;

    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    let title = page.title().await?;
    let url = page.url().await?;
    println!("📄 标题: {title}");
    println!("🔗 URL: {url}");

    if title.contains("百度安全验证") {
        println!("⚠️  百度触发验证码（IP 风控）");
    } else {
        let results = page.eles("h3").await?;
        println!("\n📋 搜索结果 ({} 条):", results.len());
        for (i, r) in results.iter().enumerate() {
            println!("  {}. {}", i + 1, r.text());
        }
    }

    println!("\n👀 窗口将保持 5 秒...");
    tokio::time::sleep(std::time::Duration::from_secs(5)).await;

    Ok(())
}
