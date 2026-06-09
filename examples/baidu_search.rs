//! 🚀 rpage 极简百度搜索 — 零验证码，零手动 sleep
//!
//! WebPage::new() 自动：检测 Chrome → 启动子进程 → 等待就绪 → CDP 连接
//! get() 自动等待页面加载完成
//! ele() 自动重试等待元素出现（最多 5 秒）
//! fill() 自动清空+输入，支持中文

use rpage::WebPage;

#[tokio::main]
async fn main() -> rpage::Result<()> {
    // 一行启动浏览器
    let page = WebPage::new().await?;

    // 自动等待页面加载
    page.get("https://www.baidu.com").await?;
    println!("📄 标题: {}", page.title().await?);

    // fill = 清空 + 输入，支持中文
    page.ele("#kw").await?.fill("rust教程").await?;

    // 点击搜索
    page.ele("#su").await?.click().await?;

    // 等一下让搜索结果加载
    page.sleep(std::time::Duration::from_secs(2)).await;

    // 获取结果
    println!("📄 标题: {}", page.title().await?);
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
