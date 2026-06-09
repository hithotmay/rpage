# rpage 🦀🌐

> Rust 版 DrissionPage — 浏览器自动化 + HTTP 会话 + Cookie 互通，三合一。

`rpage` 是一个受 [DrissionPage](https://github.com/g1879/DrissionPage) 启发的 Rust 浏览器自动化库，提供三种核心对象：

| 对象 | 说明 |
|------|------|
| **`ChromiumPage`** | 浏览器控制（CDP）— 自动启动 Chrome 并接管 |
| **`SessionPage`** | HTTP 会话 — 纯 HTTP 请求，带 Cookie 管理 |
| **`WebPage`** | 双模式页 — 浏览器 + HTTP，共享 Cookie |

## ✨ 核心特性

- **一个函数启动浏览器** — `WebPage::new()` 自动检测 Chrome → 启动子进程 → CDP 连接
- **零自动化标记** — 不走 chromiumoxide 默认启动，没有 `--enable-automation`，永不触发验证码
- **接管已打开浏览器** — `WebPage::connect("http://localhost:9222")` 接管你的 Chrome
- **Cookie 互通** — 浏览器 ↔ HTTP 会话共享 Cookie
- **链式 API** — `page.ele("#kw").await?.input("rust教程").await?`

## 🚀 快速开始

### 一行代码搜索百度（永不触发验证码）

```rust
use rpage::WebPage;

#[tokio::main]
async fn main() -> rpage::Result<()> {
    let mut page = WebPage::new().await?;   // 自动启动 Chrome + 连接

    page.get("https://www.baidu.com").await?;
    let search_box = page.ele("#kw").await?;
    search_box.js("this.value='rust教程'; this.dispatchEvent(new Event('input',{bubbles:true}));").await?;
    page.ele("#su").await?.click().await?;

    let results = page.eles("h3").await?;
    for (i, r) in results.iter().enumerate() {
        println!("{}. {}", i + 1, r.text());
    }
    Ok(())
}
```

### 接管已打开的浏览器

```bash
# 1. 先启动你自己的 Chrome（已登录的账号都在）
chrome --remote-debugging-port=9222
```
```rust
// 2. rpage 接管
let mut page = WebPage::connect("http://localhost:9222").await?;
```

### HTTP 会话模式

```rust
let mut page = WebPage::session_only(None)?;
page.get("https://httpbin.org/get").await?;
println!("{}", page.html().await?);
```

## 📦 安装

```toml
[dependencies]
rpage = "0.1"
```

## 为什么不会触发验证码？

DrissionPage 的核心理念：**浏览器是你自己打开的，不是自动化工具启动的**。

`WebPage::new()` 内部流程：
1. `std::process::Command` 启动 Chrome — 只传 `--remote-debugging-port=9222`
2. 不传 `--enable-automation`、`--headless` 等任何自动化标记
3. 等待调试端口就绪
4. 通过 CDP 连接接管

对比 chromiumoxide 默认启动（会触发验证码）：
- 添加 `--enable-automation`
- 设置 `navigator.webdriver = true`
- Headless 模式 UA 含 `HeadlessChrome`

## API 概览

```rust
// 导航
page.get("https://example.com").await?;
page.refresh().await?;
page.back().await?;

// 元素操作
let el = page.ele("#id").await?;       // CSS 选择器
let el = page.ele("@class=btn").await?; // 属性定位
let el = page.ele("text:登录").await?;  // 文本定位
let els = page.eles("h3").await?;      // 多元素

el.click().await?;
el.input("hello").await?;
el.js("this.style.color='red'").await?;

// 页面信息
let title = page.title().await?;
let url = page.url().await?;
let html = page.html().await?;
page.screenshot("shot.png").await?;

// Cookie
let cookies = page.cookies().await?;
```

## 项目结构

```
rpage/
├── src/
│   ├── lib.rs           # 库入口
│   ├── chromium_page.rs # 浏览器控制（CDP）
│   ├── session_page.rs  # HTTP 会话
│   ├── web_page.rs      # 双模式页
│   ├── element.rs       # 统一元素
│   ├── cookie_hub.rs    # Cookie 同步
│   ├── config.rs        # 配置
│   ├── locator.rs       # 定位器
│   ├── wait.rs          # 智能等待
│   ├── download.rs      # 下载管理
│   └── error.rs         # 错误类型
├── examples/
│   ├── baidu_search.rs  # 百度搜索演示
│   └── connect_existing.rs # 接管已打开浏览器
└── tests/
    └── integration.rs   # 52 个集成测试
```

## License

MIT
