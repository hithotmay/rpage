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
- 🎯 **直觉式定位器** — `#id`、`.class`、`xpath:`、`text=`、`@attr=val` 统一语法
- 🍪 **Cookie 双向同步** — 浏览器 ↔ HTTP 会话共享 Cookie
- 🥷 **反检测** — 内置 Stealth Profile（WebDriver 隐藏、WebGL/Plugin 伪装）
- ⚡ **智能等待** — 自动等待元素可见/可点击，减少 flaky 测试
- 📡 **网络监控** — 请求/响应记录、拦截、Header 覆写
- 📥 **下载管理** — 统一管理 Chromium 和 HTTP 模式下载

## 🚀 快速开始

```toml
[dependencies]
rpage = "0.1"
tokio = { version = "1", features = ["full"] }
```

```rust
use rpage::WebPage;

#[tokio::main]
async fn main() -> rpage::Result<()> {
    // 启动浏览器
    let mut page = WebPage::new().await?;
    
    // 导航
    page.get("https://example.com").await?;
    
    // 查找元素
    let heading = page.ele("h1").await?;
    println!("标题: {}", heading.text());
    
    // 切换到 HTTP 模式（Cookie 自动同步）
    page.to_session().await?;
    page.get("https://example.com/api").await?;
    
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

// 链式定位
page.ele("tag:form@@text=提交").await?
```

## 🔧 Session 模式（纯 HTTP）

```rust
use rpage::SessionPage;

fn main() -> rpage::Result<()> {
    let rt = tokio::runtime::Runtime::new()?;
    let mut page = SessionPage::new()?;
    
    rt.block_on(async {
        page.get("https://httpbin.org/get").await?;
        let title = page.title();
        let elements = page.eles("p")?;
        println!("找到 {} 个段落", elements.len());
        Ok::<(), rpage::Error>(())
    })?;
    
    Ok(())
}
```

## 🥷 Stealth 反检测

```rust
use rpage::stealth::{apply_stealth, StealthConfig, user_agents};
use rpage::ChromiumPage;

let page = ChromiumPage::new().await?;
let config = StealthConfig::new()
    .user_agent(user_agents::CHROME_WINDOWS)
    .viewport(1920, 1080);
apply_stealth(page.inner_page(), &config).await?;
```

## 📡 网络监控

```rust
use rpage::network::NetworkMonitor;

let monitor = NetworkMonitor::new();
monitor.record_request(/* ... */);
let api_calls = monitor.find_requests_by_url("/api/");
```

## 🏗️ 架构

```
┌─────────────────────────────────────┐
│              WebPage                 │
│  (双模式统一入口 + Cookie 同步)      │
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
| **v0.2** | 🔲 iframe 支持 + Cookie 双向增量同步 + 丰富 locator |
| **v0.3** | 🔲 链式定位 + 智能等待增强 + 元素生命周期自动 re-resolve |
| **v0.4** | 🔲 请求拦截 + 代理 + 下载管理增强 |
| **v1.0** | 🔲 文档完善 + 跨平台测试 + API 冻结 |

## 📄 License

MIT
