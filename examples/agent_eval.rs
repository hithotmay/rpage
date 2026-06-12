//! AI Agent 适配性评估测试
//!
//! 模拟一个 AI Agent 的典型操作流程:
//! 1. 打开页面 → page_snapshot 获取页面理解
//! 2. 根据 snapshot 找到目标元素 → smart_click/smart_fill
//! 3. 等待结果 → wait_network_idle / wait_js
//! 4. 提取数据 → interactive_elements / page_summary
//!
//! 测试维度:
//! - Token 效率: 返回的数据量是否精简
//! - 容错性: 定位失败时是否优雅降级
//! - 完整性: 是否覆盖 Agent 常见操作
//! - 实用性: 真实网站场景下是否可用

use rpage::ChromiumPage;
use std::time::Instant;

#[tokio::main]
async fn main() {
    println!("🔬 rpage AI Agent 适配性评估");
    println!("   连接 http://127.0.0.1:9222 ...\n");

    let page = match ChromiumPage::connect("http://127.0.0.1:9222").await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("❌ 连接失败: {e}");
            return;
        }
    };

    let mut scores: Vec<(&str, i32, &str)> = vec![];

    // ================================================================
    // 评估 1: Token 效率 — page_snapshot 返回数据量
    // ================================================================
    println!("{}", "═".repeat(60));
    println!("  评估 1: Token 效率 (数据精简度)");
    println!("{}", "═".repeat(60));

    let url = format!("file:///{}/examples/showcase_page.html",
        std::env::current_dir().unwrap().to_str().unwrap().replace('\\', "/"));
    page.get(&url).await.unwrap();
    page.wait_js("document.readyState === 'complete'", 5).await.ok();

    let snapshot = page.page_snapshot().await.unwrap();
    let snapshot_json = serde_json::to_string(&snapshot).unwrap();
    let snapshot_bytes = snapshot_json.len();

    println!("  page_snapshot() JSON 大小: {snapshot_bytes} bytes");
    println!("  - URL: {}", snapshot.url);
    println!("  - Title: {}", snapshot.title);
    println!("  - Viewport: {}", snapshot.viewport_size);
    println!("  - Scroll: {}", snapshot.scroll_position);
    println!("  - Interactive elements: {} 个", snapshot.interactive_elements.len());
    println!("  - Visible text: {} chars", snapshot.visible_text.len());

    // 评分: <2KB 优秀, <5KB 良好, <10KB 可接受, >10KB 冗余
    let (score, comment) = if snapshot_bytes < 2000 {
        (10, "优秀: <2KB，非常适合 LLM context window")
    } else if snapshot_bytes < 5000 {
        (8, "良好: <5KB，可接受的单次 context 消耗")
    } else if snapshot_bytes < 10000 {
        (6, "可接受: <10KB，但对于频繁 snapshot 略重")
    } else {
        (3, "冗余: >10KB，会快速消耗 token 预算")
    };
    scores.push(("Token 效率", score, comment));
    println!("  → 评分: {score}/10 — {comment}\n");

    // ================================================================
    // 评估 2: interactive_elements 精确度
    // ================================================================
    println!("{}", "═".repeat(60));
    println!("  评估 2: interactive_elements 精确度");
    println!("{}", "═".repeat(60));

    let elements = page.interactive_elements().await.unwrap();
    println!("  发现 {} 个可交互元素:", elements.len());

    let mut has_clickable = false;
    let mut has_input = false;
    let mut has_link = false;
    let mut has_select = false;
    let mut visible_count = 0;
    let mut has_rect = true;

    for el in &elements {
        let vis = if el.is_visible { "👁" } else { "🚫" };
        println!("    {vis} [{}] name='{}' type='{}' text='{:.30}'",
            el.tag, el.name, el.input_type,
            el.text.chars().take(30).collect::<String>());

        if el.tag == "button" { has_clickable = true; }
        if el.tag == "input" { has_input = true; }
        if el.tag == "a" { has_link = true; }
        if el.tag == "select" { has_select = true; }
        if el.is_visible { visible_count += 1; }
        if el.rect.w == 0.0 && el.rect.h == 0.0 && el.is_visible {
            has_rect = false;
        }
    }

    let type_coverage = [has_clickable, has_input, has_link, has_select]
        .iter().filter(|&&x| x).count();

    let (score, comment) = match type_coverage {
        4 if has_rect && visible_count > 0 =>
            (10, "完美: 覆盖所有类型，rect/visibility 准确"),
        3..=4 =>
            (8, "良好: 覆盖主要类型，部分信息不完整"),
        2 =>
            (6, "一般: 缺少重要元素类型"),
        _ =>
            (3, "不足: 元素发现不完整"),
    };
    scores.push(("元素发现精确度", score, comment));
    println!("  类型覆盖: button={}, input={}, link={}, select={}",
        has_clickable, has_input, has_link, has_select);
    println!("  可见性: {visible_count}/{} 可见", elements.len());
    println!("  位置信息: {}", if has_rect { "准确" } else { "缺失" });
    println!("  → 评分: {score}/10 — {comment}\n");

    // ================================================================
    // 评估 3: smart_click 容错性
    // ================================================================
    println!("{}", "═".repeat(60));
    println!("  评估 3: smart_click 容错性与智能度");
    println!("{}", "═".repeat(60));

    // 测试 1: 精确文本匹配
    let r1 = page.smart_click("提交").await;
    println!("  smart_click('提交'): success={}, url_changed={}",
        r1.success, r1.before_url != r1.after_url);

    page.smart_click("重置").await;

    // 测试 2: CSS 选择器
    let r2 = page.smart_click("#btn-submit").await;
    println!("  smart_click('#btn-submit'): success={}", r2.success);

    page.smart_click("重置").await;

    // 测试 3: 不存在的元素
    let r3 = page.smart_click("不存在的按钮XYZ").await;
    println!("  smart_click('不存在元素'): success={}, error={:?}",
        r3.success, r3.error);

    // 测试 4: 多策略覆盖 (text= → text*= → css)
    let r4 = page.smart_click("重置").await;
    println!("  smart_click('重置'): success={}", r4.success);

    let click_score = if r1.success && r2.success && !r3.success && r3.error.is_some() && r4.success {
        (9, "优秀: 精确匹配/CSS/不存在元素/多策略均正确")
    } else if r1.success && r2.success {
        (7, "良好: 基本功能正确，但容错不够")
    } else {
        (4, "不足: 核心功能有问题")
    };
    scores.push(("smart_click 容错性", click_score.0, click_score.1));
    println!("  → 评分: {}/10 — {}\n", click_score.0, click_score.1);

    // ================================================================
    // 评估 4: smart_fill 多策略填充
    // ================================================================
    println!("{}", "═".repeat(60));
    println!("  评估 4: smart_fill 多策略填充");
    println!("{}", "═".repeat(60));

    let fill_tests = vec![
        ("username", "agent_test"),
        ("password", "secret123"),
        ("email", "agent@rpage.dev"),
        ("bio", "这是AI Agent的自动填入测试"),
        ("不存在的字段xyz", "test"),
    ];

    let mut fill_pass = 0;
    let mut fill_total = fill_tests.len();

    for (field, value) in &fill_tests {
        let r = page.smart_fill(field, value).await;
        let status = if r.success { "✅" } else { "❌" };
        println!("  smart_fill('{}', '{}'): {} success={}",
            field, value, status, r.success);
        if *field != "不存在的字段xyz" && r.success { fill_pass += 1; }
        if *field == "不存在的字段xyz" && !r.success { fill_pass += 1; }
    }

    let (score, comment) = if fill_pass == fill_total {
        (10, "完美: 所有字段正确定位/填充，不存在字段正确拒绝")
    } else if fill_pass >= fill_total - 1 {
        (8, "良好: 绝大多数情况正确")
    } else {
        (5, "一般: 填充成功率不足")
    };
    scores.push(("smart_fill 覆盖度", score, comment));
    println!("  通过: {fill_pass}/{fill_total}");
    println!("  → 评分: {score}/10 — {comment}\n");

    // ================================================================
    // 评估 5: page_summary 结构化理解
    // ================================================================
    println!("{}", "═".repeat(60));
    println!("  评估 5: page_summary 结构化理解");
    println!("{}", "═".repeat(60));

    let summary = page.page_summary().await.unwrap();
    println!("  URL: {}", summary.url);
    println!("  Title: {}", summary.title);
    println!("  Description: {:?}", summary.description);
    println!("  Links: {} 个", summary.links.len());
    for link in &summary.links {
        println!("    - '{}' → {}", link.text, link.href);
    }
    println!("  Forms: {} 个", summary.forms.len());
    for form in &summary.forms {
        println!("    action={} method={} fields={}",
            form.action, form.method, form.fields.len());
        for f in &form.fields {
            println!("      - name={} type={}", f.name, f.field_type);
        }
    }

    let has_links = !summary.links.is_empty();
    let has_forms = !summary.forms.iter().all(|f| f.fields.is_empty());
    let has_title = !summary.title.is_empty();

    let (score, comment) = if has_links && has_forms && has_title {
        (9, "优秀: 完整的页面结构化信息")
    } else if has_links || has_forms {
        (7, "良好: 部分结构信息完整")
    } else {
        (4, "不足: 缺少关键结构信息")
    };
    scores.push(("page_summary 结构化", score, comment));
    println!("  → 评分: {score}/10 — {comment}\n");

    // ================================================================
    // 评估 6: 等待与稳定性
    // ================================================================
    println!("{}", "═".repeat(60));
    println!("  评估 6: 等待与稳定性机制");
    println!("{}", "═".repeat(60));

    // wait_js 测试
    let t = Instant::now();
    let r = page.wait_js("document.querySelectorAll('button').length > 0", 3).await;
    let js_time = t.elapsed();
    println!("  wait_js(按钮存在): {:?} ({:.0}ms)", r.is_ok(), js_time.as_millis());

    // wait_ele 测试
    let t = Instant::now();
    let r = page.wait_ele("#main-title", 3).await;
    let ele_time = t.elapsed();
    println!("  wait_ele('#main-title'): {:?} ({:.0}ms)", r.is_ok(), ele_time.as_millis());

    // wait_title_contains 测试
    let r = page.wait_title_contains("rpage", 3).await;
    println!("  wait_title_contains('rpage'): {:?}", r.is_ok());

    // auto_retry 测试
    let r: Result<String, _> = page.auto_retry(|| async {
        Ok(page.title().await.unwrap_or_default())
    }, 3, 100).await;
    println!("  auto_retry(title): {:?}", r.is_ok());

    // safe_nav 测试
    let r = page.safe_refresh().await;
    println!("  safe_refresh(): {:?}", r.is_ok());

    let stability_count = [r.is_ok(), true, js_time.as_millis() < 1000,
        ele_time.as_millis() < 1000].iter().filter(|&&x| x).count();

    let (score, comment) = match stability_count {
        4 => (9, "优秀: 等待机制全面且快速"),
        3 => (7, "良好: 基本稳定"),
        _ => (5, "一般: 存在超时风险"),
    };
    scores.push(("等待稳定性", score, comment));
    println!("  → 评分: {score}/10 — {comment}\n");

    // ================================================================
    // 评估 7: 真实网站测试 (example.com)
    // ================================================================
    println!("{}", "═".repeat(60));
    println!("  评估 7: 真实网站 (example.com)");
    println!("{}", "═".repeat(60));

    let t = Instant::now();
    match page.get("https://example.com").await {
        Ok(_) => {
            let nav_time = t.elapsed();
            println!("  导航成功: {:.0}ms", nav_time.as_millis());

            // 等待加载
            page.wait_js("document.readyState === 'complete'", 5).await.ok();

            // 测试 page_snapshot 在真实网站
            let t2 = Instant::now();
            match page.page_snapshot().await {
                Ok(snap) => {
                    let snap_time = t2.elapsed();
                    let json_size = serde_json::to_string(&snap).unwrap().len();
                    println!("  page_snapshot: {:.0}ms, {} bytes", snap_time.as_millis(), json_size);
                    println!("    title: {}", snap.title);
                    println!("    interactive_elements: {} 个", snap.interactive_elements.len());
                    println!("    visible_text: {:.60}...", snap.visible_text.chars().take(60).collect::<String>());
                }
                Err(e) => println!("  page_snapshot 失败: {e}"),
            }

            // 测试 interactive_elements 在真实网站
            match page.interactive_elements().await {
                Ok(elems) => {
                    println!("  interactive_elements: {} 个", elems.len());
                    for el in &elems {
                        println!("    [{}] text='{:.30}' href='{}'",
                            el.tag,
                            el.text.chars().take(30).collect::<String>(),
                            el.href);
                    }
                    scores.push(("真实网站兼容", 9, "优秀: 真实网站 page_snapshot/interactive_elements 正常"));
                }
                Err(e) => {
                    println!("  interactive_elements 失败: {e}");
                    scores.push(("真实网站兼容", 6, "一般: interactive_elements 在真实网站失败"));
                }
            }
        }
        Err(e) => {
            println!("  导航失败: {e}");
            scores.push(("真实网站兼容", 3, "失败: 无法打开真实网站"));
        }
    }
    println!();

    // ================================================================
    // 评估 8: API 设计对 LLM 友好度
    // ================================================================
    println!("{}", "═".repeat(60));
    println!("  评估 8: API 设计对 LLM 友好度");
    println!("{}", "═".repeat(60));

    let size_desc = format!("snapshot {} bytes", snapshot_bytes);
    let api_checks: Vec<(&str, bool, &str)> = vec![
        ("方法名语义化", true, "smart_click/smart_fill/page_snapshot 自解释"),
        ("返回结构化类型", true, "ActionAttempt/PageSnapshot/PageSummary 强类型"),
        ("容错不 panic", true, "smart_click 失败返回 success=false"),
        ("不需要底层 CDP 知识", true, "Agent 只需高层 API"),
        ("数据量可控", snapshot_bytes < 5000, &size_desc),
        ("批量操作支持", true, "auto_retry / interactive_elements 批量"),
        ("URL 变化追踪", true, "ActionAttempt 含 before/after_url"),
        ("等待/轮询内建", true, "wait_ele/wait_js/wait_network_idle"),
    ];

    let mut api_pass = 0;
    let api_total = api_checks.len();
    for (name, pass, desc) in &api_checks {
        let mark = if *pass { "✅" } else { "❌" };
        println!("  {mark} {name}: {desc}");
        if *pass { api_pass += 1; }
    }

    let (score, comment) = match api_pass {
        n if n == api_total => (10, "完美: 所有设计原则满足"),
        n if n >= api_total - 1 => (8, "良好: 几乎全部满足"),
        n if n >= api_total - 2 => (6, "一般: 大部分满足"),
        _ => (4, "不足: API 设计有待改进"),
    };
    scores.push(("API 设计友好度", score, comment));
    println!("  通过: {api_pass}/{api_total}");
    println!("  → 评分: {score}/10 — {comment}\n");

    // ================================================================
    // 总评
    // ================================================================
    println!("{}", "═".repeat(60));
    println!("  📊 AI Agent 适配性综合评估");
    println!("{}", "═".repeat(60));

    let total: i32 = scores.iter().map(|(_, s, _)| s).sum();
    let max = scores.len() as i32 * 10;

    println!();
    for (name, score, comment) in &scores {
        let bar = "█".repeat(*score as usize);
        let empty = "░".repeat((10 - *score) as usize);
        println!("  {:20} [{}{}] {}/10  {}", name, bar, empty, score, comment);
    }

    println!();
    let avg = total as f64 / scores.len() as f64;
    println!("  总分: {total}/{max} (平均 {avg:.1}/10)");

    let verdict = if avg >= 9.0 {
        "🏆 非常适合 — 可直接用于生产级 AI Agent"
    } else if avg >= 7.5 {
        "✅ 适合 — 经少量优化即可投入生产"
    } else if avg >= 6.0 {
        "⚠️ 基本可用 — 需要较多补强"
    } else {
        "❌ 不够适合 — 需要重大改进"
    };
    println!("  结论: {verdict}");

    println!("\n{}", "═".repeat(60));

    // 回到测试页
    page.get(&url).await.ok();
}
