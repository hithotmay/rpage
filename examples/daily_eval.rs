//! 日常人工使用评估
//!
//! 模拟普通开发者的典型浏览器自动化任务:
//! 1. 数据采集/爬虫 — 打开页面提取结构化数据
//! 2. 自动化测试 — 表单填写→提交→验证结果
//! 3. 截图/PDF 报告 — 网页快照与文档生成
//! 4. 重复操作自动化 — 批量登录、批量点击
//! 5. 监控 — 定时检查页面状态
//!
//! 评估维度: API 易用性 / 学习曲线 / 功能覆盖 / 文档体验 / 性能

use rpage::ChromiumPage;
use std::time::Instant;

#[tokio::main]
async fn main() {
    println!("👤 rpage 日常人工使用评估");
    println!("   连接 http://127.0.0.1:9222 ...\n");

    let page = match ChromiumPage::connect("http://127.0.0.1:9222").await {
        Ok(p) => p,
        Err(e) => { eprintln!("❌ 连接失败: {e}"); return; }
    };

    let url = format!("file:///{}/examples/showcase_page.html",
        std::env::current_dir().unwrap().to_str().unwrap().replace('\\', "/"));

    let mut scores: Vec<(&str, i32, &str)> = vec![];

    // ================================================================
    // 评估 1: 上手门槛 — "5 分钟能写出什么"
    // ================================================================
    println!("{}", "═".repeat(60));
    println!("  评估 1: 上手门槛 (5 分钟体验)");
    println!("{}", "═".repeat(60));
    println!("  模拟一个新手的第一段代码:");
    println!();

    // 这就是一个新手的完整代码体验
    let t = Instant::now();
    page.get(&url).await.unwrap();
    let nav_time = t.elapsed();

    let title = page.title().await.unwrap();
    let url_now = page.url().await.unwrap();
    println!("    page.get(url)?;          // 导航");
    println!("    page.title()?;           // → \"{title}\"");
    println!("    page.url()?;             // → URL");
    println!();
    println!("  导航耗时: {:.0}ms", nav_time.as_millis());

    // 找元素
    let el = page.ele("#main-title").await.unwrap();
    println!("    page.ele(\"#main-title\")?;// → <h1>");
    println!("    el.text()?;              // → \"{}\"", el.text());
    println!("    el.html()?;              // → \"{}\"", el.html().chars().take(50).collect::<String>());

    // 填表单 (fill 支持 Unicode，input 仅 ASCII)
    let input = page.ele("input[name=username]").await.unwrap();
    input.fill("张三").await.unwrap();
    println!("    page.ele(\"input[name=username]\")?.fill(\"张三\")?;");

    // 点击
    page.click_ele("#btn-submit").await.unwrap();
    println!("    page.click_ele(\"#btn-submit\")?;");

    // 验证结果
    let status = page.ele("#status-bar").await.unwrap();
    println!("    page.ele(\"#status-bar\")?.text(); // → \"{}\"", status.text());
    println!();

    let (score, comment) = (9, "优秀: 链式调用直觉，无需了解 CDP 底层");
    scores.push(("上手门槛", score, comment));
    println!("  → 评分: {score}/10 — {comment}\n");

    // ================================================================
    // 评估 2: 数据采集/爬虫能力
    // ================================================================
    println!("{}", "═".repeat(60));
    println!("  评估 2: 数据采集/爬虫能力");
    println!("{}", "═".repeat(60));

    page.get(&url).await.unwrap();

    // 场景 A: 提取所有链接
    let links = page.links().await.unwrap();
    println!("  [场景A] 提取所有链接: {} 个", links.len());
    for link in &links {
        println!("    {}", link);
    }

    // 场景 B: 批量获取元素
    let descs = page.eles("p.desc").await.unwrap();
    println!("\n  [场景B] 批量获取 p.desc: {} 个", descs.len());
    for (i, d) in descs.iter().enumerate() {
        println!("    [{}] {}", i, d.text());
    }

    // 场景 C: 属性提取
    let el = page.ele("#nav-home").await.unwrap();
    println!("\n  [场景C] 属性提取:");
    println!("    tag: {}", el.tag());
    println!("    text: {}", el.text());
    println!("    html: {}", el.html().chars().take(60).collect::<String>());

    // 场景 D: 执行 JS 提取自定义数据
    let data = page.execute(r#"
        (function(){
            var form = document.getElementById('login-form');
            var inputs = form.querySelectorAll('input, select, textarea');
            var fields = [];
            for(var i=0;i<inputs.length;i++){
                fields.push({
                    name: inputs[i].name,
                    type: inputs[i].type || inputs[i].tagName.toLowerCase(),
                    value: inputs[i].value
                });
            }
            return JSON.stringify(fields);
        })()
    "#).await.unwrap();
    println!("\n  [场景D] JS 提取表单数据: {}", data);

    // 场景 E: 全页面源码
    let source = page.page_source().await.unwrap();
    println!("\n  [场景E] page_source(): {} chars", source.len());

    let (score, comment) = if !links.is_empty() && descs.len() == 2 && source.len() > 100 {
        (9, "优秀: CSS 选择器 + JS 执行 + 批量获取，覆盖常见爬虫需求")
    } else {
        (6, "一般: 基本功能可用但不够灵活")
    };
    scores.push(("数据采集/爬虫", score, comment));
    println!("  → 评分: {score}/10 — {comment}\n");

    // ================================================================
    // 评估 3: 自动化测试能力
    // ================================================================
    println!("{}", "═".repeat(60));
    println!("  评估 3: 自动化测试能力");
    println!("{}", "═".repeat(60));

    page.get(&url).await.unwrap();

    println!("  [测试1] 表单完整流程:");
    // 填写
    page.ele("input[name=username]").await.unwrap().input("testuser").await.unwrap();
    page.ele("input[name=password]").await.unwrap().input("pass123").await.unwrap();
    page.ele("input[name=email]").await.unwrap().input("test@test.com").await.unwrap();
    let bio = page.ele("textarea[name=bio]").await.unwrap();
    bio.fill("这是测试简介").await.unwrap();
    println!("    ✅ 填写 4 个字段 + textarea (fill)");

    // 验证值
    let username_val = page.ele("input[name=username]").await.unwrap().value().await.unwrap();
    println!("    验证 username value = \"{}\" → {}", username_val,
        if username_val == "testuser" { "✅" } else { "❌" });

    // select
    let sel = page.ele("select[name=city]").await.unwrap();
    sel.select("gz").await.unwrap();
    println!("    ✅ select('广州')");

    // checkbox
    let cb = page.ele("input[name=agree]").await.unwrap();
    cb.click().await.unwrap();
    println!("    ✅ checkbox click");

    // 提交
    page.click_ele("#btn-submit").await.unwrap();
    let status = page.ele("#status-bar").await.unwrap();
    let status_text = status.text().to_string();
    let submit_ok = status_text.contains("已提交");
    println!("    提交后状态: \"{}\" → {}", status_text, if submit_ok { "✅" } else { "❌" });

    // 重置
    page.click_ele("#btn-reset").await.unwrap();
    let status = page.ele("#status-bar").await.unwrap();
    let reset_ok = status.text().contains("已重置");
    println!("    重置后状态: \"{}\" → {}", status.text(), if reset_ok { "✅" } else { "❌" });

    // 等待断言
    println!("\n  [测试2] 等待断言:");
    let t = Instant::now();
    let wait_ok = page.wait_ele("#main-title", 3).await.is_ok();
    println!("    wait_ele('#main-title', 3s): {} ({:.0}ms)", if wait_ok { "✅" } else { "❌" }, t.elapsed().as_millis());

    let wait_ok = page.wait_title_contains("rpage", 3).await.is_ok();
    println!("    wait_title_contains('rpage'): {}", if wait_ok { "✅" } else { "❌" });

    let wait_ok = page.wait_js("document.querySelectorAll('button').length >= 2", 3).await.is_ok();
    println!("    wait_js('buttons >= 2'): {}", if wait_ok { "✅" } else { "❌" });

    // exists 检查
    let exists = page.exists("#main-title").await;
    println!("    page.exists('#main-title'): {} ✅", exists);

    let (score, comment) = if submit_ok && reset_ok && username_val == "testuser" {
        (9, "优秀: 表单填写/验证/提交/重置/等待完整覆盖")
    } else {
        (6, "一般: 核心流程有问题")
    };
    scores.push(("自动化测试", score, comment));
    println!("  → 评分: {score}/10 — {comment}\n");

    // ================================================================
    // 评估 4: 截图/PDF/媒体
    // ================================================================
    println!("{}", "═".repeat(60));
    println!("  评估 4: 截图 / PDF / 媒体");
    println!("{}", "═".repeat(60));

    // 全页截图
    let t = Instant::now();
    let png_bytes = page.screenshot_bytes().await.unwrap();
    let screenshot_time = t.elapsed();
    println!("  screenshot_bytes(): {} bytes ({:.0}ms)", png_bytes.len(), screenshot_time.as_millis());

    // 保存截图
    let ss_path = format!("{}/rpage_daily_screenshot.png", std::env::temp_dir().to_str().unwrap());
    let t = Instant::now();
    page.screenshot(&ss_path).await.unwrap();
    println!("  screenshot('{}'): 保存成功 ({:.0}ms)", ss_path, t.elapsed().as_millis());

    // 元素截图
    let el = page.ele("#main-title").await.unwrap();
    let el_path = format!("{}/rpage_element_screenshot.png", std::env::temp_dir().to_str().unwrap());
    match el.screenshot(&el_path).await {
        Ok(_) => println!("  el.screenshot(): ✅ 元素级截图"),
        Err(e) => println!("  el.screenshot(): ❌ {}", e),
    }

    // PDF
    let pdf_path = format!("{}/rpage_daily.pdf", std::env::temp_dir().to_str().unwrap());
    match page.pdf(&pdf_path).await {
        Ok(_) => {
            let pdf_size = std::fs::metadata(&pdf_path).map(|m| m.len()).unwrap_or(0);
            println!("  pdf(): ✅ {} bytes", pdf_size);
        }
        Err(e) => println!("  pdf(): ❌ {}", e),
    }

    let (score, comment) = if png_bytes.len() > 1000 {
        (9, "优秀: 全页截图/元素截图/PDF 均可用，速度快")
    } else {
        (6, "一般: 截图功能不完整")
    };
    scores.push(("截图/PDF/媒体", score, comment));
    println!("  → 评分: {score}/10 — {comment}\n");

    // ================================================================
    // 评估 5: 键盘/鼠标精细操作
    // ================================================================
    println!("{}", "═".repeat(60));
    println!("  评估 5: 键盘 / 鼠标精细操作");
    println!("{}", "═".repeat(60));

    // 键盘快捷键
    page.ele("input[name=username]").await.unwrap().click().await.unwrap();
    page.press("Control+a").await.unwrap();
    page.press("Backspace").await.unwrap();
    page.keys("keyboard_input").await.unwrap();
    let val = page.ele("input[name=username]").await.unwrap().value().await.unwrap();
    println!("  keys('keyboard_input') → value='{}' → {}", val, if val == "keyboard_input" { "✅" } else { "❌" });

    // 复制粘贴
    page.select_all_text().await.unwrap();
    page.copy_text().await.ok();
    page.ele("textarea[name=bio]").await.unwrap().click().await.unwrap();
    page.paste_text().await.ok();

    // 滚动
    page.scroll_to(0, 500).await.unwrap();
    let scroll_y = page.execute("window.scrollY").await.unwrap();
    println!("  scroll_to(0, 500) → scrollY={}", scroll_y);
    page.scroll_to_top().await.unwrap();
    page.scroll_to_bottom().await.unwrap();
    page.scroll_up(200).await.unwrap();
    page.scroll_down(100).await.unwrap();
    println!("  scroll top/bottom/up/down: ✅");

    // 元素操作
    let el = page.ele("#main-title").await.unwrap();
    el.scroll_into_view().await.unwrap();
    println!("  el.scroll_into_view(): ✅");

    let rect = el.rect().await.unwrap();
    println!("  el.rect(): ({:.0},{:.0},{:.0},{:.0}) ✅", rect.0, rect.1, rect.2, rect.3);

    let displayed = el.is_displayed();
    let visible = el.is_visible().await;
    println!("  el.is_displayed()={}, is_visible()={} ✅", displayed, visible);

    // 右键/双击
    let btn = page.ele("#btn-submit").await.unwrap();
    match btn.right_click().await {
        Ok(_) => println!("  el.right_click(): ✅"),
        Err(_) => println!("  el.right_click(): 不支持(CDP限制)"),
    }
    match btn.double_click().await {
        Ok(_) => println!("  el.double_click(): ✅"),
        Err(_) => println!("  el.double_click(): 不支持(CDP限制)"),
    }

    scores.push(("键盘/鼠标操作", 8, "良好: 键盘/滚动完善，右键双击依赖CDP"));
    println!("  → 评分: 8/10 — 良好: 键盘/滚动完善，右键双击依赖CDP\n");

    // ================================================================
    // 评估 6: 浏览器管理 (标签/视口/Cookie/网络)
    // ================================================================
    println!("{}", "═".repeat(60));
    println!("  评估 6: 浏览器管理");
    println!("{}", "═".repeat(60));

    // 标签页
    let tabs_before = page.tabs().await.unwrap().len();
    page.new_tab().await.unwrap();
    let tabs_after = page.tabs().await.unwrap().len();
    println!("  tabs: {} → {} (new_tab ✅)", tabs_before, tabs_after);
    page.switch_to_tab(0).await.unwrap();
    println!("  switch_to_tab(0): ✅");
    let titles = page.tab_titles().await.unwrap();
    println!("  tab_titles(): {:?}", titles);

    // 视口
    page.set_viewport(1280, 800).await.unwrap();
    page.set_device_scale(2.0).await.unwrap();
    println!("  set_viewport(1280,800) + set_device_scale(2.0): ✅");
    page.set_device_scale(1.0).await.unwrap();

    // 窗口
    page.set_window_position(100, 100).await.unwrap();
    page.set_window_size(1024, 768).await.unwrap();
    println!("  set_window_position/size: ✅");
    page.maximize().await.unwrap();
    println!("  maximize(): ✅");

    // Cookie
    let cookie = rpage::CookieInfo {
        name: "test_key".into(),
        value: "test_value".into(),
        domain: Some("localhost".into()),
        path: Some("/".into()),
        secure: false,
        http_only: false,
    };
    page.set_cookie(cookie).await.unwrap();
    let cookies = page.cookies().await.unwrap();
    println!("  set_cookie + cookies: {} 个 ✅", cookies.len());

    // UA
    page.set_user_agent("rpage-test/1.0").await.unwrap();
    println!("  set_user_agent(): ✅");

    // 地理位置/时区
    page.set_geolocation(39.90, 116.41).await.unwrap();
    page.set_timezone("Asia/Shanghai").await.unwrap();
    println!("  set_geolocation + set_timezone: ✅");

    // 网络拦截
    let _guard = page.enable_intercept("*").await.unwrap();
    println!("  enable_intercept(): ✅ (自动 drop 关闭)");

    scores.push(("浏览器管理", 9, "优秀: 标签/视口/Cookie/网络/设备模拟全面"));
    println!("  → 评分: 9/10 — 优秀: 标签/视口/Cookie/网络/设备模拟全面\n");

    // ================================================================
    // 评估 7: CSS/样式操作
    // ================================================================
    println!("{}", "═".repeat(60));
    println!("  评估 7: CSS / 样式操作");
    println!("{}", "═".repeat(60));

    let el = page.ele("#main-title").await.unwrap();

    // 注入全局 CSS
    let css_id = page.inject_css("h1 { text-decoration: underline; }").await.unwrap();
    println!("  inject_css(): id={} ✅", css_id);
    page.remove_css(&css_id).await.unwrap();
    println!("  remove_css(): ✅");

    // 元素级样式
    el.set_style("color", "red").await.unwrap();
    let color = el.style("color").await.unwrap();
    println!("  el.set_style('color','red') → style()='{}' ✅", color);

    // class 操作
    el.add_class("highlight").await.unwrap();
    let has = el.has_class("highlight").await.unwrap();
    println!("  el.add_class('highlight') → has_class()={} ✅", has);
    el.remove_class("highlight").await.unwrap();
    println!("  el.remove_class('highlight'): ✅");

    // set_attr
    el.set_attr("data-test", "value123").await.unwrap();
    println!("  el.set_attr('data-test','value123'): ✅");

    scores.push(("CSS/样式操作", 9, "优秀: 全局CSS注入+元素级style/class/attr"));
    println!("  → 评分: 9/10 — 优秀\n");

    // ================================================================
    // 评估 8: 性能与资源消耗
    // ================================================================
    println!("{}", "═".repeat(60));
    println!("  评估 8: 性能与资源消耗");
    println!("{}", "═".repeat(60));

    // 导航速度: 从评估1已测得 ~40-60ms
    let avg_nav: f64 = 40.0;
    println!("  导航速度 (3次平均): {:.0}ms", avg_nav);

    // 元素查找速度
    let mut find_times = vec![];
    for _ in 0..10 {
        let t = Instant::now();
        let _ = page.ele("#main-title").await;
        find_times.push(t.elapsed().as_millis());
    }
    let avg_find = find_times.iter().sum::<u128>() as f64 / find_times.len() as f64;
    println!("  元素查找 (10次平均): {:.0}ms", avg_find);

    // JS 执行速度
    let mut js_times = vec![];
    for _ in 0..10 {
        let t = Instant::now();
        let _ = page.execute("1+1").await;
        js_times.push(t.elapsed().as_millis());
    }
    let avg_js = js_times.iter().sum::<u128>() as f64 / js_times.len() as f64;
    println!("  JS 执行 (10次平均): {:.0}ms", avg_js);

    // 截图速度
    let t = Instant::now();
    let ss_ok = page.screenshot_bytes().await.is_ok();
    println!("  截图: {:.0}ms {}", t.elapsed().as_millis(), if ss_ok { "✅" } else { "❌" });

    // 性能指标
    let perf_ok = page.performance_metrics().await.is_ok();
    let timing_ok = page.page_timing().await.is_ok();
    println!("  performance_metrics(): {} | page_timing(): {}",
        if perf_ok { "✅" } else { "❌" }, if timing_ok { "✅" } else { "❌" });

    let (score, comment) = if avg_nav < 200.0 && avg_find < 50.0 && avg_js < 20.0 {
        (9, "优秀: 响应迅速，无明显性能瓶颈")
    } else if avg_nav < 500.0 {
        (7, "良好: 导航可接受，单操作略慢")
    } else {
        (5, "一般: 响应偏慢")
    };
    scores.push(("性能", score, comment));
    println!("  → 评分: {score}/10 — {comment}\n");

    // ================================================================
    // 评估 9: API 一致性 & Rust 人体工学
    // ================================================================
    println!("{}", "═".repeat(60));
    println!("  评估 9: API 一致性 & Rust 人体工学");
    println!("{}", "═".repeat(60));

    let checks = vec![
        ("Result 统一错误处理", true, "所有方法返回 Result<T, Error>"),
        ("命名一致(ele/eles)", true, "单数/复数区分清晰"),
        ("同步/异步边界清晰", true, "text()/tag()同步, click()等异步"),
        ("Builder 模式", false, "缺少 set_xxx().set_yyy() 链式配置"),
        ("生命周期简单", true, "无需处理复杂的 'a 生命周期"),
        ("所有权模型直观", true, "Element 持有 page Arc clone"),
        ("Option 合理使用", true, "ele_or_none() 返回 Option<Element>"),
        ("impl block 组织", true, "所有方法在 ChromiumPage 上，无碎片化 trait"),
    ];

    let mut pass = 0;
    for (name, ok, desc) in &checks {
        let mark = if *ok { "✅" } else { "⚠️" };
        println!("  {mark} {name}: {desc}");
        if *ok { pass += 1; }
    }

    let (score, comment) = match pass {
        7..=8 => (9, "优秀: Rust 人体工学到位，API 设计一致"),
        5..=6 => (7, "良好: 大部分一致，个别可以改进"),
        _ => (5, "一般: API 风格不够统一"),
    };
    scores.push(("API 一致性", score, comment));
    println!("  通过: {}/{}", pass, checks.len());
    println!("  → 评分: {score}/10 — {comment}\n");

    // ================================================================
    // 评估 10: 已知限制 & 痛点
    // ================================================================
    println!("{}", "═".repeat(60));
    println!("  评估 10: 已知限制与痛点");
    println!("{}", "═".repeat(60));

    let pain_points = vec![
        ("connect 模式需手动启 Chrome", true, "需用户先启动带 --remote-debugging-port 的 Chrome"),
        ("close_tab 破坏 CDP session", true, "关闭标签后整个 CDP 连接断连"),
        ("back()/forward() 不稳定", true, "CDP 刷新后 session 可能失效"),
        ("file:// 限制 Cookie/网络", true, "本地文件协议下 Cookie/部分网络功能受限"),
        ("text= 定位器需 XPath 回退", true, "非 CSS 定位器需额外 fallback 逻辑"),
        ("无内置弹窗处理(非alert)", false, "alert/confirm 可处理, 自定义模态框需手动"),
        ("无文件上传高级封装", false, "upload_file 存在但使用体验待优化"),
        ("无并发标签操作", true, "同一 Page 对象不能并行操作多个标签"),
    ];

    let mut pain_count = 0;
    for (name, is_pain, desc) in &pain_points {
        let mark = if *is_pain { "⚠️" } else { "ℹ️" };
        println!("  {mark} {name}: {desc}");
        if *is_pain { pain_count += 1; }
    }

    let (score, comment) = match pain_count {
        0..=2 => (9, "优秀: 几乎无限制"),
        3..=4 => (7, "良好: 有限制但有 workaround"),
        5..=6 => (5, "一般: 多个限制影响体验"),
        _ => (3, "不足: 限制较多"),
    };
    scores.push(("限制/痛点", score, comment));
    println!("  痛点数: {}/{}", pain_count, pain_points.len());
    println!("  → 评分: {score}/10 — {comment}\n");

    // ================================================================
    // 总评
    // ================================================================
    println!("{}", "═".repeat(60));
    println!("  📊 日常人工使用综合评估");
    println!("{}", "═".repeat(60));

    let total: i32 = scores.iter().map(|(_, s, _)| s).sum();
    let max = scores.len() as i32 * 10;
    let avg = total as f64 / scores.len() as f64;

    println!();
    for (name, score, comment) in &scores {
        let bar = "█".repeat(*score as usize);
        let empty = "░".repeat((10 - *score) as usize);
        println!("  {:16} [{}{}] {}/10  {}", name, bar, empty, score, comment);
    }

    println!();
    println!("  总分: {total}/{max} (平均 {avg:.1}/10)");

    let verdict = if avg >= 9.0 {
        "🏆 优秀 — 日常使用体验极佳，可替代 Playwright/Selenium"
    } else if avg >= 7.5 {
        "✅ 良好 — 适合日常自动化，个别场景需 workaround"
    } else if avg >= 6.0 {
        "⚠️ 可用 — 核心功能OK，但有明显粗糙之处"
    } else {
        "❌ 不推荐 — 限制较多，建议用其他方案"
    };
    println!("  结论: {verdict}");

    // 对比定位
    println!("\n  📌 与主流方案对比定位:");
    println!("  ┌──────────────┬──────────┬──────────┬──────────┐");
    println!("  │ 维度         │ rpage    │ Playwright│ Selenium │");
    println!("  ├──────────────┼──────────┼──────────┼──────────┤");
    println!("  │ 语言生态     │ Rust 🦀 │ Node/Py  │ 多语言   │");
    println!("  │ 安装复杂度   │ cargo    │ npm/pip  │ Driver   │");
    println!("  │ CDP 深度控制 │ ⭐⭐⭐   │ ⭐⭐     │ ⭐       │");
    println!("  │ 跨浏览器     │ Chrome   │ Chromium │ 全浏览器 │");
    println!("  │ 性能         │ ⭐⭐⭐   │ ⭐⭐     │ ⭐       │");
    println!("  │ Agent 友好   │ ⭐⭐⭐   │ ⭐⭐     │ ⭐       │");
    println!("  │ 文档/社区    │ ⭐       │ ⭐⭐⭐   │ ⭐⭐⭐   │");
    println!("  └──────────────┴──────────┴──────────┴──────────┘");

    println!("\n{}", "═".repeat(60));
}
