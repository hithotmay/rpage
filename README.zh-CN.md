# rpage 🦀🌐

[English](README.md) | **简体中文**

> Rust 版 DrissionPage — 浏览器自动化 + HTTP 会话 + Cookie 互通，三合一。**580+ 公开方法，12,000+ 行 Rust**。

`rpage` 是一个受 [DrissionPage](https://github.com/g1879/DrissionPage) 启发的 Rust 浏览器自动化库。

## ✨ 核心特性

- **一个函数启动** — `WebPage::new()` 自动检测 Chrome → 启动子进程 → CDP 连接
- **零自动化标记** — 不传 `--enable-automation`，永不触发验证码
- **智能标签接管** — `connect()` 自动选择当前激活标签页，不会误开新标签
- **自动反检测** — stealth 脚本自动注入
- **Cookie 互通** — 浏览器 ↔ HTTP 会话共享 Cookie，支持导入/导出文件
- **智能等待** — `get()` 等加载，`ele()`/`eles()` 自动重试，超时可配置
- **中文完美** — `fill()` 用 JS `nativeInputValueSetter`
- **鲁棒交互** — `click()` 自动 fallback CDP→JS，拖拽到元素或坐标
- **链式调用** — `page.goto(url).await?.type_text("#kw", "rust").await?` 一行完成导航+输入
- **一步到位 API** — `click_ele()`/`get_text()`/`get_attr()` 无需先 `ele()` 再操作
- **网络包实时监听** — `on_request()`/`on_response()` 回调式捕获每个请求和响应
- **网络监听模式** — listen_start/stop + wait_data_packet 抓包；请求级控制（继续/重定向/拒绝）走 InterceptGuard
- **设备模拟** — 设备像素比/触摸模式/地理位置+刷新
- **资源分析** — links/images/ele_count 一键提取页面资源
- **网络控制** — URL 拦截/离线模式/清除缓存/禁用图片
- **标签页增强** — tab_to_front/get_tab_by_title/get_tab_by_url/wait_new_tab
- **Tab 管理** — 创建/切换/关闭/列表
- **条件等待** — 等元素出现/消失/删除，等标题/URL 变化，等 JS 表达式
- **Element 等待** — wait_for_visible/hidden/stale/clickable/enabled/text/attribute
- **运行时修改** — headers/user_agent/viewport
- **窗口管理** — maximize/minimize/fullscreen/restore/set_size
- **加载策略** — normal/eager/none 三种页面加载模式
- **事件回调** — on_dialog/on_load/on_close 事件监听
- **批量操作** — `eles().texts()` 一行获取所有文本
- **文件上传** — 浏览器 + Session multipart 双模式
- **PDF/截图** — 页面级和元素级
- **下载管理** — CDP 下载事件监听 + 等待下载完成
- **网络拦截** — Fetch.enable 拦截/修改/阻止请求
- **ActionChain** — 复杂鼠标键盘序列
- **iframe 上下文** — 进入 iframe 执行操作
- **网络监控** — 记录所有请求/响应/失败
- **Console 捕获** — 拦截 console.log/warn/error
- **WebSocket 监听** — 监听 WebSocket 帧
- **性能指标** — 页面计时 + JS Heap 等运行时指标
- **Init Script** — 页面加载前注入 JS
- **CSS 注入** — 动态注入/移除 CSS 样式
- **灵活配置** — 环境变量 `RPAGE_CHROME_PATH`，PATH 搜索，标准路径
- **同步 API** — `SyncPage` 零 await 封装，`SyncPage::connect()` 一步到位，自动接管当前激活标签页
- **Agent 智能接口** — `interactive_elements()`/`page_snapshot()`/`smart_click()`/`smart_fill()` 直接给 AI Agent 用
- **JS XPath 回退** — 非 CSS 定位器（`text:`/`tag:`）自动回退 XPath
- **原始 CDP 直通** — `run_cdp()` 执行任意 CDP 命令；`get_response_body()` 抓响应体（含 base64 解码）
- **元素状态** — `is_in_viewport()` / `is_alive()`，对标 DrissionPage `states`
- **语义定位** — `get_by_role/text/label/placeholder/test_id`，对标 Playwright
- **响应 mock** — `InterceptGuard::fulfill_request/fulfill_json` 伪造响应，对标 Playwright `route.fulfill`
- **多条件定位** — `@@` 同元素 AND / `@|` OR，对标 DrissionPage
- **持续抓包** — `data_packets()` 一次取所有匹配的完整 DataPacket（含 body）
- **自动可操作性等待** — `click()` 前自动等 visible+enabled，对标 Playwright actionability
- **DrissionPage 定位器** — 文本/属性运算符 `=`/`:`/`^`/`$`（精确/包含/前缀/后缀）全支持

## 🚀 快速开始

```rust
use rpage::prelude::*;

#[tokio::main]
async fn main() -> rpage::Result<()> {
    let page = WebPage::new().await?;           // 一行启动 Chrome
    page.get("https://www.baidu.com").await?;   // 自动等待加载
    page.ele("#kw").await?.fill("rust教程").await?;  // 中文输入
    page.ele("#su").await?.click().await?;       // 搜索
    let results = page.eles("h3").await?;       // 自动重试等待
    for (i, text) in results.texts().iter().enumerate() {
        println!("{}. {}", i + 1, text);
    }
    Ok(())
}
```

### 链式调用 — 一行完成复杂操作

```rust
// 链式：导航 → 输入 → 点击，一步到位
page.goto("https://www.baidu.com").await?
    .type_text("#kw", "rust教程").await?
    .click_ele("#su").await?;
```

### 同步 API — 零 await，零 tokio

`SyncPage` 将全部 async API 封装为同步调用，无需 `#[tokio::main]`：

```rust
use rpage::sync::SyncPage;

fn main() -> rpage::Result<()> {
    let p = SyncPage::connect("http://127.0.0.1:9222")?;  // 自动接管当前激活标签页

    p.get("https://httpbin.org/forms/post")?;
    p.type_text("input[name='custname']", "张三")?;
    p.click_ele("input[value='medium']")?;
    p.screenshot("form.png")?;

    let title = p.title()?;
    let cookies = p.cookies()?;
    let links = p.links()?;
    println!("title={}, {} cookies, {} links", title, cookies.len(), links.len());
    Ok(())
}
```

### Agent 智能接口 — 为 AI Agent 设计

```rust
// 1. 分析页面交互元素
let elements = p.interactive_elements()?;
for el in &elements {
    println!("{:?} {} type={:?}", el.tag, el.text, el.input_type);
}

// 2. 页面快照（给 LLM 上下文）
let snap = p.page_snapshot()?;
// snap.url, snap.title, snap.viewport_size, snap.interactive_elements, snap.visible_text

// 3. 智能操作 — 用自然语言描述即可
let result = p.smart_click("提交");
let result = p.smart_fill("用户名", "agent_bot");

// 4. DOM 快照（结构化 JSON）
let dom = p.dom_snapshot()?;

// 5. 性能指标
let metrics = p.performance_metrics()?;
```

### 能力展示

运行 `cargo run --example showcase` 查看 **10 大场景 50+ 项操作** 的完整演示：

```
 1. 页面导航 & DOM 操作     — 导航、表单填写、复选框/单选框、截图
 2. JavaScript 执行         — execute、run_async_js、run_js_with_args、全局注入
 3. 元素高级操作            — DOM 树遍历、属性/样式操作、拖拽
 4. 多标签页管理            — new_tab、switch_to_tab、跨标签导航
 5. Cookie 管理             — 读取、设置、删除、跨请求验证
 6. 网络控制                — UA伪装、自定义Headers、URL屏蔽
 7. 截图 & PDF              — 全页截图、元素截图、PDF生成
 8. 智能等待                — wait_ele、wait_js、wait_url_contains
 9. 设备模拟 & 视口         — viewport、device_scale、touch、geolocation、timezone
10. Agent 智能接口          — interactive_elements、page_snapshot、smart_click/fill
```

## 📦 安装

```toml
[dependencies]
rpage = "1"
```

## 📖 API 参考 (568 方法)

### 链式便捷 API (8)

无需先 `ele()` 再操作，一步到位。所有方法返回 `&Self`，支持链式调用。

```rust
page.goto(url).await?;                       // 导航（返回 &Self）
page.type_text("#kw", "rust教程").await?;    // 定位+清空+输入
page.click_ele("#btn").await?;               // 定位+点击
page.get_text("h1").await?;                  // 定位+获取文本
page.get_attr("#link", "href").await?;       // 定位+获取属性
page.input_text("#input", "追加文本").await?; // 定位+追加输入
page.hover_ele("#menu").await?;              // 定位+悬停
page.scroll_to_ele("#section").await?;       // 定位+滚动到元素

// 链式组合示例
page.goto("https://example.com").await?
    .type_text("#search", "query").await?
    .click_ele("#submit").await?;
```

### 快速查询 (3)

```rust
let exists = page.exists("#result").await?;     // bool — 元素是否存在
let n = page.count("li.item").await?;           // usize — 匹配数量
let el = page.ele_or_none("#opt").await?;       // Option<Element> — 无则 None
```

### DrissionPage 别名 (3)

兼容 DrissionPage 命名习惯：

```rust
let url = page.current_url().await?;    // 等同 page.url()
let title = page.current_title().await?; // 等同 page.title()
let src = page.page_source().await?;    // 等同 page.html()
```

### 页面导航 + 信息 (9)

```rust
page.get(url).await?;           // 自动等 DOMContentLoaded
page.refresh().await?;          // 智能等待
page.back().await?;
page.forward().await?;
let title = page.title().await?;
let url = page.url().await?;
let html = page.html().await?;
let val = page.execute("1+1").await?;
page.evaluate_on_new_document("...").await?;
```

### 元素定位 — 自动重试 (2)

```rust
let el = page.ele("#id").await?;
let els = page.eles("h3").await?;
```

文本/属性运算符对齐 DrissionPage：`=` 精确，`:` 包含，`^` 前缀，`$` 后缀。

| 语法 | 说明 |
|------|------|
| `#id`, `.class` | CSS |
| `@class=btn` | 属性精确 |
| `@class:btn`（或 `@class*=btn`） | 属性包含 |
| `@class^btn` | 属性前缀 |
| `@class$btn` | 属性后缀 |
| `text=登录` | 文本精确 |
| `text:登录`（或 `text*=登录`） | 文本包含 |
| `text^登录` | 文本前缀 |
| `text$登录` | 文本后缀 |
| `tag:div@@class=x@@text:y` | 同元素多条件 AND |
| `@|class=a@|class=b` | 同元素多条件 OR |
| `a@@@b@@@c` | 后代链式（rpage 扩展） |

### 条件等待 (10)

```rust
let el = page.wait_ele("#result", 10).await?;
page.wait_ele_hidden("#loading", 5).await?;
page.wait_ele_deleted(".modal", 5).await?;
page.wait_title_contains("搜索结果", 5).await?;
page.wait_url_contains("search", 5).await?;
page.wait_js("document.querySelectorAll('.item').length > 5", 10).await?;
let dl = page.wait_download(30).await?;
// ── 精确匹配等待 ──
page.wait_url_is("https://example.com/done", 10).await?;
page.wait_title_is("完成", 10).await?;
page.wait_for_navigation(10).await?;
```

### 标签页 (10)

```rust
page.new_tab().await?;
let titles = page.tab_titles().await?;
let urls = page.tab_urls().await?;
page.switch_to_tab(1).await?;
page.close_tab(0).await?;
let tabs = page.tabs().await?;
// ── iter10 增强 ──
page.tab_to_front(0).await?;           // 将标签页置顶
let idx = page.get_tab_by_title("百度").await?;  // 按标题查找
let idx = page.get_tab_by_url("example").await?; // 按 URL 查找
page.wait_new_tab(5).await?;           // 等待新标签页
page.set_download_file_name("report.pdf").await?; // 设置下载文件名
```

### 元素操作 (55+ 方法)

```rust
// ── 基础交互 ──
el.click().await?;              // CDP → JS 自动 fallback
el.fill("rust教程").await?;    // 清空+输入
el.input("追加").await?;
el.clear().await?;
el.hover().await?;
el.submit().await?;
el.right_click().await?;
el.double_click().await?;
el.press_key("Enter").await?;

// ── 拖拽 ──
el.drag_to(&target).await?;     // 拖到另一个元素
el.drag_to_offset(100.0, 50.0).await?;  // 拖到相对坐标

// ── 下拉框 / 文件 ──
el.select("选项").await?;
el.select_by_value("val").await?;
el.upload_file("/path/to/file").await?;
el.upload_files(&["/a", "/b"]).await?;

// ── 复选框 ──
el.check().await?;
el.uncheck().await?;
let checked = el.checked();

// ── 截图 ──
el.screenshot("el.png").await?;
let bytes = el.screenshot_bytes().await?;

// ── 属性 / 状态 ──
let v = el.attr("href");           // Option<&str>
let v = el.value().await?;
let (x,y,w,h) = el.rect().await?;
let bbox = el.bounding_box().await?;  // CDP + JS fallback
let s = el.style("color").await?;
el.set_attr("class", "active").await?;
el.is_displayed();                  // 同步
el.is_visible().await?;            // 异步，CSS+几何双重检测
el.is_enabled();
el.is_selected().await?;

// ── 相对定位 ──
let p = el.parent().await?;
let c = el.first_child().await?;
let n = el.next().await?;
let pv = el.prev().await?;
let child = el.ele("a")?;
let children = el.eles("li")?;
let children = el.children()?;
let sib = el.sibling()?;
let inner = el.inner_html()?;
let outer = el.outer_html()?;

// ── 焦点 / 选择 ──
el.focus().await?;
el.blur().await?;
el.select_text().await?;
el.scroll_into_view().await?;
el.scroll_to_top().await?;

// ── Shadow DOM ──
let shadow = el.shadow_ele("div").await?;
let shadows = el.shadow_eles("li").await?;

// ── JS ──
el.js("this.style.color='red'").await?;
```

### Element 等待方法 (40+)

```rust
// ── 可见性 ──
el.wait_for_visible().await?;
el.wait_for_visible_with_timeout(Duration::from_secs(10)).await?;
el.wait_for_hidden().await?;
el.wait_for_hidden_with_timeout(Duration::from_secs(5)).await?;

// ── 状态 ──
el.wait_for_enabled().await?;
el.wait_for_clickable().await?;
el.wait_for_stale().await?;

// ── 文本 ──
el.wait_for_text("加载完成").await?;
el.wait_for_text_eq("精确匹配").await?;
el.wait_for_text_contains("部分").await?;

// ── 属性 ──
el.wait_for_attribute("class", "active").await?;
el.wait_for_attribute_contains("class", "act").await?;

// ── 完全自定义 ──
let opts = WaitOptions::new(Duration::from_secs(30), Duration::from_millis(200));
el.wait_for_visible_with_options(opts).await?;
```

### 批量操作 — ElementBatch

```rust
let els = page.eles("h3").await?;
let texts = els.texts();                // Vec<&str>
let hrefs = els.attr_values("href");    // Vec<Option<&str>>
let visible = els.displayed();          // Vec<&Element>
```

### 页面操作 (20+)

```rust
page.scroll_to(0, 500).await?;
page.scroll_by(0, 300).await?;          // 相对滚动
page.scroll_to_top().await?;
page.scroll_to_bottom().await?;
page.screenshot("shot.png").await?;
page.pdf("page.pdf").await?;
page.press("Enter").await?;
page.set_viewport(1920, 1080).await?;
page.handle_alert(true, None).await?;
page.frame_html("iframe").await?;
page.frame_execute("iframe", "document.title").await?;
page.scroll_to_element(&el).await?;
page.run_async_js("await fetch('/api')").await?;
page.keys("hello world").await?;        // 逐字符输入（支持中文）
page.refresh_ele(&el).await?;           // 重新定位元素
```

### 窗口管理 (7)

```rust
page.maximize().await?;
page.minimize().await?;
page.fullscreen().await?;
page.restore().await?;
page.set_window_size(1280, 800).await?;
let (l, t, w, h) = page.get_window_bounds().await?;
```

### 加载策略

```rust
page.set_load_strategy("eager").await?;  // normal / eager / none
let strategy = page.load_strategy();      // &str
```

### 事件回调 (3)

```rust
page.on_dialog(|msg, r#type| {
    println!("Dialog: {} ({})", msg, r#type);
    true  // accept
}).await?;

page.on_load(|url| {
    println!("Page loaded: {}", url);
}).await?;

page.on_close(|| {
    println!("Page closed");
}).await?;
```

### Cookie (10)

```rust
let cookies = page.cookies().await?;
page.set_cookie(cookie).await?;
page.delete_cookie("name").await?;
page.clear_cookies().await?;
page.sync_cookies().await?;
page.save_cookies_to_file("cookies.json").await?;
page.load_cookies_from_file("cookies.json").await?;
```

### 运行时修改 (3)

```rust
page.set_extra_headers(headers).await?;
page.set_user_agent("Mozilla/5.0 ...").await?;
page.set_viewport(1920, 1080).await?;
```

### Session HTTP (7)

```rust
let resp = page.session_get(url).await?;
let resp = page.session_post(url, body).await?;
let resp = page.session_put(url, body).await?;
let resp = page.session_delete(url).await?;
let resp = page.session_head(url).await?;
let resp = page.session_patch(url, body).await?;
let resp = page.session_post_json(url, &data).await?;  // JSON body
```

此外还有：

```rust
page.post(url, body).await?;
page.post_multipart(url, fields, "file", "/path").await?;
page.post_json(url, data).await?;
page.session_download(url, "file.zip").await?;
let status = page.session_status(url).await?;
```

### 网络包实时监听 (3)

回调式捕获浏览器每个请求和响应：

```rust
// 监听请求
page.on_request(|req| {
    println!("→ {} {}", req.method, req.url);
}).await?;

// 监听响应
page.on_response(|resp| {
    println!("← {} {}", resp.status, resp.url);
}).await?;

// 清除所有监听器
page.clear_listeners().await?;
```

### Console 捕获 (3)

```rust
page.execute("console.log('hello')").await?;
let logs = page.console_log();     // Vec<ConsoleMessage>
page.clear_console();
```

### Init Script + CSS 注入 (4)

```rust
let id = page.add_init_script("my", "window.test = 42").await?;
page.remove_init_script(&id).await?;

let css_id = page.inject_css("body { background: red }").await?;
page.remove_css(&css_id).await?;
```

### 性能指标 (3)

```rust
let timing = page.page_timing().await?;     // HashMap<String, f64>
let metrics = page.performance_metrics().await?;  // Vec<(String, f64)>
let snapshot = page.dom_snapshot().await?;   // serde_json::Value
```

### 下载管理 (4)

```rust
let downloads = page.downloads();           // 获取所有下载
let dl = page.wait_download(30).await?;    // 等待最近下载完成
println!("{:?}", dl.save_path);             // 下载路径
page.download_manager().completed();        // 已完成的下载
```

### 网络拦截 (3)

```rust
let guard = page.enable_intercept("*/api/*").await?;
tokio::time::sleep(Duration::from_secs(5)).await;
for req in guard.paused_requests() {
    guard.continue_request(req.request_id.as_ref(), None).await?;
}
guard.disable().await?;  // 或 drop(guard) 自动关闭
```

### ActionChain — 复杂操作序列 (8)

```rust
page.actions()
    .move_to(100.0, 200.0)
    .click_at(100.0, 200.0)
    .double_click_at(150.0, 200.0)
    .right_click_at(200.0, 200.0)
    .key_down("Control")
    .press("a")
    .key_up("Control")
    .pause(Duration::from_millis(500))
    .perform()
    .await?;
```

### iframe 上下文 (4)

```rust
let ctx = page.enter_frame("iframe").await?;
ctx.execute("document.title").await?;
let el = ctx.ele("#btn").await?;
let html = ctx.html().await?;
```

### 网络监控 (5)

```rust
page.network_monitor().requests();          // 所有请求
page.network_monitor().responses();         // 所有响应
page.network_monitor().failures();          // 失败请求
page.network_monitor().find_requests_by_url("api");
page.network_monitor().clear();
```

### WebSocket 监听 (2)

```rust
page.listen_websocket().await?;
let frames = page.websocket_frames();
```

### 原始 CDP + 响应体 (DrissionPage `run_cdp` 对标)

```rust
// 任意 CDP 命令直通 —— rpage 没封装的命令都能用
let m = page.run_cdp("Page.getLayoutMetrics", serde_json::json!({})).await?;
page.run_cdp("Network.setUserAgentOverride",
    serde_json::json!({ "userAgent": "my-bot/1.0" })).await?;

// 抓响应体（配合 on_response / get_responses 拿到 request_id）
for r in page.get_responses("/api/") {
    let body = page.get_response_body(&r.request_id).await?;
    println!("{} -> {}", r.url, body.text());   // base64 自动解码用 body.bytes()
}
```

### 监听抓包 DataPacket (DrissionPage `listen.wait()` 对标)

一步等待匹配 URL 的响应并抓取完整请求/响应（含响应体）：

```rust
page.listen_start().await?;
page.ele("#load").await?.click().await?;     // 触发 XHR / fetch
let pkt = page.wait_data_packet("/api/data", 10).await?;          // 等一个
println!("{} {} -> {}", pkt.status, pkt.url, pkt.body_text());
// 或一次取所有匹配的完整包（对标 listen.steps()）
for p in page.data_packets("/api/").await { println!("{}", p.body_text()); }
// pkt.method / status / mime_type / request_headers / response_headers / response_body
```

### 静态快照元素 (DrissionPage `s_ele` 对标)

一次解析当前页面 HTML，返回不依赖实时 CDP 的静态元素，批量只读提取更快：

```rust
let title = page.s_ele("#title").await?;       // 静态查找
let rows = page.s_eles("table tr").await?;      // 批量，快
for row in &rows { println!("{}", row.text()); }
```

### 元素状态 (DrissionPage `states` 对标)

```rust
let el = page.ele("#item").await?;
el.is_in_viewport().await;   // 是否在可视区内
el.is_alive().await;          // 是否仍挂在 DOM 上
```

### 语义定位 (Playwright `get_by_*` 对标)

```rust
page.get_by_text("登录").await?;          // 可见文本（包含）
page.get_by_role("button").await?;        // ARIA role（含 button/link/textbox… 隐含 role）
page.get_by_label("用户名").await?;       // 关联 label（aria-label / <label for> / 包裹）
page.get_by_placeholder("搜索").await?;   // placeholder（包含）
page.get_by_test_id("submit").await?;     // data-testid（精确）
```

### 响应 mock (Playwright `route.fulfill` 对标)

拦截请求并**伪造响应**，不走网络 —— 测试 / 反爬 / 离线复现：

```rust
let guard = page.enable_intercept("*://*/api/*").await?;
page.click_ele("#load").await?;
for req in guard.paused_requests() {
    guard.fulfill_json(req.request_id.as_ref(), r#"{"mock":true}"#).await?;
    // 或自定义状态/头/体：
    // guard.fulfill_request(id, 200, &[("Content-Type","text/html")], body).await?;
}
guard.disable().await?;
```

### 自动可操作性等待 (Playwright actionability)

`click()` 操作前自动等元素 visible + enabled（best-effort，超时不阻断），减少偶发"点了没反应"；也可显式调用：

```rust
page.ele("#submit").await?.wait_actionable(5).await?;
```

### 网络监听模式 — iter10

```rust
page.listen_start().await?;                              // 开启网络监听
// ... 用户操作 ...
let reqs = page.network_monitor().requests();            // 获取捕获的请求
let pkt = page.wait_data_packet("/api/", 10).await?;     // 等一个完整响应（含 body）
page.listen_stop().await?;                               // 停止监听
```

请求级控制（继续 / 重定向 / 拒绝）通过 `enable_intercept` 返回的 `InterceptGuard`：

```rust
let guard = page.enable_intercept("*://*/api/*").await?;
for req in guard.paused_requests() {
    guard.continue_request(req.request_id.as_ref(), None).await?;                 // 继续
    // guard.continue_request(req.request_id.as_ref(), Some("https://new.url")).await?; // 重定向
    // guard.fail_request(req.request_id.as_ref()).await?;                            // 拒绝
}
guard.disable().await?;                                  // 或 drop(guard) 自动关闭
```

同步用法相同 —— `SyncPage::enable_intercept` 返回 `SyncInterceptGuard`，零 await：

```rust
let guard = p.enable_intercept("*://*/api/*")?;
p.click_ele("#load")?;                                   // 触发请求
for req in guard.paused_requests() {
    guard.fail_request(req.request_id.as_ref())?;        // 拦截并拒绝
}
guard.disable()?;
```

### 设备模拟 + 网络控制 (9) — iter10

```rust
page.set_device_scale(2.0).await?;                       // 设置设备像素比
page.set_touch(true).await?;                             // 开启触摸模式
page.set_location_and_reload(39.9, 116.4).await?;       // 地理位置+刷新
page.smooth_scroll(0, 1500, 500).await?;                 // 平滑滚动（x, y, ms）
page.set_blocked_urls(&["*://*/*/*.css"]).await?;        // URL 拦截
page.set_offline(true).await?;                           // 离线模式
page.clear_cache().await?;                               // 清除缓存
page.disable_images().await?;                            // 禁用所有图片
page.get_and_wait(url, 10).await?;                       // 导航+等待标题
```

### 资源分析 (3) — iter10

```rust
let links = page.links().await?;                         // Vec<String> 所有链接
let imgs = page.images().await?;                         // Vec<String> 所有图片
let n = page.ele_count("a").await?;                      // 匹配元素数量
```

### 模式切换 + 生命周期

```rust
page.to_session().await?;    // 浏览器 → HTTP
page.to_chromium().await?;   // HTTP → 浏览器
page.sleep(Duration::from_secs(1)).await;
page.close().await?;
page.quit().await?;
```

### 自定义配置

```rust
let opts = WebPageOptions {
    chromium: ChromiumOptions {
        headless: true,
        timeout: Duration::from_secs(10),  // 自定义超时
        proxy: Some("http://proxy:8080".into()),
        user_agent: "Mozilla/5.0 ...".into(),
        browser_path: Some("/usr/bin/chromium".into()),
        disable_gpu: true,
        no_sandbox: true,
        extension_dirs: vec!["/path/to/ext".into()],
        ..Default::default()
    },
    ..Default::default()
};
let page = WebPage::with_options(opts).await?;
```

环境变量：`RPAGE_CHROME_PATH` 指定 Chrome 路径

### 低级访问

```rust
page.is_chromium();       // bool
page.is_session();        // bool
page.inner_page();        // Option<&Page>
page.options();           // Option<&ChromiumOptions>
```

## 项目结构

```
rpage/  (12,000+ 行 Rust, 580+ 方法)
├── src/
│   ├── chromium_page.rs  # CDP 浏览器控制 (190+ 方法) + Agent 接口
│   ├── element.rs        # 元素操作 (100+ 方法 + ElementBatch + Wait)
│   ├── sync.rs           # SyncPage 同步封装 (100+ 方法)
│   ├── agent.rs          # Agent 数据结构 (InteractiveElement/PageSnapshot/ActionAttempt)
│   ├── js_helpers.rs     # JS 代码片段 (XPath 回退/DOM 遍历等)
│   ├── web_page.rs       # 统一双模式页 (120+ 方法)
│   ├── session_page.rs   # HTTP 会话 (25+ 方法)
│   ├── download.rs       # 下载管理 (13 方法)
│   ├── network.rs        # 网络监控 + 实时监听 + 监听模式 (20 方法)
│   ├── locator.rs        # 定位器解析 + XPath 回退 (6 方法)
│   ├── config.rs         # 配置 (24 方法)
│   ├── stealth.rs        # 反检测 (5 方法)
│   ├── cookie-hub.rs     # Cookie 同步 + 文件 (10 方法)
│   ├── wait.rs           # 等待策略 (6 方法)
│   ├── console.rs        # Console 捕获 (5 方法)
│   ├── websocket.rs      # WebSocket 监听 (5 方法)
│   ├── error.rs          # 错误类型
│   ├── prelude.rs        # 预导入
│   └── lib.rs            # 入口
├── examples/
│   └── showcase.rs       # 能力展示：10 大场景 50+ 项操作
└── tests/                # 145 个测试
```

## License

MIT
