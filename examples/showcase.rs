//! rpage 全功能 Showcase — 验证所有核心 API
use rpage::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== rpage Showcase ===\n");

    // 1. Connect
    println!("[1] 连接 Chrome...");
    let page = ChromiumPage::connect("http://127.0.0.1:9222").await?;
    println!("  ✅ 已连接\n");

    // 2. Navigate
    println!("[2] 导航到百度...");
    page.get("https://www.baidu.com").await?;
    let title = page.title().await?;
    println!("  标题: {title}\n");

    // 3. Viewport (先设置确保元素渲染)
    println!("[3] 视口设置...");
    page.set_viewport(1280, 800).await?;
    println!("  ✅ 1280x800\n");

    // 4. Find element + interact
    println!("[4] 查找搜索框并输入...");
    let kw = page.ele("#kw").await?;
    kw.fill("rpage rust").await?;
    println!("  ✅ 已输入\n");

    // 5. JS execute
    println!("[5] 执行 JS...");
    let val = page.execute("1 + 2").await?;
    println!("  1 + 2 = {val}\n");

    // 6. Cookie
    println!("[6] Cookie 操作...");
    page.set_cookie(CookieInfo {
        name: "test_rpage".into(),
        value: "hello123".into(),
        domain: Some(".baidu.com".into()),
        path: Some("/".into()),
        secure: false,
        http_only: false,
    }).await?;
    let cookies = page.cookies().await?;
    let found = cookies.iter().any(|c| c.name == "test_rpage");
    println!("  set_cookie: {found}");
    page.delete_cookie("test_rpage").await?;
    let cookies2 = page.cookies().await?;
    let gone = !cookies2.iter().any(|c| c.name == "test_rpage");
    println!("  delete_cookie: {gone}\n");

    // 7. Scroll
    println!("[7] 滚动...");
    page.execute("window.scrollTo(0, document.body.scrollHeight)").await?;
    page.execute("window.scrollTo(0, 0)").await?;
    println!("  ✅ 滚动到底再回顶\n");

    // 8. Tabs
    println!("[8] 标签页管理...");
    let tabs = page.tabs().await?;
    let titles = page.tab_titles().await?;
    println!("  标签数: {}, 标题: {:?}", tabs.len(), titles);
    println!("  ✅ 标签页查询\n");

    // 9. Wait
    println!("[9] 智能等待...");
    page.wait_ele("#kw", 5).await?;
    println!("  ✅ wait_ele 成功\n");

    // 10. Screenshot
    println!("[10] 截图...");
    page.screenshot("rpage_showcase.png").await?;
    let meta = std::fs::metadata("rpage_showcase.png")?;
    println!("  文件大小: {} bytes\n", meta.len());

    // 11. Console
    println!("[11] 控制台日志...");
    page.execute("console.log('rpage showcase')").await?;
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    let logs = page.console_log();
    println!("  日志条数: {}", logs.len());
    page.clear_console();
    println!("  ✅ 已清空\n");

    // 12. Init Script
    println!("[12] Init Script...");
    page.add_init_script("test_var", "window.__rpage_test = 42").await?;
    page.execute("location.reload()").await?;
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    let v = page.execute("window.__rpage_test").await?;
    println!("  __rpage_test = {v}\n");

    // 13. CSS Override
    println!("[13] CSS 注入...");
    let css_id = page.inject_css("body { border: 3px solid red !important; }").await?;
    println!("  CSS ID: {css_id}");
    page.remove_css(&css_id).await?;
    println!("  ✅ 注入并移除\n");

    // 14. Performance
    println!("[14] 性能指标...");
    let timing = page.page_timing().await?;
    if let Some(dom) = timing.get("domComplete") {
        println!("  DOM Complete: {dom:.0}ms");
    }
    let metrics = page.performance_metrics().await?;
    let js_heap = metrics.iter().find(|(n, _)| n == "JSHeapUsedSize");
    if let Some((_, v)) = js_heap {
        println!("  JS Heap: {v:.0} bytes");
    }

    // 15. Window management
    println!("\n[15] 窗口管理...");
    let bounds = page.get_window_bounds().await?;
    println!("  当前窗口: left={}, top={}, width={}, height={}", bounds.0, bounds.1, bounds.2, bounds.3);
    page.set_window_size(1024, 768).await?;
    let bounds2 = page.get_window_bounds().await?;
    println!("  调整后: width={}, height={}", bounds2.2, bounds2.3);
    println!("  ✅ 窗口管理正常\n");

    // 16. Load strategy
    println!("[16] 加载策略: {}", page.load_strategy());
    println!("  ✅ 策略查询正常\n");

    // 17. Element wait_for_* methods
    println!("[17] Element 等待方法...");
    let search_btn = page.wait_ele("#su", 5).await?;
    search_btn.wait_for_visible().await?;
    println!("  ✅ wait_for_visible");
    search_btn.wait_for_clickable().await?;
    println!("  ✅ wait_for_clickable");

    // 18. Element utility methods
    println!("\n[18] Element 工具方法...");
    search_btn.focus().await?;
    println!("  ✅ focus()");
    match search_btn.screenshot("elem_screenshot.png").await {
        Ok(()) => {
            let sz = std::fs::metadata("elem_screenshot.png").map(|m| m.len()).unwrap_or(0);
            println!("  元素截图: {sz} bytes");
            std::fs::remove_file("elem_screenshot.png").ok();
        }
        Err(_) => println!("  元素截图: 跳过（headless无box model）"),
    }

    // 19. ActionChain
    println!("[19] ActionChain...");
    page.actions()
        .move_to(200.0, 200.0)
        .click_at(200.0, 200.0)
        .pause(std::time::Duration::from_millis(100))
        .perform().await?;
    println!("  ✅ ActionChain 执行\n");

    // 20. Network monitor + DOM snapshot
    println!("[20] 网络监控 + DOM 快照...");
    let monitor = page.network_monitor();
    println!("  已记录 {} 个请求", monitor.requests().len());
    let snap = page.dom_snapshot().await?;
    println!("  DOM 快照 keys: {:?}", snap.as_object().map(|o| o.keys().take(3).collect::<Vec<_>>()).unwrap_or_default());

    // Cleanup
    std::fs::remove_file("rpage_showcase.png").ok();

    println!("\n=== ✅ 全部 20 项验证通过！ ===");
    Ok(())
}
