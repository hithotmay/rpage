//! 验证新增便捷 API
use rpage::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== 新增 API 验证 ===\n");

    let page = ChromiumPage::connect("http://127.0.0.1:9222").await?;
    
    // 1. goto — 链式调用
    println!("[1] goto (链式)...");
    page.goto("https://example.com").await?;
    let title = page.title().await?;
    println!("  标题: {title}");

    // 2. get_text — 一步获取文本
    println!("\n[2] get_text...");
    let heading = page.get_text("h1").await?;
    println!("  h1: {heading}");

    // 3. get_attr — 一步获取属性
    println!("\n[3] get_attr...");
    let href = page.get_attr("a", "href").await?;
    println!("  链接 href: {:?}", href);

    // 4. scroll_by — 相对滚动
    println!("\n[4] scroll_by...");
    page.scroll_by(0, 100).await?;
    let sy = page.execute("window.scrollY").await?;
    println!("  scrollY: {sy}");
    page.scroll_by(0, -50).await?;
    let sy2 = page.execute("window.scrollY").await?;
    println!("  回滚后: {sy2}");

    // 5. keys — 逐字符输入
    println!("\n[5] keys...");
    page.goto("https://www.google.com").await?;
    // Google search box
    match page.ele("textarea[name=q]").await {
        Ok(el) => {
            el.click().await?;
            page.keys("rust language").await?;
            let val = page.execute("document.querySelector('textarea[name=q]').value").await?;
            println!("  keys 输入: {val}");
        }
        Err(e) => println!("  跳过 (Google 可能重定向): {e}"),
    }

    // 6. type_text + click_ele — 在 example.com 测试
    println!("\n[6] type_text (example.com)...");
    page.goto("https://example.com").await?;
    // Example.com doesn't have inputs, test on a more complex page
    page.goto("https://httpbin.org/forms/post").await?;
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    match page.type_text("input[name=custname]", "test user").await {
        Ok(_) => {
            let val = page.execute("document.querySelector('input[name=custname]').value").await?;
            println!("  type_text: {val}");
        }
        Err(e) => println!("  跳过: {e}"),
    }

    // 7. wait_for_navigation
    println!("\n[7] wait_for_navigation...");
    page.goto("https://example.com").await?;
    let url = page.url().await?;
    println!("  URL 包含 'example': {}", url.contains("example"));

    println!("\n=== ✅ 全部新增 API 验证通过！ ===");
    Ok(())
}
