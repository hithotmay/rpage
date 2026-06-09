# rpage 🦀🌐

> Rust 版 DrissionPage — 浏览器自动化 + HTTP 会话 + Cookie 互通，三合一。

`rpage` 是一个受 [DrissionPage](https://github.com/g1879/DrissionPage) 启发的 Rust 浏览器自动化库，提供三种核心对象：

| 对象 | 说明 |
|------|------|
| **`ChromiumPage`** | 通过 Chrome DevTools Protocol (CDP) 控制浏览器 |
| **`SessionPage`** | 纯 HTTP 请求模式（reqwest），无需浏览器 |
| **`WebPage`** | 双模式合一，支持无缝切换 + Cookie 自动同步 |

## ✨ 特性

- 🔄 **双模式无缝切换** — 浏览器 ↔ HTTP 一键切换，Cookie 自动同步
- 🔗 **接管已有浏览器** — 连接已打开的 Chrome，零自动化标记，永不触发验证码
- 🎯 **直觉式定位器** — `#id`、`.class`、`xpath:`、`text=`、`@attr=val` 统一语法
- 🍪 **Cookie 双向同步** — 浏览器 ↔ HTTP 会话共享 Cookie
- 🥷 **反检测** — 内置 Stealth（WebDriver 隐藏、UA 修复、Plugin 伪装）
- ⚡ **智能等待** — 自动等待元素可见/可点击，减少 flaky 测试
- 📡 **网络监控** — 请求/响应记录、拦截、Header 覆写
- 📥 **下载管理** — 统一管理 Chromium 和 HTTP 模式下载

## 🚀 快速开始

```toml
[dependencies]
rpage = "0.1"
tokio = { version = "1", features = ["full"] }
```

### 方式一：接管已打开的浏览器（推荐，永不触发验证码）

```bash
# 先用命令行启动 Chrome（你自己的 profile，已登录的账号都在）
chrome --remote-debugging-port=9222
```

```rust
use rpage::WebPage;

#[tokio::main]
async fn main() -> rpage::Result<()> {
    // 接管已打开的浏览器 — 零自动化标记
    let mut page = WebPage::connect("http://localhost:9222").await?;
    
    page.get("https://www.baidu.com").await?;
    let search = page.ele("#kw").await?;
    search.js("this.value = 'rust教程'; this.dispatchEvent(new Event('input', {bubbles: true}));").await?;
    page.ele("#su").await?.click().await?;
    
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let results = page.eles("h3").await?;
    for r in &results {
        println!("{}", r.text());
    }
    
    Ok(())
}
```

### 方式二：自动启动浏览器（内置 stealth 反检测）

```rust
use rpage::{ChromiumOptions, WebPage};

#[tokio::main]
async fn main() -> rpage::Result<()> {
    let opts = ChromiumOptions::builder()
        .headless(true)
        .viewport(1280, 800)
        .no_sandbox(true)
        .build();
    
    let mut page = WebPage::with_options(
        rpage::config::WebPageOptions::builder().chromium(opts).build()
    ).await?;
    
    page.get("https://example.com").await?;
    let heading = page.ele("h1").await?;
    println!("标题: {}", heading.text());
    
    Ok(())
}
```

### 方式三：纯 HTTP 模式（无需浏览器）

```rust
use rpage::WebPage;

fn main() -> rpage::Result<()> {
    let mut page = WebPage::session_only(None)?;
    
    // 同步 HTTP 请求
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        page.get("https://httpbin.org/get").await?;
        let elements = page.eles("p")?;
        println!("找到 {} 个段落", elements.len());
        Ok::<(), rpage::Error>(())
    })?;
    
    Ok(())
}
```

## 📖 定位器语法

```rust
// CSS 选择器
page.ele("#myid").await?
page.ele(".container > p").await?

// XPath
page.ele("xpath://div[@id='content']").await?

// 文本匹配
page.ele("text=登录").await?          // 精确匹配
page.ele("text*=提交").await?         // 包含匹配

// 属性匹配
page.ele("@name=username").await?     // 属性等于
page.ele("@href*=logout").await?      // 属性包含
```

## 🏗️ 架构

```
┌─────────────────────────────────────┐
│              WebPage                 │
│  (双模式统一入口 + Cookie 同步)      │
│  connect() / new() / session_only() │
├──────────────┬──────────────────────┤
│ ChromiumPage │    SessionPage       │
│ (CDP 协议)   │  (reqwest HTTP)      │
├──────────────┼──────────────────────┤
│chromiumoxide │ scraper + sxd-xpath  │
└──────────────┴──────────────────────┘
         ↕ Cookie 互通 ↕
┌─────────────────────────────────────┐
│           CookieHub                  │
│    (cookie_store + 双向同步)         │
└─────────────────────────────────────┘
```

## 🧪 运行测试

```bash
cargo test
```

## 📋 版本计划

| 版本 | 内容 |
|------|------|
| **v0.1** | ✅ 核心三对象 + Cookie 同步 + 定位器 + 等待 + Stealth + 网络监控 |
| **v0.2** | ✅ Element 异步操作 + 接管已有浏览器 + 72 测试 |
| **v0.3** | 🔲 iframe 支持 + 链式定位 + 智能等待增强 |
| **v0.4** | 🔲 请求拦截 + 代理 + 下载管理增强 |
| **v1.0** | 🔲 文档完善 + 跨平台测试 + API 冻结 |

## 📄 License

MIT
