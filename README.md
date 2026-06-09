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
- **智能等待** — `get()` 自动等页面加载，`ele()` 自动重试等待元素出现
- **中文完美支持** — `fill()` 用 JS `nativeInputValueSetter`，中文/Unicode 无损输入
- **鲁棒交互** — `click()` 自动 fallback CDP→JS，不怕元素"不可见"

## 🚀 快速开始

### 5 行代码搜索百度（永不触发验证码）

```rust
use rpage::WebPage;

#[tokio::main]
async fn main() -> rpage::Result<()> {
    let page = WebPage::new().await?;           // 自动启动 Chrome
    page.get("https://www.baidu.com").await?;   // 自动等待页面加载
    page.ele("#kw").await?.fill("rust教程").await?;  // 中文输入
    page.ele("#su").await?.click().await?;       // 点击搜索
    page.sleep(std::time::Duration::from_secs(2)).await;
    for (i, r) in page.eles("h3").await?.iter().enumerate() {
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
let page = WebPage::connect("http://localhost:9222").await?;
```

### HTTP 会话模式

```rust
let page = WebPage::session_only(None)?;
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
// 导航（自动等待页面加载）
page.get("https://example.com").await?;
page.refresh().await?;
page.back().await?;

// 元素操作（自动等待元素出现）
let el = page.ele("#id").await?;        // CSS 选择器
let el = page.ele("@class=btn").await?; // 属性定位
let el = page.ele("text:登录").await?;  // 文本定位
let els = page.eles("h3").await?;      // 多元素

el.click().await?;           // 自动 fallback CDP→JS
el.fill("rust教程").await?;  // 清空+输入，中文完美
el.input("追加文字").await?;  // 追加输入
el.hover().await?;           // 悬停
el.clear().await?;           // 清空

// 页面信息
let title = page.title().await?;
let url = page.url().await?;
let html = page.html().await?;
page.screenshot("shot.png").await?;

// 便捷方法
page.sleep(std::time::Duration::from_secs(1)).await;
page.close().await?;
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
│   ├── baidu_search.rs     # 百度搜索演示
│   ├── baidu_visible.rs    # 非无头模式演示
│   ├── connect_existing.rs # 接管已打开浏览器
│   └── basic.rs            # 基本用法
└── tests/
    └── integration.rs   # 52 个集成测试
```

## License

MIT
