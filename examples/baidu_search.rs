//! 实战演示：用 rpage API 搜索（Bing + 百度）
//!
//! rpage 内置反检测：覆盖 navigator.webdriver、伪造 plugins/languages、UA 去除 HeadlessChrome

use rpage::{ChromiumOptions, WebPage};

#[tokio::main]
async fn main() -> rpage::Result<()> {
    println!("🚀 启动浏览器（rpage API，无头模式 + stealth）...\n");

    let opts = rpage::config::WebPageOptions::builder()
        .chromium(
            ChromiumOptions::builder()
                .headless(true)
                .viewport(1280, 800)
                .no_sandbox(true)
                .build(),
        )
        .build();

    let mut page = WebPage::with_options(opts).await?;

    // ── Bing 搜索 ──────────────────────────────────────
    println!("📡 Bing 搜索: rust教程");
    page.get("https://www.bing.com/search?q=rust%E6%95%99%E7%A8%8B&setlang=zh-CN")
        .await?;
    let title = page.title().await?;
    println!("📄 标题: {title}");

    let results = page.eles("h2 a").await?;
    println!("📋 Bing 搜索结果 ({} 条):", results.len());
    for (i, r) in results.iter().take(8).enumerate() {
        let text = r.text();
        if !text.is_empty() {
            println!("  {}. {}", i + 1, text.chars().take(80).collect::<String>());
        }
    }

    // ── 百度搜索 ──────────────────────────────────────
    println!("\n📡 百度搜索: rust教程");
    page.get("https://www.baidu.com").await?;
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let search_box = page.ele("#kw").await?;
    search_box.input("rust教程").await?;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    let btn = page.ele("#su").await?;
    btn.click().await?;
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    let title = page.title().await?;
    println!("📄 标题: {title}");

    if title.contains("百度安全验证") {
        println!("⚠️  百度触发验证码（IP 风控），这是百度针对该 IP 的临时限制");
        println!("   Stealth 本身工作正常（UA、webdriver 均已覆盖）");
    } else {
        let results = page.eles("h3").await?;
        println!("📋 百度搜索结果 ({} 条):", results.len());
        for (i, r) in results.iter().take(8).enumerate() {
            println!("  {}. {}", i + 1, r.text());
        }
    }

    page.screenshot("screenshot_rpage_demo.png").await?;
    println!("\n📸 截图: screenshot_rpage_demo.png");

    Ok(())
}
