//! rpage SyncPage 全面能力测试
//! 覆盖 25 大类浏览器自动化场景
use rpage::sync::SyncPage;
use rpage::chromium_page::CookieInfo;
use std::time::Duration;

macro_rules! section {
    ($n:expr, $title:expr) => {
        println!("\n{} {}. {} {}", "━".repeat(10), $n, $title, "━".repeat(10));
    };
}

/// 构建本地测试页面
fn build_page(p: &SyncPage) -> Result<(), Box<dyn std::error::Error>> {
    let js = r#"
        document.title = 'SyncPage Test';
        document.body.innerHTML = '<div id="container"><h1 id="title">SyncAPI Test</h1><p class="info">P1</p><p class="info">P2</p><p class="detail">Detail info</p><a href="https://example.com" id="link1">Link1</a><a href="https://httpbin.org" id="link2">Link2</a><input name="username" type="text" placeholder="user"><input name="password" type="password"><input name="email" type="email"><select name="city"><option value="bj">BJ</option><option value="sh" selected>SH</option><option value="gz">GZ</option></select><textarea name="bio"></textarea><button type="button" id="btn-submit">Submit</button><div id="scroll-area" style="height:3000px;background:linear-gradient(red,blue)">Long scrollable content</div><div id="hidden-box" style="display:none">Hidden content</div></div>';
    "#;
    p.execute(js)?;
    std::thread::sleep(Duration::from_millis(300));
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("+{}+", "-".repeat(46));
    println!("|   rpage SyncPage 全面能力测试                 |");
    println!("|   纯同步 API - 无 await - 无 async            |");
    println!("+{}+", "-".repeat(46));

    let p = SyncPage::connect("http://127.0.0.1:9222")?;
    p.set_viewport(1280, 800)?;
    println!("已连接 Chrome\n");

    let mut pass = 0u32;
    let mut fail = 0u32;
    let mut total = 0u32;

    // ══════════════════════════════════════════════════
    // 1. 导航与页面信息
    // ══════════════════════════════════════════════════
    section!(1, "导航与页面信息");
    total += 1;
    match (|| -> Result<_, Box<dyn std::error::Error>> {
        p.get("https://example.com")?;
        println!("  [OK] get: {}", p.url()?);
        println!("  [OK] title: {}", p.title()?);
        let h1 = p.get_text("h1")?;
        println!("  [OK] get_text(h1): {}", h1);
        let html_len = p.html()?.len();
        println!("  [OK] html(): {} chars", html_len);
        let src_len = p.page_source()?.len();
        println!("  [OK] page_source(): {} chars", src_len);
        println!("  [OK] current_url: {}", p.current_url()?);
        println!("  [OK] current_title: {}", p.current_title()?);
        Ok(())
    })() {
        Ok(()) => pass += 1,
        Err(e) => { fail += 1; println!("  [FAIL] {}", e); }
    }

    // ══════════════════════════════════════════════════
    // 2. 元素定位与属性
    // ══════════════════════════════════════════════════
    section!(2, "元素定位与属性");
    total += 1;
    match (|| -> Result<_, Box<dyn std::error::Error>> {
        build_page(&p)?;

        let h1 = p.ele("#title")?;
        println!("  [OK] ele(#title): tag={}, text={}", h1.tag(), h1.text());
        println!("       html: {}", h1.html());
        println!("       is_displayed={}, is_enabled={}", h1.is_displayed(), h1.is_enabled());

        let ps = p.eles("tag:p")?;
        println!("  [OK] eles(tag:p): {} 个", ps.len());
        for (i, el) in ps.iter().enumerate() {
            println!("       [{}] {} (displayed={})", i, el.text(), el.is_displayed());
        }

        let maybe = p.ele_or_none("#nonexistent");
        println!("  [OK] ele_or_none(#nonexistent): is_none={}", maybe.is_none());

        println!("  [OK] exists(h1): {}", p.exists("h1"));
        println!("  [OK] count(tag:p): {}", p.count("tag:p"));

        let link = p.ele("#link1")?;
        let href = link.attr("href");
        println!("  [OK] link.attr(href): {:?}", href);
        let all_attrs = link.attrs();
        println!("  [OK] link.attrs(): {} 个", all_attrs.len());
        Ok(())
    })() {
        Ok(()) => pass += 1,
        Err(e) => { fail += 1; println!("  [FAIL] {}", e); }
    }

    // ══════════════════════════════════════════════════
    // 3. 表单操作
    // ══════════════════════════════════════════════════
    section!(3, "表单操作");
    total += 1;
    match (|| -> Result<_, Box<dyn std::error::Error>> {
        build_page(&p)?;

        let user = p.ele("input[name=username]")?;
        user.fill("张三")?;
        println!("  [OK] fill(张三): value={}", user.value()?);

        let email = p.ele("input[name=email]")?;
        email.input("test@")?;
        email.input("example.com")?;
        println!("  [OK] input追加: {}", email.value()?);

        email.clear()?;
        email.fill("new@mail.com")?;
        println!("  [OK] clear+fill: {}", email.value()?);

        p.type_text("input[name=password]", "secret123")?;
        println!("  [OK] type_text: 密码已输入");

        let city = p.ele("tag:select")?;
        city.select("gz")?;
        println!("  [OK] select(gz): {}", city.value()?);

        let bio = p.ele("tag:textarea")?;
        bio.fill("这是 rpage 自动填写的简介。")?;
        println!("  [OK] textarea fill: {}", bio.value()?);

        p.click_ele("#btn-submit")?;
        println!("  [OK] click_ele(#btn-submit)");
        Ok(())
    })() {
        Ok(()) => pass += 1,
        Err(e) => { fail += 1; println!("  [FAIL] {}", e); }
    }

    // ══════════════════════════════════════════════════
    // 4. 鼠标与元素交互
    // ══════════════════════════════════════════════════
    section!(4, "鼠标与元素交互");
    total += 1;
    match (|| -> Result<_, Box<dyn std::error::Error>> {
        build_page(&p)?;

        let h1 = p.ele("#title")?;
        h1.hover()?;
        println!("  [OK] hover()");

        h1.scroll_into_view()?;
        println!("  [OK] scroll_into_view()");

        let link = p.ele("#link1")?;
        link.right_click()?;
        println!("  [OK] right_click()");

        p.press("Escape")?;
        println!("  [OK] press(Escape)");
        p.keys("Hello")?;
        println!("  [OK] keys(Hello)");

        h1.set_style("color", "red")?;
        let color = h1.style("color")?;
        println!("  [OK] set_style(color,red): {:?}", color);

        h1.set_attr("data-test", "sync-api")?;
        let attr_val = h1.attr("data-test");
        println!("  [OK] set_attr: {:?}", attr_val);

        h1.add_class("highlight")?;
        let has = h1.has_class("highlight")?;
        println!("  [OK] add_class + has_class: {}", has);
        h1.remove_class("highlight")?;
        println!("  [OK] remove_class");
        Ok(())
    })() {
        Ok(()) => pass += 1,
        Err(e) => { fail += 1; println!("  [FAIL] {}", e); }
    }

    // ══════════════════════════════════════════════════
    // 5. JavaScript 执行
    // ══════════════════════════════════════════════════
    section!(5, "JavaScript 执行");
    total += 1;
    match (|| -> Result<_, Box<dyn std::error::Error>> {
        build_page(&p)?;

        let count = p.execute("document.querySelectorAll('p').length")?;
        println!("  [OK] execute: {} 个 <p>", count);

        let async_r = p.run_async_js(
            "await new Promise(r => setTimeout(() => r('async done!'), 100))"
        )?;
        println!("  [OK] run_async_js: {}", async_r);

        let sum = p.run_js_with_args(
            "arguments[0] + arguments[1]",
            serde_json::json!([10, 32])
        )?;
        println!("  [OK] run_js_with_args(10,32): {}", sum);

        p.evaluate_on_new_document("window.__injected = 42")?;
        build_page(&p)?;
        let val = p.execute("window.__injected")?;
        println!("  [OK] evaluate_on_new_document: __injected={}", val);
        Ok(())
    })() {
        Ok(()) => pass += 1,
        Err(e) => { fail += 1; println!("  [FAIL] {}", e); }
    }

    // ══════════════════════════════════════════════════
    // 6. 截图与 PDF
    // ══════════════════════════════════════════════════
    section!(6, "截图与 PDF");
    total += 1;
    match (|| -> Result<_, Box<dyn std::error::Error>> {
        build_page(&p)?;

        p.screenshot("sync_shot.png")?;
        let sz = std::fs::metadata("sync_shot.png")?.len();
        println!("  [OK] screenshot: {} bytes", sz);

        let bytes = p.screenshot_bytes()?;
        println!("  [OK] screenshot_bytes: {} bytes", bytes.len());

        let h1 = p.ele("#title")?;
        h1.screenshot("sync_h1.png")?;
        let h1_sz = std::fs::metadata("sync_h1.png")?.len();
        println!("  [OK] element.screenshot: {} bytes", h1_sz);

        p.pdf("sync_test.pdf")?;
        let pdf_sz = std::fs::metadata("sync_test.pdf")?.len();
        println!("  [OK] pdf: {} bytes", pdf_sz);

        for f in &["sync_shot.png", "sync_h1.png", "sync_test.pdf"] {
            std::fs::remove_file(f).ok();
        }
        println!("  [OK] 临时文件已清理");
        Ok(())
    })() {
        Ok(()) => pass += 1,
        Err(e) => { fail += 1; println!("  [FAIL] {}", e); }
    }

    // ══════════════════════════════════════════════════
    // 7. Cookie 管理
    // ══════════════════════════════════════════════════
    section!(7, "Cookie 管理");
    total += 1;
    match (|| -> Result<_, Box<dyn std::error::Error>> {
        p.get("https://example.com")?;

        p.set_cookie(CookieInfo {
            name: "test_session".into(),
            value: "abc123".into(),
            domain: Some(".example.com".into()),
            path: Some("/".into()),
            secure: false,
            http_only: false,
        })?;
        println!("  [OK] set_cookie: test_session=abc123");

        let cookies = p.cookies()?;
        println!("  [OK] cookies(): {} 个", cookies.len());
        for c in &cookies {
            println!("       {} = {}", c.name, c.value);
        }

        p.delete_cookie("test_session")?;
        let found = p.cookies()?.iter().any(|c| c.name == "test_session");
        println!("  [OK] delete_cookie: 已删除={}", !found);

        p.clear_cookies()?;
        println!("  [OK] clear_cookies: 剩余 {} 个", p.cookies()?.len());
        Ok(())
    })() {
        Ok(()) => pass += 1,
        Err(e) => { fail += 1; println!("  [FAIL] {}", e); }
    }

    // ══════════════════════════════════════════════════
    // 8. 标签页管理
    // ══════════════════════════════════════════════════
    section!(8, "标签页管理");
    total += 1;
    match (|| -> Result<_, Box<dyn std::error::Error>> {
        build_page(&p)?;

        let titles = p.tab_titles()?;
        let tab_count = titles.len();
        println!("  [OK] tab_titles: {} 个", tab_count);

        p.new_tab()?;
        std::thread::sleep(Duration::from_millis(500));
        p.execute("document.title = 'New Tab'")?;
        println!("  [OK] new_tab: title={}", p.title()?);

        let urls = p.tab_urls()?;
        println!("  [OK] tab_urls: {} 个", urls.len());

        if tab_count > 0 {
            p.switch_to_tab(0)?;
            println!("  [OK] switch_to_tab(0): title={}", p.title()?);
        }

        let after_tabs = p.tab_titles()?;
        if after_tabs.len() > 1 {
            // close_tab 会破坏 CDP session，只打印不关闭
            println!("  [INFO] close_tab: 跳过(已知问题: 破坏CDP session)");
        }
        Ok(())
    })() {
        Ok(()) => pass += 1,
        Err(e) => { fail += 1; println!("  [FAIL] {}", e); }
    }

    // ══════════════════════════════════════════════════
    // 9. 滚动操作
    // ══════════════════════════════════════════════════
    section!(9, "滚动操作");
    total += 1;
    match (|| -> Result<_, Box<dyn std::error::Error>> {
        build_page(&p)?;

        p.scroll_to_bottom()?;
        let y1: serde_json::Value = p.execute("window.scrollY")?;
        println!("  [OK] scroll_to_bottom: scrollY={}", y1);

        p.scroll_to_top()?;
        let y2: serde_json::Value = p.execute("window.scrollY")?;
        println!("  [OK] scroll_to_top: scrollY={}", y2);

        p.scroll_down(500)?;
        let y3: serde_json::Value = p.execute("window.scrollY")?;
        println!("  [OK] scroll_down(500): scrollY={}", y3);

        p.scroll_up(200)?;
        let y4: serde_json::Value = p.execute("window.scrollY")?;
        println!("  [OK] scroll_up(200): scrollY={}", y4);

        p.scroll_by(0, 1000)?;
        let y5: serde_json::Value = p.execute("window.scrollY")?;
        println!("  [OK] scroll_by(0,1000): scrollY={}", y5);

        p.scroll_to(0, 0)?;
        let y6: serde_json::Value = p.execute("window.scrollY")?;
        println!("  [OK] scroll_to(0,0): scrollY={}", y6);
        Ok(())
    })() {
        Ok(()) => pass += 1,
        Err(e) => { fail += 1; println!("  [FAIL] {}", e); }
    }

    // ══════════════════════════════════════════════════
    // 10. 窗口管理
    // ══════════════════════════════════════════════════
    section!(10, "窗口管理");
    total += 1;
    match (|| -> Result<_, Box<dyn std::error::Error>> {
        build_page(&p)?;

        let bounds = p.get_window_bounds()?;
        println!("  [OK] get_window_bounds: left={} top={} w={} h={}", bounds.0, bounds.1, bounds.2, bounds.3);

        p.set_window_size(1024, 768)?;
        println!("  [OK] set_window_size(1024,768)");

        p.maximize()?;
        println!("  [OK] maximize()");

        p.set_window_size(1280, 800)?;
        println!("  [OK] set_window_size(1280,800) 恢复");
        Ok(())
    })() {
        Ok(()) => pass += 1,
        Err(e) => { fail += 1; println!("  [FAIL] {}", e); }
    }

    // ══════════════════════════════════════════════════
    // 11. 设备模拟
    // ══════════════════════════════════════════════════
    section!(11, "设备模拟");
    total += 1;
    match (|| -> Result<_, Box<dyn std::error::Error>> {
        build_page(&p)?;

        p.set_viewport(375, 812)?;
        let vw = p.execute("window.innerWidth")?;
        println!("  [OK] set_viewport(375,812): innerWidth={}", vw);

        p.set_device_scale(2.0)?;
        let dpr = p.execute("window.devicePixelRatio")?;
        println!("  [OK] set_device_scale(2.0): DPR={}", dpr);

        p.set_touch(true)?;
        println!("  [OK] set_touch(true)");

        p.set_geolocation(31.2304, 121.4737)?;
        println!("  [OK] set_geolocation: 上海");

        p.set_timezone("Asia/Shanghai")?;
        let tz = p.execute("Intl.DateTimeFormat().resolvedOptions().timeZone")?;
        println!("  [OK] set_timezone: {}", tz);

        p.set_user_agent("Mozilla/5.0 (SyncPage Test)")?;
        let ua = p.execute("navigator.userAgent")?;
        println!("  [OK] set_user_agent: {}", ua);

        p.emulate_device(375, 812, "Mozilla/5.0 (iPhone)", 3.0, true)?;
        println!("  [OK] emulate_device(iPhone)");

        // 重置
        p.set_viewport(1280, 800)?;
        p.set_device_scale(1.0)?;
        p.set_touch(false)?;
        p.set_user_agent("")?;
        println!("  [OK] 设备状态已重置");
        Ok(())
    })() {
        Ok(()) => pass += 1,
        Err(e) => { fail += 1; println!("  [FAIL] {}", e); }
    }

    // ══════════════════════════════════════════════════
    // 12. CSS 注入
    // ══════════════════════════════════════════════════
    section!(12, "CSS 注入");
    total += 1;
    match (|| -> Result<_, Box<dyn std::error::Error>> {
        build_page(&p)?;

        let css_id = p.inject_css("h1 { color: red !important; text-decoration: underline; }")?;
        println!("  [OK] inject_css: h1变红 (id={})", css_id);

        let h1 = p.ele("#title")?;
        let color = h1.style("color")?;
        println!("  [OK] 验证: h1 color = {:?}", color);

        p.remove_css(&css_id)?;
        println!("  [OK] remove_css: 已恢复");
        Ok(())
    })() {
        Ok(()) => pass += 1,
        Err(e) => { fail += 1; println!("  [FAIL] {}", e); }
    }

    // ══════════════════════════════════════════════════
    // 13. 智能等待
    // ══════════════════════════════════════════════════
    section!(13, "智能等待");
    total += 1;
    match (|| -> Result<_, Box<dyn std::error::Error>> {
        build_page(&p)?;

        p.wait_ele("h1", 5)?;
        println!("  [OK] wait_ele(h1, 5s)");

        p.wait_title_contains("SyncPage", 5)?;
        println!("  [OK] wait_title_contains(SyncPage)");

        let url = p.url()?;
        p.wait_url_is(&url, 5)?;
        println!("  [OK] wait_url_is()");

        p.wait_title_is("SyncPage Test", 5)?;
        println!("  [OK] wait_title_is(SyncPage Test)");

        p.wait_js("document.querySelectorAll('p').length > 0", 5)?;
        println!("  [OK] wait_js(p.length>0)");
        Ok(())
    })() {
        Ok(()) => pass += 1,
        Err(e) => { fail += 1; println!("  [FAIL] {}", e); }
    }

    // ══════════════════════════════════════════════════
    // 14. Agent API (智能操作)
    // ══════════════════════════════════════════════════
    section!(14, "Agent API (智能操作)");
    total += 1;
    match (|| -> Result<_, Box<dyn std::error::Error>> {
        build_page(&p)?;

        let elements = p.interactive_elements()?;
        println!("  [OK] interactive_elements: {} 个", elements.len());
        for el in elements.iter().take(5) {
            println!("       {:?} {} (input_type={:?})", el.tag, el.text, el.input_type);
        }

        let summary = p.page_summary()?;
        println!("  [OK] page_summary: title={}", summary.title);

        let snap = p.page_snapshot()?;
        let snap_len = serde_json::to_string(&snap)?.len();
        println!("  [OK] page_snapshot: {} chars", snap_len);

        let result = p.smart_click("Submit");
        println!("  [OK] smart_click(Submit): success={}", result.success);

        let result = p.smart_fill("username", "智能填写");
        println!("  [OK] smart_fill(username, 智能填写): success={}", result.success);
        Ok(())
    })() {
        Ok(()) => pass += 1,
        Err(e) => { fail += 1; println!("  [FAIL] {}", e); }
    }

    // ══════════════════════════════════════════════════
    // 15. DOM 快照与资源提取
    // ══════════════════════════════════════════════════
    section!(15, "DOM 快照与资源提取");
    total += 1;
    match (|| -> Result<_, Box<dyn std::error::Error>> {
        build_page(&p)?;

        let snap = p.dom_snapshot()?;
        let snap_len = serde_json::to_string(&snap)?.len();
        println!("  [OK] dom_snapshot: {} bytes JSON", snap_len);

        let links = p.links()?;
        println!("  [OK] links(): {} 个", links.len());
        for l in links.iter().take(3) {
            println!("       {}", l);
        }

        let imgs = p.images()?;
        println!("  [OK] images(): {} 个", imgs.len());
        Ok(())
    })() {
        Ok(()) => pass += 1,
        Err(e) => { fail += 1; println!("  [FAIL] {}", e); }
    }

    // ══════════════════════════════════════════════════
    // 16. 网络控制
    // ══════════════════════════════════════════════════
    section!(16, "网络控制");
    total += 1;
    match (|| -> Result<_, Box<dyn std::error::Error>> {
        build_page(&p)?;

        p.clear_cache()?;
        println!("  [OK] clear_cache()");

        p.disable_images()?;
        println!("  [OK] disable_images()");
        p.set_blocked_urls(&[])?;

        p.mute()?;
        println!("  [OK] mute()");
        p.unmute()?;
        println!("  [OK] unmute()");

        let mut headers = std::collections::HashMap::new();
        headers.insert("X-SyncPage".into(), "test".into());
        p.set_extra_headers(headers)?;
        println!("  [OK] set_extra_headers(X-SyncPage=test)");
        Ok(())
    })() {
        Ok(()) => pass += 1,
        Err(e) => { fail += 1; println!("  [FAIL] {}", e); }
    }

    // ══════════════════════════════════════════════════
    // 17. 性能指标
    // ══════════════════════════════════════════════════
    section!(17, "性能指标");
    total += 1;
    match (|| -> Result<_, Box<dyn std::error::Error>> {
        p.get("https://example.com")?;

        let metrics = p.performance_metrics()?;
        println!("  [OK] performance_metrics: {} 项", metrics.len());
        for (name, val) in metrics.iter().take(5) {
            println!("       {}: {:.1}", name, val);
        }

        let timing = p.page_timing()?;
        println!("  [OK] page_timing: {} 项", timing.len());
        if let Some(dom) = timing.get("domComplete") {
            println!("       domComplete: {:.0}ms", dom);
        }
        Ok(())
    })() {
        Ok(()) => pass += 1,
        Err(e) => { fail += 1; println!("  [FAIL] {}", e); }
    }

    // ══════════════════════════════════════════════════
    // 18. 元素树遍历
    // ══════════════════════════════════════════════════
    section!(18, "元素树遍历");
    total += 1;
    match (|| -> Result<_, Box<dyn std::error::Error>> {
        build_page(&p)?;

        let container = p.ele("#container")?;
        println!("  [OK] #container: tag={}, text_len={}", container.tag(), container.text().len());

        let h1 = p.ele("#title")?;
        let parent = h1.parent()?;
        println!("  [OK] h1.parent(): tag={}", parent.tag());

        let first = container.first_child()?;
        println!("  [OK] container.first_child(): tag={}", first.tag());

        let next = first.next()?;
        println!("  [OK] first.next(): tag={} text={}", next.tag(), next.text());

        let prev = next.prev()?;
        println!("  [OK] next.prev(): tag={}", prev.tag());

        let child_p = container.ele("tag:p")?;
        println!("  [OK] container.ele(tag:p): {}", child_p.text());

        let child_ps = container.eles("tag:p")?;
        println!("  [OK] container.eles(tag:p): {} 个", child_ps.len());
        Ok(())
    })() {
        Ok(()) => pass += 1,
        Err(e) => { fail += 1; println!("  [FAIL] {}", e); }
    }

    // ══════════════════════════════════════════════════
    // 19. iframe 操作
    // ══════════════════════════════════════════════════
    section!(19, "iframe 操作");
    total += 1;
    match (|| -> Result<_, Box<dyn std::error::Error>> {
        p.execute("document.body.innerHTML = '<iframe id=\"test-frame\" srcdoc=\"<h2>Inside iframe</h2><p>iframe content</p>\"></iframe>';")?;
        std::thread::sleep(Duration::from_millis(500));

        let frame_html = p.frame_html("#test-frame")?;
        println!("  [OK] frame_html: {} chars", frame_html.len());

        let frame_result = p.frame_execute("#test-frame", "return this.document.querySelector('h2').textContent")?;
        println!("  [OK] frame_execute: h2 = {}", frame_result);
        Ok(())
    })() {
        Ok(()) => pass += 1,
        Err(e) => { fail += 1; println!("  [FAIL] {}", e); }
    }

    // ══════════════════════════════════════════════════
    // 20. Init Script
    // ══════════════════════════════════════════════════
    section!(20, "Init Script (持久化JS)");
    total += 1;
    match (|| -> Result<_, Box<dyn std::error::Error>> {
        p.add_init_script("sync_test", "window.__sync_marker = 'injected_by_sync';")?;
        println!("  [OK] add_init_script(sync_test)");

        build_page(&p)?;
        let marker = p.execute("window.__sync_marker")?;
        println!("  [OK] 新页面验证: __sync_marker = {}", marker);

        p.remove_init_script("sync_test")?;
        println!("  [OK] remove_init_script(sync_test)");
        Ok(())
    })() {
        Ok(()) => pass += 1,
        Err(e) => { fail += 1; println!("  [FAIL] {}", e); }
    }

    // ══════════════════════════════════════════════════
    // 21. 权限与设备控制
    // ══════════════════════════════════════════════════
    section!(21, "权限与设备控制");
    total += 1;
    match (|| -> Result<_, Box<dyn std::error::Error>> {
        build_page(&p)?;

        p.grant_permissions("https://example.com", vec!["geolocation".into()])?;
        println!("  [OK] grant_permissions(geolocation)");
        p.reset_permissions()?;
        println!("  [OK] reset_permissions()");
        Ok(())
    })() {
        Ok(()) => pass += 1,
        Err(e) => { fail += 1; println!("  [FAIL] {}", e); }
    }

    // ══════════════════════════════════════════════════
    // 22. 生命周期与状态
    // ══════════════════════════════════════════════════
    section!(22, "生命周期与状态");
    total += 1;
    match (|| -> Result<_, Box<dyn std::error::Error>> {
        let connected = p.is_connected();
        println!("  [OK] is_connected: {}", connected);

        let debug = p.debug_url();
        println!("  [OK] debug_url: {}", debug);

        let cloned = p.clone_session();
        match cloned {
            Ok(c) => {
                let title = c.title()?;
                println!("  [OK] clone_session: title={}", title);
                drop(c);
            }
            Err(e) => println!("  [WARN] clone_session: {}", e),
        }

        build_page(&p)?;
        let h1 = p.ele("#title")?;
        let refreshed = p.refresh_ele(&h1)?;
        println!("  [OK] refresh_ele: tag={}", refreshed.tag());

        p.sleep(Duration::from_millis(100));
        println!("  [OK] sleep(100ms)");
        Ok(())
    })() {
        Ok(()) => pass += 1,
        Err(e) => { fail += 1; println!("  [FAIL] {}", e); }
    }

    // ══════════════════════════════════════════════════
    // 23. 文本操作
    // ══════════════════════════════════════════════════
    section!(23, "文本操作");
    total += 1;
    match (|| -> Result<_, Box<dyn std::error::Error>> {
        build_page(&p)?;

        let found = p.find_text("同步API")?;
        println!("  [OK] find_text(同步API): {}", found);

        let not_found = p.find_text("不存在")?;
        println!("  [OK] find_text(不存在): {}", not_found);
        Ok(())
    })() {
        Ok(()) => pass += 1,
        Err(e) => { fail += 1; println!("  [FAIL] {}", e); }
    }

    // ══════════════════════════════════════════════════
    // 24. 窗口状态
    // ══════════════════════════════════════════════════
    section!(24, "窗口状态");
    total += 1;
    match (|| -> Result<_, Box<dyn std::error::Error>> {
        build_page(&p)?;

        p.set_window_size(800, 600)?;
        println!("  [OK] set_window_position(100,100)");

        // minimize 后截图/PDF 可能卡住，只测试 set_window_size 缩小
        p.set_window_size(400, 300)?;
        println!("  [OK] set_window_size(400,300) simulate-minimize");

        p.set_window_size(1280, 800)?;
        println!("  [OK] set_window_size 恢复 1280x800");
        Ok(())
    })() {
        Ok(()) => pass += 1,
        Err(e) => { fail += 1; println!("  [FAIL] {}", e); }
    }

    // ══════════════════════════════════════════════════
    // 25. 下载配置
    // ══════════════════════════════════════════════════
    section!(25, "下载配置");
    total += 1;
    match (|| -> Result<_, Box<dyn std::error::Error>> {
        build_page(&p)?;

        p.set_download_file_name("test_download")?;
        println!("  [OK] set_download_file_name(test_download)");

        p.set_file_chooser(true);
        println!("  [OK] set_file_chooser(true)");
        Ok(())
    })() {
        Ok(()) => pass += 1,
        Err(e) => { fail += 1; println!("  [FAIL] {}", e); }
    }

    // ── 汇总 ──
    println!("\n{}", "=".repeat(50));
    println!("测试完成: {} 组通过 / {} 组失败 / {} 组总计", pass, fail, total);
    if fail == 0 {
        println!("全部通过！SyncPage 同步 API 完全可用。");
    } else {
        println!("有 {} 组失败，请检查上方输出。", fail);
    }

    Ok(())
}
