//! 实战演示：打开百度搜索"rust教程"并点击搜索（带反检测）

use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::page::ScreenshotParams;
use futures::StreamExt;
use std::io::Write;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 启动浏览器（带反检测）...");

    // 清理可能的残留 user-data-dir 临时目录
    for entry in std::fs::read_dir(std::env::temp_dir())? {
        let e = entry?;
        let name = e.file_name();
        let s = name.to_string_lossy();
        if s.starts_with("rpage-chrome-") {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }

    let (browser, mut handler) = Browser::launch(
        BrowserConfig::builder()
            .window_size(1280, 800)
            .arg("--disable-blink-features=AutomationControlled")
            .arg("--disable-infobars")
            .arg("--no-first-run")
            .arg("--disable-background-networking")
            .arg("--disable-client-side-phishing-detection")
            .arg("--disable-default-apps")
            .arg("--disable-extensions")
            .arg("--disable-hang-monitor")
            .arg("--disable-popup-blocking")
            .arg("--disable-prompt-on-repost")
            .arg("--disable-sync")
            .arg("--metrics-recording-only")
            .arg("--safebrowsing-disable-auto-update")
            .user_data_dir(std::env::temp_dir().join("rpage-baidu-test"))
            .no_sandbox()
            .build()?,
    )
    .await?;

    tokio::spawn(async move { while handler.next().await.is_some() {} });

    let page = browser.new_page("about:blank").await?;

    // 注入反检测 JS
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

    // 截图
    let png = page.screenshot(ScreenshotParams::builder().build()).await?;
    let mut f = std::fs::File::create("screenshot_baidu_home.png")?;
    f.write_all(&png)?;
    println!(
        "📸 首页截图: screenshot_baidu_home.png ({} bytes)",
        png.len()
    );

    println!("🔍 输入: rust教程");
    page.evaluate(
        r#"
        var input = document.querySelector('#kw');
        input.focus();
        input.value = 'rust教程';
        input.dispatchEvent(new Event('input', {bubbles: true}));
    "#,
    )
    .await?;

    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    println!("🖱️  点击搜索...");
    page.evaluate(r#"document.querySelector('#su').click();"#)
        .await?;

    println!("⏳ 等待搜索结果...");
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    let new_title = page.get_title().await?;
    let url = page.url().await?;
    println!("\n📄 标题: {}", new_title.unwrap_or_default());
    println!("🔗 URL: {}", url.unwrap_or_default());

    // 截图
    let png = page.screenshot(ScreenshotParams::builder().build()).await?;
    let mut f = std::fs::File::create("screenshot_baidu_results.png")?;
    f.write_all(&png)?;
    println!(
        "📸 搜索截图: screenshot_baidu_results.png ({} bytes)",
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

    println!("\n📋 搜索结果:");
    if let Some(text) = result.value().and_then(|v| v.as_str()) {
        for line in text.lines() {
            println!("  {line}");
        }
    }

    println!("\n✅ 演示完成！10 秒后关闭...");
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;

    Ok(())
}
