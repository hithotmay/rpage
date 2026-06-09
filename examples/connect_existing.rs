//! 接管已打开的浏览器 — 永不触发验证码
//!
//! 步骤：
//! 1. 先用命令行启动 Chrome（用你自己的 profile，已登录的账号都在）：
//!    "C:\Program Files\Google\Chrome\Application\chrome.exe" --remote-debugging-port=9222
//! 2. 运行本示例：cargo run --example connect_existing

use rpage::WebPage;

#[tokio::main]
async fn main() -> rpage::Result<()> {
    println!("🔗 接管已打开的浏览器...");
    println!("   请确保 Chrome 已用 --remote-debugging-port=9222 启动\n");

    let page = WebPage::connect("http://localhost:9222").await?;

    // ── 百度搜索（永不触发验证码，因为是你自己的浏览器）──
    println!("📡 导航到百度...");
    page.get("https://www.baidu.com").await?;
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let title = page.title().await?;
    println!("📄 标题: {title}");

    println!("⌨️  输入: rust教程");
    let search_box = page.ele("#kw").await?;
    // 用 JS 直接设置 value + dispatchEvent，避免中文编码问题
    search_box
        .js("this.value = 'rust教程'; this.dispatchEvent(new Event('input', {bubbles: true}));")
        .await?;
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    println!("🖱️  点击搜索...");
    let btn = page.ele("#su").await?;
    btn.click().await?;

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

    page.screenshot("screenshot_connect.png").await?;
    println!("\n📸 截图: screenshot_connect.png");

    Ok(())
}
