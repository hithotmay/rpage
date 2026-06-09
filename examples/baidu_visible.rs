//! 实战演示：非无头模式打开百度搜索"rust教程"（真实 Chrome，无验证码）
//!
//! 关键反检测策略：
//! 1. chrome_executable() 指定 Chrome 而非 Edge
//! 2. --disable-blink-features=AutomationControlled 抑制自动化特征
//! 3. evaluate_on_new_document 在每个页面注入 stealth 脚本
//! 4. 逐字输入 + 随机延迟模拟真人打字

use chromiumoxide::browser::{Browser, BrowserConfig};
use chromiumoxide::page::ScreenshotParams;
use futures::StreamExt;
use std::io::Write;

/// Stealth 脚本：在每个新文档加载前执行
const STEALTH_JS: &str = r#"
    // 1. 删除 navigator.webdriver
    Object.defineProperty(navigator, 'webdriver', { get: () => undefined });
    // 2. 伪装 plugins（空 plugins 是自动化标志）
    Object.defineProperty(navigator, 'plugins', {
        get: () => {
            const a = [
                { name: 'Chrome PDF Plugin', filename: 'internal-pdf-viewer' },
                { name: 'Chrome PDF Viewer', filename: 'mhjfbmdgcfjbbpaeojofohoefgiehjai' },
                { name: 'Native Client', filename: 'internal-nacl-plugin' },
            ];
            a.refresh = () => {};
            return a;
        }
    });
    // 3. 伪装 chrome runtime
    if (!window.chrome) window.chrome = {};
    if (!window.chrome.runtime) window.chrome.runtime = { connect: function(){}, sendMessage: function(){} };
    // 4. 伪装 permissions
    const origQuery = window.navigator.permissions.query;
    window.navigator.permissions.query = (p) =>
        p.name === 'notifications'
            ? Promise.resolve({ state: Notification.permission })
            : origQuery(p);
    // 5. 伪装 languages
    Object.defineProperty(navigator, 'languages', { get: () => ['zh-CN', 'zh', 'en-US', 'en'] });
"#;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let chrome_path = r"C:\Program Files\Google\Chrome\Application\chrome.exe";
    if !std::path::Path::new(chrome_path).exists() {
        eprintln!("❌ 未找到 Chrome: {chrome_path}");
        std::process::exit(1);
    }

    println!("🚀 启动 Chrome（非无头模式）...");

    let (browser, mut handler) = Browser::launch(
        BrowserConfig::builder()
            .chrome_executable(chrome_path)
            .window_size(1280, 800)
            .with_head()
            .arg("--disable-blink-features=AutomationControlled")
            .user_data_dir(std::env::temp_dir().join("rpage-chrome-stealth"))
            .no_sandbox()
            .build()?,
    )
    .await?;

    tokio::spawn(async move { while handler.next().await.is_some() {} });

    let page = browser.new_page("about:blank").await?;

    // 关键：在每个新文档加载前注入 stealth 脚本
    page.evaluate_on_new_document(STEALTH_JS).await?;
    println!("✅ Stealth 脚本已注入");

    println!("📡 导航到百度...");
    page.goto("https://www.baidu.com").await?;
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let title = page.get_title().await?;
    println!("✅ 已打开: {}", title.unwrap_or_default());

    // 截图首页
    let png = page.screenshot(ScreenshotParams::builder().build()).await?;
    let mut f = std::fs::File::create("screenshot_baidu_home.png")?;
    f.write_all(&png)?;
    println!("📸 首页截图: screenshot_baidu_home.png");

    // 逐字输入，模拟真人
    println!("⌨️  输入: rust教程");
    for ch in "rust教程".chars() {
        page.evaluate(
            format!(
                r#"
            (function() {{
                var el = document.querySelector('#kw');
                if (!el) return;
                var pos = el.value.length;
                el.value += '{ch}';
                el.dispatchEvent(new Event('input', {{ bubbles: true }}));
            }})()
        "#
            )
            .as_str(),
        )
        .await?;
        let delay = 120 + ((ch as u32).wrapping_mul(7) % 160) as u64;
        tokio::time::sleep(std::time::Duration::from_millis(delay)).await;
    }
    println!("✅ 输入完成");

    tokio::time::sleep(std::time::Duration::from_millis(600)).await;

    println!("🖱️  点击搜索...");
    page.evaluate(
        r#"
        var btn = document.querySelector('#su');
        if (btn) { btn.click(); }
    "#,
    )
    .await?;

    println!("⏳ 等待搜索结果...");
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    let new_title = page.get_title().await?;
    let url = page.url().await?;
    println!("\n📄 标题: {}", new_title.unwrap_or_default());
    println!("🔗 URL: {}", url.unwrap_or_default());

    // 截图结果页
    let png = page.screenshot(ScreenshotParams::builder().build()).await?;
    let mut f = std::fs::File::create("screenshot_baidu_results.png")?;
    f.write_all(&png)?;
    println!("📸 结果截图: screenshot_baidu_results.png");

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
            println!("\n⚠️  未获取到搜索结果（可能验证码）");
        } else {
            println!("\n📋 搜索结果:");
            for line in text.lines() {
                println!("  {line}");
            }
        }
    }

    println!("\n✅ 完成！按 Ctrl+C 退出");
    tokio::signal::ctrl_c().await.ok();

    Ok(())
}
