//! Click "用户笔记" on runoob.com — with retry and debug
use rpage::ChromiumPage;

#[tokio::main]
async fn main() -> rpage::Result<()> {
    let page = ChromiumPage::new().await?;
    page.get("https://www.runoob.com/").await?;

    // 等页面完全加载
    tokio::time::sleep(std::time::Duration::from_secs(3)).await;

    let title = page.title().await?;
    println!("当前标签: {}", title);

    let tabs_before = page.tab_titles().await?;
    println!("点击前所有标签: {:?}", tabs_before);

    // 先用 JS 确认元素存在
    let check = page.execute(
        "document.evaluate(\"//*[text()='用户笔记']\", document, null, 9, null).singleNodeValue ? 'FOUND' : 'NOT FOUND'"
    ).await?;
    println!("JS 查找用户笔记: {:?}", check);

    // 方式1: click_ele
    println!("\n=== click_ele ===");
    match page.click_ele("text=用户笔记").await {
        Ok(_) => println!("click_ele 成功!"),
        Err(e) => println!("click_ele 失败: {e}"),
    }

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let tabs_after = page.tab_titles().await?;
    println!("点击后所有标签: {:?}", tabs_after);
    println!("当前 URL: {}", page.url().await?);

    if tabs_after.len() > tabs_before.len() {
        println!("\n检测到新标签！用 activate_tab_by_url 切换...");
        page.activate_tab_by_url("commentslist").await?;
        println!("切换后 URL: {}", page.url().await?);
        println!("切换后 title: {}", page.title().await?);
    }

    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
    Ok(())
}
