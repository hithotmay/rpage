# rpage 🦀🌐

> Rust 版 DrissionPage — 浏览器自动化 + HTTP 会话 + Cookie 互通，三合一。**284 个公开方法，5744 行 Rust**。

`rpage` 是一个受 [DrissionPage](https://github.com/g1879/DrissionPage) 启发的 Rust 浏览器自动化库。

## ✨ 核心特性

- **一个函数启动** — `WebPage::new()` 自动检测 Chrome → 启动子进程 → CDP 连接
- **零自动化标记** — 不传 `--enable-automation`，永不触发验证码
- **自动反检测** — stealth 脚本自动注入
- **Cookie 互通** — 浏览器 ↔ HTTP 会话共享 Cookie，支持导入/导出文件
- **智能等待** — `get()` 等加载，`ele()`/`eles()` 自动重试，超时可配置
- **中文完美** — `fill()` 用 JS `nativeInputValueSetter`
- **鲁棒交互** — `click()` 自动 fallback CDP→JS，拖拽到元素或坐标
- **标签页管理** — 创建/切换/关闭/列表
- **条件等待** — 等元素出现/消失/删除，等标题/URL 变化，等 JS 表达式
- **运行时修改** — headers/user_agent/viewport
- **批量操作** — `eles().texts()` 一行获取所有文本
- **文件上传** — 浏览器 + Session multipart 双模式
- **PDF/截图** — 页面级和元素级
- **下载管理** — CDP 下载事件监听 + 等待下载完成
- **网络拦截** — Fetch.enable 拦截/修改/阻止请求
- **ActionChain** — 复杂鼠标键盘序列
- **iframe 上下文** — 进入 iframe 执行操作
- **网络监控** — 记录所有请求/响应/失败
- **灵活配置** — 环境变量 `RPAGE_CHROME_PATH`，PATH 搜索，标准路径

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

## 📦 安装

```toml
[dependencies]
rpage = "0.1"
```

## 📖 API 参考 (284 方法)

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

| 语法 | 说明 |
|------|------|
| `#id`, `.class` | CSS |
| `@class=btn` | 属性精确 |
| `@class*=btn` | 属性包含 |
| `@class^=btn` | 属性前缀 |
| `@class$=btn` | 属性后缀 |
| `text:登录` | 文本精确 |
| `text*:登录` | 文本包含 |
| `tag:form@@text:登录` | 链式定位 |

### 条件等待 (7)

```rust
let el = page.wait_ele("#result", 10).await?;
page.wait_ele_hidden("#loading", 5).await?;
page.wait_ele_deleted(".modal", 5).await?;
page.wait_title_contains("搜索结果", 5).await?;
page.wait_url_contains("search", 5).await?;
page.wait_js("document.querySelectorAll('.item').length > 5", 10).await?;
let dl = page.wait_download(30).await?;
```

### 标签页 (6)

```rust
page.new_tab().await?;
let titles = page.tab_titles().await?;
let urls = page.tab_urls().await?;
page.switch_to_tab(1).await?;
page.close_tab(0).await?;
let tabs = page.tabs().await?;
```

### 元素操作 (41 方法)

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

// ── 截图 ──
el.screenshot("el.png").await?;

// ── 属性 / 状态 ──
let v = el.attr("href");           // Option<&str>
let v = el.value().await?;
let (x,y,w,h) = el.rect().await?;
let s = el.style("color").await?;
el.set_attr("class", "active").await?;
el.is_displayed();                  // 同步
el.is_visible().await?;            // 异步，真实 CDP 检测
el.is_enabled();
el.is_selected().await?;

// ── 相对定位 ──
let p = el.parent().await?;
let c = el.first_child().await?;
let n = el.next().await?;
let pv = el.prev().await?;
let child = el.ele("a")?;
let children = el.eles("li")?;

// ── JS ──
el.js("this.style.color='red'").await?;
```

### 批量操作 — ElementBatch

```rust
let els = page.eles("h3").await?;
let texts = els.texts();                // Vec<&str>
let hrefs = els.attr_values("href");    // Vec<Option<&str>>
let visible = els.displayed();          // Vec<&Element>
```

### 页面操作 (15)

```rust
page.scroll_to(0, 500).await?;
page.scroll_to_top/bottom/up/down().await?;
page.screenshot("shot.png").await?;
page.pdf("page.pdf").await?;
page.press("Enter").await?;
page.set_viewport(1920, 1080).await?;
page.handle_alert(true, None).await?;
page.frame_html("iframe").await?;
page.frame_execute("iframe", "document.title").await?;
```

### Cookie (7)

```rust
let cookies = page.cookies().await?;
page.set_cookie(cookie).await?;
page.delete_cookie("name").await?;
page.clear_cookies().await?;
page.sync_cookies().await?;
page.save_cookies_to_file("cookies.json").await?;
page.load_cookies_from_file("cookies.json").await?;
```

### 运行时修改 (2)

```rust
page.set_extra_headers(headers).await?;
page.set_user_agent("Mozilla/5.0 ...").await?;
```

### Session HTTP (4)

```rust
page.post(url, body).await?;
page.post_multipart(url, fields, "file", "/path").await?;
page.get(url).await?;   // 通用
page.post_json(url, data).await?;
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
    guard.continue_request(&req.request_id, None).await?;
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
rpage/  (5744 行 Rust, 284 方法)
├── src/
│   ├── chromium_page.rs  # CDP 浏览器控制 (77 方法)
│   ├── web_page.rs       # 统一双模式页 (73 方法)
│   ├── element.rs        # 元素操作 (41 方法 + ElementBatch)
│   ├── session_page.rs   # HTTP 会话 (16 方法)
│   ├── download.rs       # 下载管理 (13 方法)
│   ├── network.rs        # 网络监控 (13 方法)
│   ├── cookie_hub.rs     # Cookie 同步 + 文件 (10 方法)
│   ├── config.rs         # 配置 (24 方法)
│   ├── wait.rs           # 等待策略 (6 方法)
│   ├── locator.rs        # 定位器解析 (6 方法)
│   ├── stealth.rs        # 反检测 (5 方法)
│   └── error.rs          # 错误类型
├── examples/             # 8 个示例
└── tests/                # 72 个测试
```

## License

MIT
