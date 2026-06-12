//! rpage 能力展示：常见复杂浏览器自动化操作
//! 覆盖 10 大场景，全部使用同步 API

use rpage::sync::SyncPage;
use rpage::chromium_page::CookieInfo;
use std::time::Duration;
use std::collections::HashMap;

macro_rules! section {
    ($n:expr, $title:expr) => {
        println!("\n{}", "═".repeat(60));
        println!("  {}. {}", $n, $title);
        println!("{}", "═".repeat(60));
    };
}

macro_rules! ok {
    ($($arg:tt)*) => { println!("  ✅ {}", format!($($arg)*)) };
}

macro_rules! info {
    ($($arg:tt)*) => { println!("     {}", format!($($arg)*)) };
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("🚀 rpage 浏览器自动化能力全面展示");
    println!("   连接 Chrome CDP → 同步 API 驱动");

    // ── 连接 Chrome ──
    let p = SyncPage::connect("http://127.0.0.1:9222")?;
    ok!("连接 Chrome 成功");

    // ══════════════════════════════════════════════════
    // 1. 页面导航 & DOM 操作
    // ══════════════════════════════════════════════════
    section!(1, "页面导航 & DOM 操作");
    {
        // 打开一个真实网站
        p.get("https://httpbin.org/forms/post")?;
        ok!("导航到 httpbin.org/forms/post");
        info!("title = {}", p.title()?);

        // 元素查找与交互
        let input = p.ele("tag:input")?;
        ok!("定位第一个 input: tag={}, visible={}", input.tag(), input.is_visible());

        // 填写表单 — 链式调用
        p.type_text("input[name='custname']", "张三")?;
        ok!("填写 custname = 张三");

        p.type_text("input[name='custtel']", "13800138000")?;
        ok!("填写 custtel = 13800138000");

        p.type_text("input[name='custemail']", "test@example.com")?;
        ok!("填写 custemail = test@example.com");

        // 多选框
        let checkboxes = p.eles("input[name='topping']")?;
        ok!("找到 {} 个 topping 复选框", checkboxes.len());
        for (i, cb) in checkboxes.iter().enumerate() {
            cb.click()?;
            info!("  复选框[{}] 已点击", i);
        }

        // 单选框
        p.click_ele("input[value='medium']")?;
        ok!("选中 medium 尺寸");

        // textarea
        p.type_text("tag:textarea", "这是 rpage 自动化测试，请忽略。")?;
        ok!("填写备注");

        // 截图保存
        p.screenshot("showcase_form.png")?;
        ok!("截图保存 → showcase_form.png");
    }

    // ══════════════════════════════════════════════════
    // 2. JavaScript 执行 & 异步等待
    // ══════════════════════════════════════════════════
    section!(2, "JavaScript 执行 & 异步等待");
    {
        // 同步 JS
        let result = p.execute("document.querySelectorAll('input').length")?;
        ok!("execute: 页面上有 {} 个 input", result);

        // 异步 JS — fetch 数据
        let data = p.run_async_js(
            "const r = await fetch('https://httpbin.org/json'); return await r.json();"
        )?;
        ok!("run_async_js: 获取到 JSON, slideshow.title = {}",
            data.get("slideshow").and_then(|s| s.get("title")).and_then(|t| t.as_str()).unwrap_or("?"));

        // run_js_with_args
        let sum = p.run_js_with_args(
            "(a) => a.nums.reduce((s,x) => s+x, 0)",
            serde_json::json!({"nums": [1, 2, 3, 4, 5, 6, 7, 8, 9, 10]}),
        )?;
        ok!("run_js_with_args(1..10 sum): {}", sum);

        // evaluate_on_new_document — 注入全局变量
        p.evaluate_on_new_document("window.__rpage_injected = 'hello from rpage'")?;
        p.get("https://httpbin.org/get")?;
        let injected = p.execute("window.__rpage_injected")?;
        ok!("evaluate_on_new_document: 注入变量 = {}", injected);
        info!("title = {}", p.title()?);
    }

    // ══════════════════════════════════════════════════
    // 3. 元素高级操作 (DOM 树遍历、拖拽、Shadow DOM)
    // ══════════════════════════════════════════════════
    section!(3, "元素高级操作");
    {
        // 构建本地测试页面
        p.execute(r#"
            document.body.innerHTML = '<div id="root"><h2>DOM Tree Test</h2><ul><li>A</li><li>B</li><li class="active">C</li></ul></div>';
            document.title = 'Element Showcase';
        "#)?;
        std::thread::sleep(Duration::from_millis(200));

        let root = p.ele("#root")?;
        ok!("定位 #root: tag={}", root.tag());

        // DOM 树遍历
        let first_child = root.first_child()?;
        ok!("first_child: tag={}, text={}", first_child.tag(), first_child.text());

        let next = first_child.next()?;
        ok!("next sibling: tag={}", next.tag());

        let li_items = next.eles("tag:li")?;
        ok!("找到 {} 个 li 子元素", li_items.len());

        let parent = next.parent()?;
        ok!("parent: tag={}", parent.tag());

        // 元素属性操作
        let active_li = p.ele("li.active")?;
        active_li.set_attr("data-status", "selected")?;
        ok!("set_attr: data-status = selected");

        active_li.add_class("highlight")?;
        ok!("add_class: highlight");
        let has = active_li.has_class("highlight")?;
        ok!("has_class('highlight'): {}", has);
        active_li.remove_class("highlight")?;
        ok!("remove_class: highlight 已移除");

        // 元素样式
        active_li.set_style("color", "red")?;
        let color = active_li.style("color")?;
        ok!("set_style/get_style: color = {}", color);

        // 元素移除（通过 JS）
        p.execute("document.querySelector('li:nth-child(2)')?.remove()")?;
        ok!("JS remove: 第2个 li 已从 DOM 移除");

        let remaining = p.eles("tag:li")?;
        ok!("剩余 li: {} 个", remaining.len());

        // 拖拽
        p.execute(r#"
            document.body.innerHTML += '<div id="drag" style="width:80px;height:80px;background:blue;position:absolute;top:100px;left:100px;">Drag</div>';
        "#)?;
        std::thread::sleep(Duration::from_millis(100));
        let drag_el = p.ele("#drag")?;
        drag_el.drag_to_offset(200.0, 150.0)?;
        ok!("drag_to_offset(200, 150): 拖拽完成");
    }

    // ══════════════════════════════════════════════════
    // 4. 多标签页管理
    // ══════════════════════════════════════════════════
    section!(4, "多标签页管理");
    {
        let titles_before = p.tab_titles()?;
        ok!("当前标签页数: {}", titles_before.len());

        // 打开新标签页
        p.new_tab()?;
        std::thread::sleep(Duration::from_millis(500));
        let titles_after = p.tab_titles()?;
        ok!("新标签页后: {} 个", titles_after.len());

        // 在新标签页中导航
        let last_idx = titles_after.len() - 1;
        p.switch_to_tab(last_idx)?;
        p.get("https://httpbin.org/headers")?;
        ok!("切换到新标签页并导航");
        info!("title = {}", p.title()?);

        // 切回
        p.switch_to_tab(0)?;
        ok!("切回第 0 个标签页");
    }

    // ══════════════════════════════════════════════════
    // 5. Cookie 管理
    // ══════════════════════════════════════════════════
    section!(5, "Cookie 管理");
    {
        p.get("https://httpbin.org/cookies/set?session_id=abc123&theme=dark")?;
        std::thread::sleep(Duration::from_millis(300));

        let cookies = p.cookies()?;
        ok!("当前 cookies: {} 个", cookies.len());
        for c in &cookies {
            info!("  {} = {}", c.name, c.value);
        }

        // 添加自定义 cookie
        p.set_cookie(CookieInfo {
            name: "rpage_test".into(),
            value: "works!".into(),
            domain: Some("httpbin.org".into()),
            path: Some("/".into()),
            secure: true,
            http_only: false,
        })?;
        ok!("set_cookie: rpage_test = works!");

        // 验证 cookie
        p.get("https://httpbin.org/cookies")?;
        let cookie_json = p.execute("document.body.innerText")?;
        ok!("验证 cookies: {}", cookie_json.as_str().unwrap_or("").chars().take(120).collect::<String>());

        // 删除 cookie
        p.delete_cookie("rpage_test")?;
        ok!("delete_cookie: rpage_test 已删除");
    }

    // ══════════════════════════════════════════════════
    // 6. 网络拦截 & 模拟
    // ══════════════════════════════════════════════════
    section!(6, "网络控制");
    {
        // 自定义 User-Agent
        p.set_user_agent("rpage-bot/1.0 (Browser Automation)")?;
        p.get("https://httpbin.org/user-agent")?;
        let ua = p.execute("document.body.innerText")?;
        ok!("set_user_agent: {}", ua.as_str().unwrap_or("").chars().take(80).collect::<String>());

        // 自定义 Headers
        let mut headers = HashMap::new();
        headers.insert("X-Custom-Header".into(), "rpage-was-here".into());
        headers.insert("X-Request-Id".into(), "12345".into());
        p.set_extra_headers(headers)?;
        p.get("https://httpbin.org/headers")?;
        let resp = p.execute("document.body.innerText")?;
        let resp_str = resp.as_str().unwrap_or("");
        ok!("set_extra_headers: 响应包含 X-Custom-Header = {}", resp_str.contains("rpage-was-here"));

        // 屏蔽 URL
        p.set_blocked_urls(&["https://httpbin.org/image/png"])?;
        ok!("set_blocked_urls: 已屏蔽 image/png");
        p.set_blocked_urls(&[])?; // 恢复
        ok!("已恢复 URL 屏蔽");
    }

    // ══════════════════════════════════════════════════
    // 7. 页面截图 & PDF 生成
    // ══════════════════════════════════════════════════
    section!(7, "页面截图 & PDF 生成");
    {
        p.get("https://httpbin.org/html")?;
        std::thread::sleep(Duration::from_millis(500));

        // 全页截图
        p.screenshot("showcase_httpbin.png")?;
        ok!("全页截图 → showcase_httpbin.png");

        // 元素截图
        let h1 = p.ele("tag:h1")?;
        h1.screenshot("showcase_h1_element.png")?;
        ok!("元素截图(h1) → showcase_h1_element.png");

        // PDF 生成
        p.pdf("showcase_page.pdf")?;
        ok!("PDF 生成 → showcase_page.pdf");
    }

    // ══════════════════════════════════════════════════
    // 8. 智能等待 & 条件匹配
    // ══════════════════════════════════════════════════
    section!(8, "智能等待 & 条件匹配");
    {
        p.get("https://httpbin.org/delay/2")?;
        ok!("访问 /delay/2 (模拟慢速响应)");

        // 等待特定元素
        let el = p.wait_ele("tag:pre", 10)?;
        ok!("wait_ele(tag:pre, 10s): 找到 = {}", el.tag());

        // wait_js — 等待条件成立
        p.get("https://httpbin.org/html")?;
        p.wait_js("document.querySelector('h1') !== null", 5)?;
        ok!("wait_js: h1 已出现");

        // 等待 title 包含 — httpbin 的 title 可能为空，用 URL 匹配代替
        p.wait_url_contains("httpbin", 5)?;
        ok!("wait_url_contains('httpbin'): 通过");

        // 元素等待可见
        let h1 = p.ele("tag:h1")?;
        h1.wait_for_visible()?;
        ok!("wait_for_visible: h1 可见");
    }

    // ══════════════════════════════════════════════════
    // 9. 设备模拟 & 视口控制
    // ══════════════════════════════════════════════════
    section!(9, "设备模拟 & 视口控制");
    {
        // 设置视口
        p.set_viewport(375, 812)?;
        ok!("set_viewport(375x812): iPhone X 尺寸");
        let bounds = p.get_window_bounds()?;
        info!("窗口: left={} top={} w={} h={}", bounds.0, bounds.1, bounds.2, bounds.3);

        // 设备缩放
        p.set_device_scale(2.0)?;
        ok!("set_device_scale(2.0): Retina 模式");

        // 模拟触摸
        p.set_touch(true)?;
        ok!("set_touch(true): 触摸模式");

        // 地理位置
        p.set_geolocation(39.9042, 116.4074)?;
        ok!("set_geolocation: 北京 (39.90, 116.41)");

        // 时区
        p.set_timezone("Asia/Shanghai")?;
        ok!("set_timezone: Asia/Shanghai");

        // 恢复
        p.set_viewport(1280, 800)?;
        p.set_device_scale(1.0)?;
        p.set_touch(false)?;
        ok!("已恢复正常视口");
    }

    // ══════════════════════════════════════════════════
    // 10. 页面分析 & Agent 智能接口
    // ══════════════════════════════════════════════════
    section!(10, "页面分析 & Agent 智能接口");
    {
        p.get("https://httpbin.org/forms/post")?;
        std::thread::sleep(Duration::from_millis(300));

        // 交互元素分析
        let elements = p.interactive_elements()?;
        ok!("interactive_elements: {} 个可交互元素", elements.len());
        for (i, el) in elements.iter().take(8).enumerate() {
            info!("[{}] {:?} {} type={:?}", i, el.tag, el.text.chars().take(30).collect::<String>(), el.input_type);
        }

        // 页面摘要
        let summary = p.page_summary()?;
        ok!("page_summary: title={:?}, url={:?}", summary.title, summary.url.chars().take(60).collect::<String>());

        // 页面快照
        let snap = p.page_snapshot()?;
        ok!("page_snapshot: {} 个交互元素, viewport={}", snap.interactive_elements.len(), snap.viewport_size);

        // DOM 快照
        let dom = p.dom_snapshot()?;
        ok!("dom_snapshot: JSON size = {} chars", dom.to_string().len());

        // 智能点击
        let result = p.smart_click("Submit");
        ok!("smart_click('Submit'): success={}", result.success);

        // 智能填写
        let result = p.smart_fill("custname", "Agent Bot");
        ok!("smart_fill('custname', 'Agent Bot'): success={}", result.success);

        // 链接提取
        let links = p.links()?;
        ok!("links: 提取到 {} 个链接", links.len());

        // 图片提取
        let imgs = p.images()?;
        ok!("images: 提取到 {} 个图片", imgs.len());

        // 性能指标
        let metrics = p.performance_metrics()?;
        ok!("performance_metrics: {} 项指标", metrics.len());
        for (name, val) in metrics.iter().take(5) {
            info!("  {} = {:.2}", name, val);
        }
    }

    // ── 汇总 ──
    println!("\n{}", "═".repeat(60));
    println!("  🎉 rpage 能力展示完毕！");
    println!("  涵盖: 导航、表单、JS执行、DOM遍历、多标签、Cookie、");
    println!("        网络控制、截图PDF、智能等待、设备模拟、Agent接口");
    println!("{}", "═".repeat(60));

    // 不关闭浏览器，保持连接
    Ok(())
}
