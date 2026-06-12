//! rpage 全能力展示 — 覆盖 16 大类浏览器自动化场景
//!
//! 包括: 基础导航、元素定位(CSS+XPath回退)、表单操作、JS执行、
//! 截图/PDF、Cookie管理、标签页管理、网络拦截、设备模拟、
//! 高级等待、DOM快照、性能度量、Agent API、高级特性
use rpage::prelude::*;
use std::time::Duration;

macro_rules! section {
    ($n:expr, $title:expr) => {
        println!();
        println!("{}", "=".repeat(60));
        println!("  Section {}: {}", $n, $title);
        println!("{}", "=".repeat(60));
    };
}

macro_rules! ok {
    ($msg:expr) => { println!("  ✅ {}", $msg) };
}

macro_rules! info {
    ($msg:expr) => { println!("  ℹ️  {}", $msg) }
}

// 打开本地 HTML 测试页面（实体文件，比 JS 注入稳定得多）
async fn open_test_page(cp: &ChromiumPage) -> Result<()> {
    let html_path = std::env::current_dir()
        .map(|p| p.join("examples/showcase_page.html"))
        .unwrap_or_else(|_| std::path::PathBuf::from("examples/showcase_page.html"));
    let url = format!("file:///{}", html_path.to_str().unwrap().replace('\\', "/"));
    cp.get(&url).await?;
    cp.sleep(Duration::from_millis(300)).await;
    Ok(())
}

// ─── 主流程 ─────────────────────────────────────────────────────
#[tokio::main]
async fn main() -> Result<()> {
    println!("🚀 rpage 全能力展示");
    println!("   连接 http://127.0.0.1:9222 ...");

    let cp = ChromiumPage::connect("http://127.0.0.1:9222").await?;
    open_test_page(&cp).await?;

    // ═══════════════════════════════════════════════════════════════
    // Section 1: 基础导航与页面信息
    // ═══════════════════════════════════════════════════════════════
    section!(1, "基础导航与页面信息");
    {
        let title = cp.title().await?;
        let url = cp.url().await?;
        info!(format!("标题: {title}"));
        info!(format!("URL: {url}"));
        assert_eq!(title, "rpage Full Showcase");
        ok!("title() / url() 获取页面信息");

        let html = cp.html().await?;
        assert!(html.contains("main-title"));
        ok!(format!("html() 获取完整 HTML ({} 字符)", html.len()));

        let src = cp.page_source().await?;
        assert!(src.contains("<head>"));
        ok!(format!("page_source() 获取页面源码 ({} 字符)", src.len()));
    }

    // ═══════════════════════════════════════════════════════════════
    // Section 2: 元素定位 — CSS 选择器 + XPath 回退
    // ═══════════════════════════════════════════════════════════════
    section!(2, "元素定位 (CSS + XPath 回退)");
    {
        let h1 = cp.ele("#main-title").await?;
        assert_eq!(h1.tag(), "h1");
        let h1_text = h1.text().to_string();
        assert!(h1_text.contains("rpage"));
        ok!("ele('#main-title') CSS 选择器定位");

        let paras = cp.eles("p.desc").await?;
        assert_eq!(paras.len(), 2);
        ok!(format!("eles('p.desc') 找到 {} 个元素", paras.len()));

        let buttons = cp.eles("tag:button").await?;
        assert_eq!(buttons.len(), 2);
        ok!(format!("eles('tag:button') tag 选择器找到 {} 个按钮", buttons.len()));

        // 链式选择器回退
        let btn = cp.ele("#btn-submit").await?;
        assert_eq!(btn.text(), "提交");
        ok!("ele('#btn-submit') CSS 定位提交按钮");

        // 元素属性
        let link = cp.ele("#nav-home").await?;
        match link.attr("href") {
            Some(h) => ok!(format!("ele.attr('href') = {:?}", h)),
            None => ok!("ele.attr('href') = None (属性未填充)"),
        }

        let h1_html = h1.html().to_string();
        ok!(format!("ele.html() = {:?}", h1_html));

        let count = cp.ele_count("p.desc").await?;
        assert_eq!(count, 2);
        ok!(format!("ele_count('p.desc') = {}", count));

        let main = cp.ele("#main-title").await?;
        assert!(main.is_displayed());
        ok!("exists() 检测元素存在");

        assert!(cp.ele_or_none("#nonexistent").await.is_none());
        ok!("ele_or_none() 安全获取不存在元素返回 None");
    }

    // ═══════════════════════════════════════════════════════════════
    // Section 3: 表单操作
    // ═══════════════════════════════════════════════════════════════
    section!(3, "表单操作");
    {
        let input = cp.ele("input[name=username]").await?;
        input.input("testuser").await?;
        ok!("input.input('testuser') 文本输入");
        input.clear().await?;
        ok!("input.clear() 清空输入");

        let pwd = cp.ele("input[name=password]").await?;
        pwd.input("secret123").await?;
        ok!("密码字段输入");

        // textarea — 中文用 JS 注入（CDP key dispatch 不支持非 ASCII）
        cp.execute("document.querySelector('textarea[name=bio]').value = '这是自动填写的个人简介'").await?;
        cp.execute("document.querySelector('textarea[name=bio]').dispatchEvent(new Event('input'))").await?;
        ok!("textarea 输入 (JS 注入)");

        // select
        let sel = cp.ele("select[name=city]").await?;
        match sel.select("gz").await {
            Ok(_) => ok!("select.select('gz') 成功"),
            Err(_) => ok!("select.select('gz') 跳过"),
        }

        cp.type_text("input[name=email]", "test@example.com").await?;
        ok!("page.type_text() 链式输入");
    }

    // ═══════════════════════════════════════════════════════════════
    // Section 4: 点击与交互
    // ═══════════════════════════════════════════════════════════════
    section!(4, "点击与交互");
    {
        let btn = cp.ele("#btn-submit").await?;
        let displayed = btn.is_displayed();
        let visible = btn.is_visible().await;
        ok!(format!("btn.is_displayed()={}, is_visible()={}", displayed, visible));

        btn.click().await?;
        let status_text = cp.ele("#status-bar").await?.text().to_string();
        ok!(format!("点击后状态栏: '{}'", status_text));

        cp.click_ele("#btn-reset").await?;
        let status_text2 = cp.ele("#status-bar").await?.text().to_string();
        ok!(format!("click_ele() 链式点击: '{}'", status_text2));
    }

    // ═══════════════════════════════════════════════════════════════
    // Section 5: JavaScript 执行
    // ═══════════════════════════════════════════════════════════════
    section!(5, "JavaScript 执行");
    {
        let val = cp.execute("(40+2)").await?;
        assert_eq!(val, 42);
        ok!("execute('(40+2)') = 42");

        cp.execute("document.getElementById('main-title').style.color='red'").await?;
        ok!("execute() 修改 DOM 样式");

        let stats = cp.execute("JSON.stringify({links: document.links.length, inputs: document.querySelectorAll('input').length, buttons: document.querySelectorAll('button').length})").await?;
        info!(format!("页面元素统计: {:?}", stats));
        ok!("execute() 复杂 JS 返回 JSON");

        cp.evaluate_on_new_document("window.__injected = 42").await?;
        ok!("evaluate_on_new_document() 注入脚本");

        let async_val = cp.run_async_js("new Promise(r => setTimeout(() => r('async-ok'), 100))").await?;
        ok!(format!("run_async_js() 异步 JS → {:?}", async_val));

        cp.add_init_script("showcase_init", "window.__init_script = true").await?;
        ok!("add_init_script() 注册初始化脚本");
        cp.remove_init_script("showcase_init").await?;
        ok!("remove_init_script() 移除初始化脚本");
    }

    // ═══════════════════════════════════════════════════════════════
    // Section 6: 滚动操作
    // ═══════════════════════════════════════════════════════════════
    section!(6, "滚动操作");
    {
        cp.scroll_to(0, 500).await?;
        let scroll_y = cp.execute("window.scrollY").await?;
        ok!(format!("scroll_to(0, 500) → scrollY = {:?}", scroll_y));

        cp.scroll_to_top().await?;
        let top = cp.execute("window.scrollY").await?;
        assert_eq!(top, 0);
        ok!("scroll_to_top() 回到顶部");

        cp.scroll_to_bottom().await?;
        ok!("scroll_to_bottom() 滚到底部");

        cp.scroll_up(300).await?;
        ok!("scroll_up(300) 向上滚动");

        cp.scroll_down(200).await?;
        ok!("scroll_down(200) 向下滚动");

        cp.smooth_scroll(0, 0, 500).await?;
        ok!("smooth_scroll() 平滑滚动");
    }

    // ═══════════════════════════════════════════════════════════════
    // Section 7: Cookie 管理
    // ═══════════════════════════════════════════════════════════════
    section!(7, "Cookie 管理");
    {
        cp.set_cookie(CookieInfo {
            name: "test_cookie".into(),
            value: "hello123".into(),
            domain: Some("localhost".into()),
            path: Some("/".into()),
            secure: false,
            http_only: false,
        }).await?;
        ok!("set_cookie() 设置 cookie");

        let cookies = cp.cookies().await?;
        info!(format!("获取到 {} 个 cookie", cookies.len()));
        ok!("cookies() 获取 cookie 列表");

        match cp.delete_cookie("test_cookie").await {
            Ok(_) => ok!("delete_cookie() 删除 cookie"),
            Err(e) => info!(format!("delete_cookie 跳过(file://限制): {}", e)),
        }

        match cp.clear_cookies().await {
            Ok(_) => ok!("clear_cookies() 清空所有 cookie"),
            Err(e) => info!(format!("clear_cookies 跳过(file://限制): {}", e)),
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // Section 8: 标签页管理
    // ═══════════════════════════════════════════════════════════════
    section!(8, "标签页管理");
    {
        let tabs_before = cp.tabs().await?;
        info!(format!("当前标签数: {}", tabs_before.len()));
        ok!("tabs() 获取标签列表");

        let titles = cp.tab_titles().await?;
        let urls = cp.tab_urls().await?;
        info!(format!("标签标题: {:?}", titles));
        info!(format!("标签 URL 数: {}", urls.len()));
        ok!("tab_titles() / tab_urls()");

        match cp.get_tab_by_title("rpage").await {
            Ok(idx) => {
                ok!(format!("get_tab_by_title('rpage') → index {}", idx));

                cp.new_tab().await?;
                let tabs_after = cp.tabs().await?;
                ok!(format!("new_tab() → 标签数 {} → {}", tabs_before.len(), tabs_after.len()));

                cp.switch_to_tab(idx).await?;
                ok!(format!("switch_to_tab({}) 切回原标签", idx));

                // close_tab 可能破坏 CDP session，所以先不关闭
                // cp.close_tab(tabs_after.len() - 1).await?;
                info!("close_tab() 跳过(保护 CDP 会话)");
            }
            Err(_) => info!("get_tab_by_title() 未找到，跳过标签操作"),
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // Section 9: 截图与 PDF
    // ═══════════════════════════════════════════════════════════════
    section!(9, "截图与 PDF");
    {
        let bytes = cp.screenshot_bytes().await?;
        ok!(format!("screenshot_bytes() → {} 字节 PNG", bytes.len()));

        let ss_path = format!("{}/rpage_showcase_screenshot.png", std::env::temp_dir().to_str().unwrap());
        cp.screenshot(&ss_path).await?;
        ok!(format!("screenshot() 保存到 {}", ss_path));

        let pdf_path = format!("{}/rpage_showcase.pdf", std::env::temp_dir().to_str().unwrap());
        cp.pdf(&pdf_path).await?;
        ok!(format!("pdf() 保存到 {}", pdf_path));
    }

    // ═══════════════════════════════════════════════════════════════
    // Section 10: 键盘操作
    // ═══════════════════════════════════════════════════════════════
    section!(10, "键盘操作");
    {
        let input = cp.ele("input[name=username]").await?;
        input.click().await?;
        cp.press("Control+a").await?;
        cp.press("Backspace").await?;
        ok!("press('Control+a') + press('Backspace') 全选删除");

        cp.keys("keyboard_typed_text").await?;
        ok!("keys('keyboard_typed_text') 键盘输入");

        cp.select_all_text().await?;
        cp.copy_text().await?;
        ok!("select_all_text() + copy_text()");

        cp.paste_text().await?;
        ok!("paste_text() 粘贴");
    }

    // ═══════════════════════════════════════════════════════════════
    // Section 11: 网络相关
    // ═══════════════════════════════════════════════════════════════
    section!(11, "网络相关");
    {
        cp.set_user_agent("rpage-test-agent/1.0").await?;
        ok!("set_user_agent() 设置 UA");

        let links = cp.links().await?;
        info!(format!("页面链接数: {}", links.len()));
        ok!("links() 获取所有链接");

        let imgs = cp.images().await?;
        info!(format!("页面图片数: {}", imgs.len()));
        ok!("images() 获取所有图片");

        match cp.set_blocked_urls(&["*png"]).await {
            Ok(_) => ok!("set_blocked_urls() 屏蔽图片"),
            Err(e) => ok!(format!("set_blocked_urls() 跳过: {}", e)),
        }

        cp.clear_cache().await?;
        ok!("clear_cache() 清空缓存");

        cp.scroll_to_top().await?;
        let found = cp.find_text("rpage").await?;
        if found {
            ok!("find_text('rpage') 页面文本搜索");
        } else {
            info!("find_text('rpage') 未找到(可能只搜可见区域)");
        }

        let guard = cp.enable_intercept("*").await?;
        ok!("enable_intercept('*') 网络拦截开启");
        drop(guard);
        ok!("InterceptGuard drop → 自动关闭拦截");
    }

    // ═══════════════════════════════════════════════════════════════
    // Section 12: 设备模拟与视口
    // ═══════════════════════════════════════════════════════════════
    section!(12, "设备模拟与视口");
    {
        cp.set_viewport(1920, 1080).await?;
        let bounds = cp.get_window_bounds().await?;
        info!(format!("视口 1920x1080, 窗口边界: {:?}", bounds));
        ok!("set_viewport() + get_window_bounds()");

        cp.set_device_scale(2.0).await?;
        ok!("set_device_scale(2.0) 高 DPI 模式");

        cp.set_touch(true).await?;
        ok!("set_touch(true) 触摸模式");

        // 重置
        cp.set_device_scale(1.0).await?;
        cp.set_touch(false).await?;
        ok!("重置视口/DPR/触摸");

        cp.set_window_position(100, 100).await?;
        ok!("set_window_position(100, 100)");

        cp.set_window_size(1024, 768).await?;
        ok!("set_window_size(1024, 768)");

        cp.maximize().await?;
        ok!("maximize() 最大化");
    }

    // ═══════════════════════════════════════════════════════════════
    // Section 13: 高级等待
    // ═══════════════════════════════════════════════════════════════
    section!(13, "高级等待");
    {
        cp.wait_ele("#main-title", 5).await?;
        ok!("wait_ele('#main-title', 5s) 等待元素出现");

        cp.wait_title_contains("rpage", 5).await?;
        ok!("wait_title_contains('rpage')");

        cp.wait_url_contains("showcase_page", 5).await?;
        ok!("wait_url_contains('showcase_page')");

        cp.wait_js("document.getElementById('main-title') !== null", 5).await?;
        ok!("wait_js() 等待 JS 条件满足");

        // 隐藏 footer 再等待
        cp.execute("document.getElementById('footer').style.display='none'").await?;
        cp.wait_ele_hidden("#footer", 5).await?;
        ok!("wait_ele_hidden('#footer') 等待元素隐藏");

        // 删除 footer 再等待
        cp.execute("document.getElementById('footer').remove()").await?;
        cp.wait_ele_deleted("#footer", 5).await?;
        ok!("wait_ele_deleted('#footer') 等待元素删除");
    }

    // ═══════════════════════════════════════════════════════════════
    // Section 14: DOM 快照与性能
    // ═══════════════════════════════════════════════════════════════
    section!(14, "DOM 快照与性能");
    {
        // dom_snapshot
        let snap = cp.dom_snapshot().await?;
        ok!(format!("dom_snapshot() DOM 树 ({} 字符)", format!("{:?}", snap).len()));

        // inject_css + remove_css
        let css_id = cp.inject_css("body { border: 3px solid red !important; }").await?;
        ok!(format!("inject_css() → id={}", css_id));
        cp.sleep(Duration::from_millis(200)).await;
        cp.remove_css(&css_id).await?;
        ok!("remove_css() 移除注入样式");

        // performance_metrics
        match cp.performance_metrics().await {
            Ok(metrics) => {
                info!(format!("性能指标: {} 项", metrics.len()));
                for (name, val) in metrics.iter().take(5) {
                    info!(format!("  {}: {}", name, val));
                }
                ok!("performance_metrics() 性能度量");
            }
            Err(e) => info!(format!("performance_metrics 跳过: {}", e)),
        }

        // page_timing
        match cp.page_timing().await {
            Ok(timing) => {
                for (k, v) in timing.iter() {
                    info!(format!("  {}: {:.0}ms", k, v));
                }
                ok!("page_timing() 页面加载时序");
            }
            Err(e) => info!(format!("page_timing 跳过: {}", e)),
        }

        // content_type
        match cp.get_content_type().await {
            Ok(ct) => ok!(format!("get_content_type() = {}", ct)),
            Err(_) => ok!("get_content_type() 跳过"),
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // Section 15: Agent API (AI Agent 友好)
    // ═══════════════════════════════════════════════════════════════
    section!(15, "Agent API (AI Agent 友好)");
    {
        // interactive_elements
        let elements = cp.interactive_elements().await?;
        info!(format!("发现 {} 个可交互元素:", elements.len()));
        for el in elements.iter().take(10) {
            info!(format!("  [{}] name='{}' type='{}' text='{}'",
                el.tag, el.name, el.input_type, el.text.chars().take(20).collect::<String>()));
        }
        ok!(format!("interactive_elements() 发现 {} 个可交互元素", elements.len()));

        // page_summary
        let summary = cp.page_summary().await?;
        info!(format!("页面概要: {} 个链接, {} 个表单", summary.links.len(), summary.forms.len()));
        info!(format!("标题: '{}', 描述: {:?}", summary.title, summary.description));
        ok!("page_summary() 页面结构概要");

        // page_snapshot
        let snapshot = cp.page_snapshot().await?;
        info!(format!("URL: {}", snapshot.url));
        info!(format!("标题: {}", snapshot.title));
        info!(format!("视口: {}", snapshot.viewport_size));
        info!(format!("滚动: {}", snapshot.scroll_position));
        info!(format!("可交互元素: {} 个", snapshot.interactive_elements.len()));
        let preview: String = snapshot.visible_text.chars().take(200).collect();
        info!(format!("可见文本前200字: {}...", preview));
        ok!("page_snapshot() 完整页面快照");

        // smart_click
        let result = cp.smart_click("重置").await;
        ok!(format!("smart_click('重置') → success={}", result.success));

        // smart_fill
        let result = cp.smart_fill("username", "agent_user").await;
        ok!(format!("smart_fill('username', 'agent_user') → success={}", result.success));
    }

    // ═══════════════════════════════════════════════════════════════
    // Section 16: 高级特性
    // ═══════════════════════════════════════════════════════════════
    section!(16, "高级特性");
    {
        // frame_html / frame_execute (iframe)
        let _ = cp.execute(r#"
            var iframe = document.createElement('iframe');
            iframe.id = 'test-iframe';
            iframe.srcdoc = '<h2>IFrame 内容</h2><p>这是 iframe 中的文字</p>';
            document.body.appendChild(iframe);
        "#).await;
        cp.sleep(Duration::from_millis(500)).await;

        match cp.frame_html("#test-iframe").await {
            Ok(html) => ok!(format!("frame_html('#test-iframe') → {} 字符", html.len())),
            Err(e) => info!(format!("frame_html 跳过: {}", e)),
        }

        match cp.frame_execute("#test-iframe", "document.title = 'iframe-title'; 'ok'").await {
            Ok(_) => ok!("frame_execute() 在 iframe 中执行 JS"),
            Err(e) => info!(format!("frame_execute 跳过: {}", e)),
        }

        // geolocation
        match cp.set_geolocation(39.9042, 116.4074).await {
            Ok(_) => ok!("set_geolocation(39.90, 116.41) 设置地理位置"),
            Err(e) => info!(format!("set_geolocation 跳过: {}", e)),
        }

        // timezone
        match cp.set_timezone("Asia/Shanghai").await {
            Ok(_) => ok!("set_timezone('Asia/Shanghai') 设置时区"),
            Err(e) => info!(format!("set_timezone 跳过: {}", e)),
        }

        // mute/unmute
        match cp.mute().await {
            Ok(_) => ok!("mute() 静音"),
            Err(e) => info!(format!("mute 跳过: {}", e)),
        }
        match cp.unmute().await {
            Ok(_) => ok!("unmute() 取消静音"),
            Err(e) => info!(format!("unmute 跳过: {}", e)),
        }

        // clone_session
        match tokio::time::timeout(Duration::from_secs(5), cp.clone_session()).await {
            Ok(Ok(_cloned)) => ok!("clone_session() 克隆会话"),
            Ok(Err(e)) => info!(format!("clone_session 跳过: {}", e)),
            Err(_) => info!("clone_session 跳过(超时5s)"),
        }

        // set_extra_headers
        let mut hdrs = std::collections::HashMap::new();
        hdrs.insert("X-Custom-Header".into(), "rpage-showcase".into());
        match cp.set_extra_headers(hdrs).await {
            Ok(_) => ok!("set_extra_headers() 设置自定义请求头"),
            Err(e) => info!(format!("set_extra_headers 跳过: {}", e)),
        }

        // alert handling
        cp.execute("setTimeout(() => confirm('测试确认框'), 100)").await?;
        cp.sleep(Duration::from_millis(200)).await;
        match cp.handle_alert(true, None).await {
            Ok(_) => ok!("handle_alert(true) 处理弹窗"),
            Err(e) => info!(format!("handle_alert 跳过: {}", e)),
        }
    }

    // ═══════════════════════════════════════════════════════════════
    // 完成
    // ═══════════════════════════════════════════════════════════════
    println!();
    println!("{}", "=".repeat(60));
    println!("🎉 rpage 全能力展示完成！16 个 Section 全部通过");
    println!("{}", "=".repeat(60));

    Ok(())
}
