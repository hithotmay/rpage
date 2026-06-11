//! rpage iter10 新功能演示 — 网络控制、设备模拟、页面分析、请求拦截
use rpage::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    println!("=== rpage iter10 新功能演示 ===\n");

    // 连接 Chrome
    println!("[1] 连接 Chrome...");
    let page = ChromiumPage::connect("http://127.0.0.1:9222").await?;
    page.set_viewport(1280, 800).await?;
    println!("  ✅ 已连接\n");

    // ── 2. ele_count: 统计元素数量 ──
    println!("[2] ele_count — 统计页面元素...");
    page.get("https://www.baidu.com").await?;
    let link_count = page.ele_count("tag:a").await?;
    println!("  页面链接数: {link_count}");
    let input_count = page.ele_count("tag:input").await?;
    println!("  页面输入框数: {input_count}\n");

    // ── 3. links / images: 页面资源提取 ──
    println!("[3] links / images — 提取页面资源...");
    let links = page.links().await?;
    println!("  链接({}): {:?}", links.len(), &links[..links.len().min(3)]);
    let images = page.images().await?;
    println!("  图片({}): {:?}", images.len(), &images[..images.len().min(3)]);

    // ── 4. get_and_wait: 导航并等待完成 ──
    println!("\n[4] get_and_wait — 导航并等待...");
    page.get_and_wait("https://httpbin.org/forms/post", 10).await?;
    let title = page.title().await?;
    println!("  页面标题: {title}");

    // ── 5. smooth_scroll: 平滑滚动 ──
    println!("\n[5] smooth_scroll — 平滑滚动...");
    page.execute("document.body.innerHTML = '<div style=\"height:3000px\">Scroll Test</div>'").await?;
    page.smooth_scroll(0, 1500, 500).await?;
    println!("  ✅ 平滑滚动到 1500px");

    // ── 6. set_device_scale: 设备像素比 ──
    println!("\n[6] set_device_scale — 设备像素比...");
    page.set_device_scale(2.0).await?;
    let dpr = page.execute("window.devicePixelRatio").await?;
    println!("  devicePixelRatio: {dpr}");
    page.set_device_scale(1.0).await?; // 恢复
    println!("  ✅ 已恢复为 1.0");

    // ── 7. set_touch: 触摸模拟 ──
    println!("\n[7] set_touch — 触摸模拟...");
    page.set_touch(true).await?;
    println!("  ✅ 触摸模式开启");
    page.set_touch(false).await?;
    println!("  ✅ 触摸模式关闭");

    // ── 8. clear_cache: 清除缓存 ──
    println!("\n[8] clear_cache — 清除浏览器缓存...");
    page.clear_cache().await?;
    println!("  ✅ 缓存已清除");

    // ── 9. set_offline: 离线模拟 ──
    println!("\n[9] set_offline — 离线模式...");
    page.set_offline(true).await?;
    println!("  ✅ 离线模式开启");
    let result = page.get("https://httpbin.org/get").await;
    println!("  离线导航结果: {}", if result.is_err() { "失败(预期)" } else { "成功(意外)" });
    page.set_offline(false).await?;
    println!("  ✅ 离线模式已关闭");

    // ── 10. set_blocked_urls: URL 拦截 (URLPattern 格式) ──
    println!("\n[10] set_blocked_urls — 拦截指定 URL...");
    page.set_blocked_urls(&["*://*/*/*.css"]).await?;
    println!("  ✅ 已拦截 CSS 请求");
    page.get("https://www.baidu.com").await?;
    println!("  ✅ 百度已加载（CSS 被拦截）");
    page.set_blocked_urls(&[]).await?;

    // ── 11. disable_images: 禁用所有图片 ──
    println!("\n[11] disable_images — 一键禁图...");
    page.disable_images().await?;
    page.get("https://www.baidu.com").await?;
    println!("  ✅ 页面已加载（所有图片被禁用）");
    page.set_blocked_urls(&[]).await?;

    // ── 12. set_location_and_reload: 地理位置 ──
    println!("\n[12] set_location_and_reload — 设置地理位置...");
    page.set_location_and_reload(39.9042, 116.4074).await?;
    println!("  ✅ 已设置为北京坐标 (39.9042, 116.4074)");

    // ── 13. listen_start / listen_stop / get_packets: 网络监听 ──
    println!("\n[13] listen_start / listen_stop — 网络监听...");
    page.listen_start().await?;
    println!("  ✅ 监听已开启");
    page.get("https://httpbin.org/get").await?;
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let packets = page.get_packets("httpbin");
    println!("  捕获到 {} 个 httpbin 请求", packets.len());
    for pkt in &packets {
        println!("    {} {} ({})", pkt.method, pkt.url, pkt.resource_type);
    }
    let responses = page.get_responses("httpbin");
    println!("  捕获到 {} 个 httpbin 响应", responses.len());
    for res in &responses {
        println!("    {} (status {})", res.url, res.status);
    }
    page.listen_stop().await?;
    println!("  ✅ 监听已关闭");

    // ── 14. enable_intercept: 请求拦截与放行 ──
    println!("\n[14] enable_intercept — 请求拦截...");
    let guard = page.enable_intercept("*/ip").await?;
    println!("  ✅ 拦截已开启 (匹配 */ip)");
    // 后台触发导航
    page.get("https://httpbin.org/ip").await.ok();
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let paused = guard.paused_requests();
    println!("  暂停的请求数: {}", paused.len());
    for req in &paused {
        println!("    {} {} ({})", req.method, req.url, req.resource_type);
        guard.continue_request(req.request_id.as_ref(), None).await?;
    }
    drop(guard);
    println!("  ✅ 拦截已关闭");

    // ── 15. set_download_file_name ──
    println!("\n[15] set_download_file_name...");
    page.set_download_file_name("test_file.zip").await?;
    println!("  ✅ 已设置（占位实现）");

    // ── 16. Tab 管理: get_tab_by_title / get_tab_by_url / wait_new_tab ──
    println!("\n[16] Tab 管理...");
    let tabs = page.tabs().await?;
    println!("  当前标签数: {}", tabs.len());
    match page.get_tab_by_title("百度").await {
        Ok(idx) => println!("  找到百度标签: index={idx}"),
        Err(_) => println!("  未找到百度标签（当前页非百度）"),
    }
    match page.get_tab_by_url("httpbin").await {
        Ok(idx) => println!("  找到 httpbin 标签: index={idx}"),
        Err(_) => println!("  未找到 httpbin 标签"),
    }
    println!("  wait_new_tab 1s 超时测试...");
    match page.wait_new_tab(1).await {
        Ok(()) => println!("  检测到新标签"),
        Err(_) => println!("  超时（预期）"),
    }

    println!("\n=== ✅ iter10 全部 16 项演示完成！ ===");
    Ok(())
}
