//! rpage 能力全景演示 — 覆盖 12 大类浏览器自动化场景
//!
//! 策略: 尽量使用本地 JS 构建页面，只在必要时访问外部站点，避免网络不稳定
use rpage::prelude::*;

/// 构建本地测试页面
async fn build_local_page(cp: &ChromiumPage) -> Result<()> {
    cp.get("about:blank").await?;
    cp.execute(r#"
        document.title = 'rpage Demo';
        document.body.innerHTML = `
        <div id="container">
          <h1 id="title">测试页面</h1>
          <p class="info">第一段文字</p>
          <p class="info">第二段文字</p>
          <a href="https://example.com" id="link1">Example Link</a>
          <input name="username" type="text" placeholder="用户名">
          <input name="password" type="password">
          <input name="email" type="email">
          <select name="city">
            <option value="bj">北京</option>
            <option value="sh">上海</option>
            <option value="gz">广州</option>
          </select>
          <textarea name="bio"></textarea>
          <button type="button" id="btn-submit">Submit</button>
          <div style="height:2000px;background:#f0f0f0">Scroll Content</div>
        </div>`;
    "#).await?;
    cp.sleep(std::time::Duration::from_millis(300)).await;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    println!("╔══════════════════════════════════════════════╗");
    println!("║     rpage 能力全景演示                        ║");
    println!("║     Rust 浏览器自动化库                       ║");
    println!("╚══════════════════════════════════════════════╝\n");

    // ═══════════════════════════════════════════════
    // 1. 浏览器启动与连接
    // ═══════════════════════════════════════════════
    println!("━━━ 1. 浏览器启动与连接 ━━━");
    println!("  支持方式:");
    println!("    [A] ChromiumPage::connect(url)  — 连接已运行的浏览器");
    println!("    [B] ChromiumPage::new()         — 自动启动 Chrome");
    println!("    [C] ChromiumPage::with_options() — 自定义 headless/proxy/UA");
    println!("    [D] WebPage::new()              — 三合一模式");

    let cp = ChromiumPage::connect("http://127.0.0.1:9222").await?;
    cp.set_viewport(1280, 800).await?;
    println!("  ✅ 已连接 Chrome (connect 模式)\n");

    // ═══════════════════════════════════════════════
    // 2. 导航与页面信息
    // ═══════════════════════════════════════════════
    println!("━━━ 2. 导航与页面信息 ━━━");
    cp.get("https://example.com").await?;
    println!("  get(): {}", cp.url().await?);
    println!("  title(): {}", cp.title().await?);

    cp.goto("https://example.com").await?;
    println!("  goto() 链式: {}", cp.current_url().await?);

    let h1_text = cp.get_text("h1").await?;
    println!("  get_text('h1'): {h1_text}");
    println!("  (back/forward/refresh 参见 showcase.rs)\n");

    // ═══════════════════════════════════════════════
    // 3. 元素定位与交互
    // ═══════════════════════════════════════════════
    println!("━━━ 3. 元素定位与交互 ━━━");
    build_local_page(&cp).await?;

    // CSS 选择器
    let h1 = cp.ele("#title").await?;
    println!("  CSS '#title': '{}'", h1.text());

    // tag 定位
    let all_p = cp.eles("tag:p").await?;
    println!("  tag:p → {} 个", all_p.len());

    // ElementBatch trait
    let texts = all_p.texts();
    println!("  ElementBatch.texts(): {:?}", texts);

    // 链式定位 (parent@@child)
    let inner_p = cp.ele("tag:div@@tag:p").await?;
    println!("  链式 tag:div@@tag:p → '{}'", inner_p.text());

    // 元素属性
    let link = cp.ele("tag:a").await?;
    println!("  tag:a → tag={}, href={}", link.tag(), link.attr("href").unwrap_or_default());
    println!("  #title html(): {}", h1.html());
    println!("  #title is_displayed(): {}", h1.is_displayed());

    // xpath, text:, @attr 等高级定位器在 showcase.rs 中有完整演示
    println!("  (xpath://, text:, @attr 定位器见 showcase.rs)\n");

    // ═══════════════════════════════════════════════
    // 4. 表单操作
    // ═══════════════════════════════════════════════
    println!("━━━ 4. 表单操作 ━━━");
    build_local_page(&cp).await?;

    // fill() — 设置值 (支持中文, 通过 nativeInputValueSetter)
    let user = cp.ele("input[name=username]").await?;
    user.fill("张三").await?;
    println!("  fill('张三'): {}", user.value().await?);

    // input() — 追加输入
    let email = cp.ele("input[name=email]").await?;
    email.input("test@").await?;
    email.input("example.com").await?;
    println!("  input() 追加: {}", email.value().await?);

    // clear() + fill()
    email.clear().await?;
    email.fill("new@mail.com").await?;
    println!("  clear+fill: {}", email.value().await?);

    // type_text — 一步定位+输入
    cp.type_text("input[name=password]", "secret123").await?;
    println!("  type_text: 密码已输入");

    // select() — 下拉选择
    let city = cp.ele("tag:select").await?;
    city.select("sh").await?;
    println!("  select('sh'): {}", city.value().await?);

    // textarea
    let bio = cp.ele("tag:textarea").await?;
    bio.fill("这是 rpage 自动填写的简介。").await?;
    println!("  textarea fill: {}", bio.value().await?);

    // click
    let btn = cp.ele("tag:button").await?;
    btn.click().await?;
    println!("  click(): 已点击 Submit\n");

    // ═══════════════════════════════════════════════
    // 5. 多标签页管理
    // ═══════════════════════════════════════════════
    println!("━━━ 5. 多标签页管理 ━━━");
    let tabs0 = cp.tabs().await?.len();
    println!("  当前标签数: {tabs0}");

    cp.new_tab().await?;
    cp.sleep(std::time::Duration::from_millis(500)).await;
    build_local_page(&cp).await?;
    println!("  new_tab: title={}", cp.title().await?);

    let titles = cp.tab_titles().await?;
    println!("  tab_titles: {} 个", titles.len());

    cp.switch_to_tab(0).await?;
    println!("  switch_to_tab(0): OK");

    let tabs1 = cp.tabs().await?.len();
    if tabs1 > tabs0 {
        // 只关闭新创建的标签
        match cp.close_tab(tabs1 - 1).await {
            Ok(()) => println!("  close_tab: ✅"),
            Err(e) => println!("  close_tab: 跳过 ({})", e.to_string().chars().take(30).collect::<String>()),
        }
    }
    println!();

    // ═══════════════════════════════════════════════
    // 6. Cookie 管理
    // ═══════════════════════════════════════════════
    println!("━━━ 6. Cookie 管理 ━━━");
    // 需要在真实域名上设置 cookie
    match cp.get("https://example.com").await {
        Ok(()) => {
            cp.set_cookie(CookieInfo {
                name: "session_id".into(),
                value: "abc123xyz".into(),
                domain: Some(".example.com".into()),
                path: Some("/".into()),
                secure: false,
                http_only: false,
            }).await?;
            println!("  set_cookie: session_id=abc123xyz");

            let cookies = cp.cookies().await?;
            println!("  cookies(): {} 个", cookies.len());
            for c in &cookies {
                println!("    {} = {}", c.name, c.value);
            }

            cp.delete_cookie("session_id").await?;
            let found = cp.cookies().await?.iter().any(|c| c.name == "session_id");
            println!("  delete_cookie: 已删除 = {}", !found);

            cp.clear_cookies().await?;
            println!("  clear_cookies: 剩余 {} 个", cp.cookies().await?.len());
        }
        Err(e) => println!("  ⚠️ Cookie 演示跳过 (导航超时: {})", e),
    }
    println!();

    // ═══════════════════════════════════════════════
    // 7. 网络监听与请求拦截
    // ═══════════════════════════════════════════════
    println!("━━━ 7. 网络监听与请求拦截 ━━━");

    // 网络监听
    cp.listen_start().await?;
    println!("  listen_start: ✅");
    match cp.get("https://example.com").await {
        Ok(()) => {
            cp.sleep(std::time::Duration::from_secs(2)).await;
            let packets = cp.get_packets("example");
            println!("  get_packets: {} 个请求", packets.len());
            for p in packets.iter().take(3) {
                println!("    {} {} ({:?})", p.method, p.url, p.resource_type);
            }
        }
        Err(_) => println!("  ⚠️ 监听期间导航超时, 跳过"),
    }
    cp.listen_stop().await?;
    println!("  listen_stop: ✅");

    // 请求拦截
    print!("  enable_intercept: ");
    let guard = cp.enable_intercept("*/").await?;
    println!("✅");
    cp.get("https://example.com").await.ok();
    cp.sleep(std::time::Duration::from_secs(2)).await;
    let paused = guard.paused_requests();
    println!("  paused_requests: {} 个", paused.len());
    for req in paused.iter().take(2) {
        println!("    {} {} ({:?})", req.method, req.url, req.resource_type);
        guard.continue_request(req.request_id.as_ref(), None).await?;
    }
    drop(guard);
    println!("  拦截已关闭\n");

    // ═══════════════════════════════════════════════
    // 8. 设备模拟
    // ═══════════════════════════════════════════════
    println!("━━━ 8. 设备模拟 ━━━");
    build_local_page(&cp).await?;

    // 视口
    cp.set_viewport(375, 812).await?;
    let vw = cp.execute("window.innerWidth").await?;
    println!("  set_viewport(375,812): innerWidth={vw}");
    cp.set_viewport(1280, 800).await?;

    // 地理位置
    cp.set_geolocation(31.2304, 121.4737).await?;
    println!("  set_geolocation: 上海 (31.23, 121.47)");

    // 时区
    cp.set_timezone("Asia/Shanghai").await?;
    let tz = cp.execute("Intl.DateTimeFormat().resolvedOptions().timeZone").await?;
    println!("  set_timezone: {tz}");

    // 触摸
    cp.set_touch(true).await?;
    println!("  set_touch(true): ✅");
    cp.set_touch(false).await?;

    // 设备像素比
    cp.set_device_scale(2.0).await?;
    let dpr = cp.execute("window.devicePixelRatio").await?;
    println!("  set_device_scale(2.0): DPR={dpr}");
    cp.set_device_scale(1.0).await?;

    // User-Agent
    cp.set_user_agent("Mozilla/5.0 (iPhone; CPU iPhone OS 16_0 like Mac OS X)").await?;
    let ua = cp.execute("navigator.userAgent").await?;
    let ua_short: String = ua.to_string().chars().take(50).collect();
    println!("  set_user_agent: {ua_short}...");

    // 一键设备模拟
    cp.emulate_device(375, 812, "Mozilla/5.0 (iPhone...)", 3.0, true).await?;
    println!("  emulate_device(iPhone): ✅");

    // 重置
    cp.set_viewport(1280, 800).await?;
    cp.set_device_scale(1.0).await?;
    cp.set_touch(false).await?;
    cp.set_user_agent("").await?;
    println!("  设备状态已重置\n");

    // ═══════════════════════════════════════════════
    // 9. 截图与PDF
    // ═══════════════════════════════════════════════
    println!("━━━ 9. 截图与PDF ━━━");
    build_local_page(&cp).await?;

    // 整页截图 → 文件
    cp.screenshot("demo_screenshot.png").await?;
    let sz = std::fs::metadata("demo_screenshot.png")?.len();
    println!("  screenshot: {sz} bytes → demo_screenshot.png");

    // 截图 → 内存
    let bytes = cp.screenshot_bytes().await?;
    println!("  screenshot_bytes: {} bytes in memory", bytes.len());

    // PDF
    cp.pdf("demo_page.pdf").await?;
    let pdf_sz = std::fs::metadata("demo_page.pdf")?.len();
    println!("  pdf: {pdf_sz} bytes → demo_page.pdf");

    // PDF 自定义纸张/页眉页脚
    let opts = rpage::PdfOptions::builder()
        .paper_width(8.5)
        .paper_height(11.0)
        .margin_top(0.5)
        .margin_bottom(0.5)
        .print_background(true)
        .display_header_footer(true)
        .header_template("<span style='font-size:8pt'>rpage Demo</span>")
        .footer_template("<span style='font-size:8pt'>Page <span class='pageNumber'></span>/<span class='totalPages'></span></span>")
        .build();
    cp.pdf_to_file("demo_styled.pdf", opts).await?;
    let pdf_sz2 = std::fs::metadata("demo_styled.pdf")?.len();
    println!("  pdf_to_file(自定义): {pdf_sz2} bytes");

    // 清理临时文件
    for f in &["demo_screenshot.png", "demo_page.pdf", "demo_styled.pdf"] {
        std::fs::remove_file(f).ok();
    }
    println!("  临时文件已清理\n");

    // ═══════════════════════════════════════════════
    // 10. JavaScript 执行与注入
    // ═══════════════════════════════════════════════
    println!("━━━ 10. JavaScript 执行与注入 ━━━");
    build_local_page(&cp).await?;

    // execute — 同步 JS 表达式
    let count = cp.execute("document.querySelectorAll('p').length").await?;
    println!("  execute: {} 个 <p>", count);

    // run_js_with_args — 带参数 (需要 execution context)
    // 使用 execute + 内联参数替代
    let sum = cp.execute("(40 + 2).toString()").await?;
    println!("  execute(40+2): {sum}");

    // run_async_js — 异步 JS
    let async_r = cp.run_async_js(
        "await new Promise(r => setTimeout(() => r('async done!'), 100))"
    ).await?;
    println!("  run_async_js: {async_r}");

    // CSS 注入 + 移除
    let css_id = cp.inject_css("h1 { color: red !important; text-decoration: underline; }").await?;
    println!("  inject_css: h1 变红 (id={css_id})");
    cp.remove_css(&css_id).await?;
    println!("  remove_css: 已恢复");

    // Init Script (每个新页面自动执行)
    cp.add_init_script("demo_marker", "window.__rpage_demo = 'injected!'").await?;
    build_local_page(&cp).await?;
    let marker = cp.execute("window.__rpage_demo").await?;
    println!("  add_init_script: __rpage_demo = {marker}");
    cp.remove_init_script("demo_marker").await?;
    println!("  list_init_scripts: {:?}", cp.list_init_scripts());
    println!();

    // ═══════════════════════════════════════════════
    // 11. 窗口管理与 ActionChain
    // ═══════════════════════════════════════════════
    println!("━━━ 11. 窗口管理与 ActionChain ━━━");

    // 窗口位置/大小
    let bounds = cp.get_window_bounds().await?;
    println!("  窗口: left={}, top={}, width={}, height={}",
        bounds.0, bounds.1, bounds.2, bounds.3);

    cp.set_window_size(1024, 768).await?;
    let _b2 = cp.get_window_bounds().await?;
    println!("  set_window_size(1024,768): ✅");

    cp.maximize().await?;
    println!("  maximize(): ✅");
    cp.set_window_size(1280, 800).await?;

    // ActionChain — 复杂交互链
    build_local_page(&cp).await?;
    cp.actions()
        .move_to(200.0, 100.0)
        .pause(std::time::Duration::from_millis(50))
        .click_at(200.0, 100.0)
        .key_down("Shift")
        .press("a")
        .key_up("Shift")
        .perform().await?;
    println!("  ActionChain: move→click→Shift+a→释放 ✅");

    cp.press("Escape").await?;
    println!("  press('Escape'): ✅");

    cp.keys("Hello rpage").await?;
    println!("  keys('Hello rpage'): ✅\n");

    // ═══════════════════════════════════════════════
    // 12. 性能指标与控制台监控
    // ═══════════════════════════════════════════════
    println!("━━━ 12. 性能指标与控制台监控 ━━━");

    // 性能指标
    let metrics = cp.performance_metrics().await?;
    println!("  performance_metrics: {} 项", metrics.len());
    for (name, val) in metrics.iter().take(5) {
        println!("    {name}: {val:.1}");
    }

    // 页面计时
    let timing = cp.page_timing().await?;
    println!("  page_timing: {} 项", timing.len());
    if let Some(dom) = timing.get("domComplete") {
        println!("    domComplete: {dom:.0}ms");
    }

    // 控制台日志
    cp.execute("console.log('rpage info')").await?;
    cp.execute("console.warn('rpage warning')").await?;
    cp.execute("console.error('rpage error')").await?;
    cp.sleep(std::time::Duration::from_millis(500)).await;

    let logs = cp.console_log();
    println!("  console_log: {} 条", logs.len());
    for entry in &logs {
        println!("    [{:?}] {}", entry.level, entry.text);
    }
    cp.clear_console();
    println!("  clear_console: 已清空\n");

    // ═══════════════════════════════════════════════
    // 附加: 更多实用能力
    // ═══════════════════════════════════════════════
    println!("━━━ 附加能力 ━━━");
    build_local_page(&cp).await?;

    // 智能等待
    cp.wait_ele("h1", 5).await?;
    println!("  wait_ele('h1', 5s): ✅");
    cp.wait_title_contains("rpage", 5).await?;
    println!("  wait_title_contains('rpage'): ✅");

    // 资源提取
    let links = cp.links().await?;
    println!("  links(): {} 个", links.len());
    for l in links.iter().take(3) {
        println!("    {}", l);
    }

    // DOM 快照
    let snap = cp.dom_snapshot().await?;
    let snap_json = serde_json::to_string(&snap)?;
    println!("  dom_snapshot: {} bytes JSON", snap_json.len());

    // 剪贴板 (需要安全上下文, about:blank 可能不支持)
    match cp.clipboard_write("Hello from rpage!").await {
        Ok(()) => {
            let clip = cp.clipboard_read().await?;
            println!("  clipboard: 写入并读回 '{}'", clip);
        }
        Err(_) => println!("  clipboard: ⚠️ 需要 HTTPS 上下文, 跳过"),
    }

    // 音频控制
    cp.mute().await?;
    println!("  mute: ✅");
    cp.unmute().await?;
    println!("  unmute: ✅");

    // 缓存
    cp.clear_cache().await?;
    println!("  clear_cache: ✅");

    // 图片拦截
    cp.disable_images().await?;
    println!("  disable_images: ✅ (下次加载生效)");
    cp.set_blocked_urls(&[]).await?;

    // 离线模式
    cp.set_offline(true).await?;
    let offline_result = cp.get("https://example.com").await;
    println!("  set_offline(true): get → {}", if offline_result.is_err() { "失败(预期)" } else { "成功" });
    match cp.set_offline(false).await {
        Ok(()) => println!("  set_offline(false): ✅"),
        Err(_) => {
            // 离线可能导致 CDP 也超时, 用 navigate 恢复
            cp.sleep(std::time::Duration::from_secs(1)).await;
            cp.set_offline(false).await.ok();
            println!("  set_offline(false): 已恢复");
        }
    }

    // 滚动
    build_local_page(&cp).await?; // 离线模式后重建页面
    cp.scroll_to_bottom().await?;
    let sy = cp.execute("window.scrollY").await?;
    println!("  scroll_to_bottom: scrollY={sy}");
    cp.scroll_to_top().await?;
    cp.smooth_scroll(0, 200, 300).await?;
    println!("  smooth_scroll(0,200): ✅");

    // 元素等待方法
    let h1_el = cp.ele("h1").await?;
    h1_el.wait_for_visible().await?;
    println!("  element.wait_for_visible: ✅");
    h1_el.wait_for_clickable().await?;
    println!("  element.wait_for_clickable: ✅");

    // 页面搜索
    let found = cp.find_text("测试").await?;
    println!("  find_text('测试'): {found}");

    // exists / count
    let has = cp.exists("h1").await;
    println!("  exists('h1'): {has}");
    let cnt = cp.count("p").await;
    println!("  count('p'): {cnt}");

    println!("\n╔══════════════════════════════════════════════╗");
    println!("║     ✅ 全部能力演示完成!                      ║");
    println!("╚══════════════════════════════════════════════╝");
    println!("\n覆盖能力清单:");
    println!("  1.  浏览器连接 (connect/new/with_options/WebPage)");
    println!("  2.  导航 (get/goto/back/forward/refresh)");
    println!("  3.  元素定位 (CSS/tag/链式/批量)");
    println!("  4.  表单操作 (fill/input/clear/type_text/select/click)");
    println!("  5.  多标签页 (new_tab/switch_to_tab/close_tab)");
    println!("  6.  Cookie (set/get/delete/clear)");
    println!("  7.  网络监听+拦截 (listen/intercept)");
    println!("  8.  设备模拟 (viewport/geo/timezone/touch/UA/DPR/emulate_device)");
    println!("  9.  截图+PDF (screenshot/screenshot_bytes/pdf/pdf_to_file)");
    println!("  10. JS执行+注入 (execute/run_async_js/inject_css/add_init_script)");
    println!("  11. 窗口管理+ActionChain");
    println!("  12. 性能+控制台 (performance_metrics/page_timing/console_log)");
    println!("  +.  智能等待/DOM快照/剪贴板/音频/离线/缓存/滚动/资源提取");

    Ok(())
}
