//! 实战演示：非无头模式打开百度搜索"rust教程"（浏览器可见）
//!
//! 运行后会弹出 Chrome 窗口，实时看到每一步操作

use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::page::ScreenshotParams;
use futures::StreamExt;
use std::io::Write;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 启动浏览器（非无头模式，窗口可见）...");

    let (browser, mut handler) = Browser::launch(
        BrowserConfig::builder()
            .window_size(1280, 800)
            .with_head() // ← 关键：显示浏览器窗口
            .arg("--disable-blink-features=AutomationControlled")
            .arg("--disable-infobars")
            .arg("--no-first-run")
            .user_data_dir(std::env::temp_dir().join("rpage-baidu-visible"))
            .no_sandbox()
            .build()?,
    )
    .await?;

    tokio::spawn(async move { while handler.next().await.is_some() {} });

    let page = browser.new_page("about:blank").await?;

    // 反检测 JS
    page.evaluate(
        r#"
        Object.defineProperty(navigator, 'webdriver', {get: () => undefined});
        Object.defineProperty(navigator, 'plugins', {get: () => [1, 2, 3, 4, 5]});
        Object.defineProperty(navigator, 'languages', {get: () => ['zh-CN', 'zh', 'en']});
    "#,
    )
    .await?;

    println!("📡 导航到百度...");
    page.goto("https://www.baidu.com").await?;
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let title = page.get_title().await?;
    println!("✅ 已打开: {}", title.unwrap_or_default());
    println!("   👆 你应该能看到 Chrome 窗口已打开百度首页");

    tokio::time::sleep(std::time::Duration::from_secs(1)).await;

    // JS 逐字输入 + 模拟真实键盘事件
    println!("⌨️  模拟键盘输入: rust教程");
    for ch in "rust教程".chars() {
        page.evaluate(
            format!(
                r#"
            (function() {{
                var input = document.querySelector('#kw');
                input.value += '{ch}';
                input.dispatchEvent(new Event('input', {{bubbles: true}}));
                input.dispatchEvent(new KeyboardEvent('keyup', {{key: '{ch}', bubbles: true}}));
            }})()
        "#
            )
            .as_str(),
        )
        .await?;
        tokio::time::sleep(std::time::Duration::from_millis(
            150 + (ch as u32 % 80) as u64,
        ))
        .await;
    }
    println!("✅ 输入完成");

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    println!("🖱️  点击搜索按钮...");
    page.evaluate(r#"document.querySelector('#su').click();"#)
        .await?;

    println!("⏳ 等待搜索结果加载...");
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    let new_title = page.get_title().await?;
    let url = page.url().await?;
    println!("\n📄 标题: {}", new_title.unwrap_or_default());
    println!("🔗 URL: {}", url.unwrap_or_default());

    // 截图留念
    let png = page.screenshot(ScreenshotParams::builder().build()).await?;
    let mut f = std::fs::File::create("screenshot_baidu_visible.png")?;
    f.write_all(&png)?;
    println!(
        "📸 截图: screenshot_baidu_visible.png ({} bytes)",
        png.len()
    );

    // 提取搜索结果
    let result = page
        .evaluate(
            r#"
        Array.from(document.querySelectorAll('h3')).slice(0, 10).map((h, i) => {
            let a = h.querySelector('a');
            return (i+1) + '. ' + (a ? a.textContent : h.textContent);
        }).join('\n');
    "#,
        )
        .await?;

    if let Some(text) = result.value().and_then(|v| v.as_str()) {
        if text.trim().is_empty() {
            println!("\n⚠️  未获取到搜索结果（可能触发了验证码）");
        } else {
            println!("\n📋 搜索结果:");
            for line in text.lines() {
                println!("  {line}");
            }
        }
    }

    println!("\n✅ 演示完成！浏览器窗口保持打开...");
    println!("   按 Ctrl+C 退出");
    tokio::signal::ctrl_c().await.ok();

    Ok(())
}
