# rpage 🦀🌐

> Rust 版 DrissionPage — 浏览器自动化 + HTTP 会话 + Cookie 互通，三合一。**164 个公开方法**。

`rpage` 是一个受 [DrissionPage](https://github.com/g1879/DrissionPage) 启发的 Rust 浏览器自动化库。

## ✨ 核心特性

- **一个函数启动** — `WebPage::new()` 自动检测 Chrome → 启动子进程 → CDP 连接
- **零自动化标记** — 不传 `--enable-automation`，永不触发验证码
- **Cookie 互通** — 浏览器 ↔ HTTP 会话共享 Cookie
- **智能等待** — `get()` 等加载，`ele()`/`eles()` 自动重试 5 秒
- **中文完美** — `fill()` 用 JS `nativeInputValueSetter`
- **鲁棒交互** — `click()` 自动 fallback CDP→JS
- **自动反检测** — stealth 脚本自动注入
- **标签页管理** — 创建/切换/关闭/列表
- **条件等待** — 等元素/标题/URL 变化
- **运行时修改** — headers/user_agent/viewport
- **拖拽** — `drag_to()` CDP 鼠标事件
- **批量操作** — `eles().texts()` 一行获取所有文本
- **PDF/截图** — 页面级和元素级

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

## 📖 API 参考 (164 方法)

### 页面导航 (5)

```rust
page.get(url).await?;           // 自动等 DOMContentLoaded
page.refresh().await?;          // 智能等待
page.back().await?;             // 智能等待
page.forward().await?;          // 智能等待
```

### 页面信息 (4)

```rust
let title = page.title().await?;
let url = page.url().await?;
let html = page.html().await?;
let val = page.execute("1+1").await?;  // JS 执行
```

### 元素定位 — 自动重试 5 秒 (2)

```rust
let el = page.ele("#id").await?;           // 单元素
let els = page.eles("h3").await?;         // 多元素（也自动重试）
```

**定位器语法：**
| 语法 | 说明 |
|------|------|
| `#id`, `.class`, `div > p` | CSS |
| `@class=btn` | 属性精确 |
| `@class*=btn` | 属性包含 |
| `@class^=btn` | 属性前缀 |
| `@class$=btn` | 属性后缀 |
| `text:登录` | 文本精确 |
| `text*:登录` | 文本包含 |
| `tag:form@@text:登录` | 链式定位 |

### 条件等待 (4)

```rust
// 自定义超时等待
let el = page.wait_ele("#result", 10).await?;
page.wait_title_contains("搜索结果", 5).await?;
page.wait_url_contains("search", 5).await?;

// 自定义超时查找元素
let el = page.wait_ele("text:提交", 15).await?;
```

### 标签页管理 (6)

```rust
let tabs = page.tabs().await?;          // Vec<Page>
let titles = page.tab_titles().await?;  // Vec<String>
let urls = page.tab_urls().await?;      // Vec<String>
page.new_tab().await?;
page.switch_to_tab(1).await?;           // 按索引切换
page.close_tab(0).await?;              // 按索引关闭
```

### 元素操作 (39 方法)

```rust
// ── 基础交互 ──
el.click().await?;              // 自动 fallback CDP→JS
el.fill("rust教程").await?;    // 清空+输入（中文OK）
el.input("追加").await?;       // 追加输入
el.clear().await?;
el.hover().await?;
el.submit().await?;
el.right_click().await?;
el.double_click().await?;
el.press_key("Enter").await?;

// ── 下拉框 ──
el.select("选项文本").await?;
el.select_by_value("val").await?;

// ── 文件上传 ──
el.upload_file("/path/to/file").await?;

// ── 拖拽 ──
let target = page.ele("#drop-zone").await?;
el.drag_to(&target).await?;

// ── 截图 ──
el.screenshot("element.png").await?;

// ── 属性 ──
let v = el.attr("href");           // Option<&str>，同步
let v = el.value().await?;         // input/textarea 值
let (x,y,w,h) = el.rect().await?;
let s = el.style("color").await?;
el.set_attr("class", "active").await?;

// ── 状态 ──
el.is_displayed();                  // 同步
el.is_enabled();                    // 同步
el.is_selected().await?;           // checkbox/radio

// ── 相对定位 ──
let p = el.parent().await?;
let c = el.first_child().await?;
let n = el.next().await?;
let pv = el.prev().await?;

// ── 子元素搜索 ──
let child = el.ele("a")?;
let children = el.eles("li")?;

// ── JS ──
el.js("this.style.color='red'").await?;
```

### 批量操作 — ElementBatch trait

```rust
use rpage::prelude::*;

let els = page.eles("h3").await?;
let texts = els.texts();                    // Vec<&str>
let hrefs = els.attr_values("href");        // Vec<Option<&str>>
let visible = els.displayed();              // Vec<&Element>
```

### 页面操作 (15)

```rust
// 滚动
page.scroll_to(0, 500).await?;
page.scroll_to_top().await?;
page.scroll_to_bottom().await?;
page.scroll_down(300).await?;
page.scroll_up(300).await?;

// 截图 / PDF
page.screenshot("shot.png").await?;
let bytes = page.screenshot_bytes().await?;
page.pdf("page.pdf").await?;

// 键盘
page.press("Enter").await?;

// 视口
page.set_viewport(1920, 1080).await?;

// 对话框
page.handle_alert(true, None).await?;     // accept
page.handle_alert(false, None).await?;    // dismiss

// iframe
page.frame_html("iframe").await?;
page.frame_execute("iframe", "document.title").await?;
```

### Cookie (5)

```rust
let cookies = page.cookies().await?;
page.set_cookie(cookie).await?;
page.delete_cookie("name").await?;
page.clear_cookies().await?;
page.sync_cookies().await?;     // Chromium ↔ Session 同步
```

### 运行时修改 (2)

```rust
page.set_extra_headers(headers).await?;   // HashMap<String, String>
page.set_user_agent("Mozilla/5.0 ...").await?;
```

### JS 注入 (1)

```rust
page.evaluate_on_new_document("Object.defineProperty(navigator, 'webdriver', {get: () => undefined})").await?;
```

### 模式切换 (2)

```rust
page.to_session().await?;    // 浏览器 → HTTP（Cookie 自动同步）
page.to_chromium().await?;   // HTTP → 浏览器
```

### 生命周期 (3)

```rust
page.sleep(Duration::from_secs(1)).await;
page.close().await?;     // 关闭当前 tab
page.quit().await?;      // 关闭整个浏览器
```

### 接管已打开浏览器

```bash
chrome --remote-debugging-port=9222
```
```rust
let page = WebPage::connect("http://localhost:9222").await?;
```

### 自定义配置

```rust
let opts = WebPageOptions {
    chromium: ChromiumOptions {
        headless: true,
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

### 低级访问 (5)

```rust
page.is_chromium();                     // bool
page.is_session();                      // bool
page.inner_page();                      // Option<&Page>
page.options();                         // Option<&ChromiumOptions>
page.download_manager();               // Option<&Arc<DownloadManager>>
```

## 为什么不会触发验证码？

`WebPage::new()` 用 `std::process::Command` 启动 Chrome，只传 `--remote-debugging-port`。
不传任何自动化标记。浏览器和手动打开的**完全一样**。
还自动注入 stealth 反检测脚本（隐藏 `navigator.webdriver` 等）。

## 项目结构

```
rpage/
├── src/
│   ├── lib.rs           # 库入口
│   ├── prelude.rs       # 方便导入（use rpage::prelude::*）
│   ├── chromium_page.rs # 浏览器控制（CDP, 49 方法）
│   ├── session_page.rs  # HTTP 会话（15 方法）
│   ├── web_page.rs      # 双模式页（61 方法，代理 Chromium+Session）
│   ├── element.rs       # 统一元素（39 方法 + ElementBatch）
│   ├── cookie_hub.rs    # Cookie 同步
│   ├── config.rs        # 配置（headless/proxy/UA/extensions...）
│   ├── locator.rs       # 定位器解析 + 共享转换函数
│   ├── network.rs       # 网络监控
│   ├── stealth.rs       # 反检测（自动注入）
│   ├── wait.rs          # 等待策略
│   ├── download.rs      # 下载管理
│   └── error.rs         # 错误类型
├── examples/            # 8 个示例
└── tests/               # 72 个测试
```

## License

MIT
