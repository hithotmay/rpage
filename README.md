# rpage 🦀🌐

> Rust 版 DrissionPage — 浏览器自动化 + HTTP 会话 + Cookie 互通，三合一。

`rpage` 是一个受 [DrissionPage](https://github.com/g1879/DrissionPage) 启发的 Rust 浏览器自动化库。

## ✨ 核心特性

- **一个函数启动浏览器** — `WebPage::new()` 自动检测 Chrome → 启动子进程 → CDP 连接
- **零自动化标记** — 没有 `--enable-automation`，没有 `navigator.webdriver`，永不触发验证码
- **接管已打开浏览器** — `WebPage::connect("http://localhost:9222")` 接管你的 Chrome
- **Cookie 互通** — 浏览器 ↔ HTTP 会话共享 Cookie
- **智能等待** — `get()` 自动等页面加载，`ele()` 自动重试等待元素出现
- **中文完美支持** — `fill()` 用 JS `nativeInputValueSetter`，中文/Unicode 无损输入
- **鲁棒交互** — `click()` 自动 fallback CDP→JS
- **30+ 元素操作** — click, fill, input, hover, select, upload, submit, drag...
- **相对定位** — parent, child, next, prev
- **随机端口** — 多实例不冲突

## 🚀 快速开始

```rust
use rpage::WebPage;

#[tokio::main]
async fn main() -> rpage::Result<()> {
    let page = WebPage::new().await?;           // 自动启动 Chrome
    page.get("https://www.baidu.com").await?;   // 自动等待加载
    page.ele("#kw").await?.fill("rust教程").await?;
    page.ele("#su").await?.click().await?;
    page.sleep(std::time::Duration::from_secs(2)).await;
    for (i, r) in page.eles("h3").await?.iter().enumerate() {
        println!("{}. {}", i + 1, r.text());
    }
    page.quit().await?;
    Ok(())
}
```

## 📦 安装

```toml
[dependencies]
rpage = "0.1"
```

## 📖 API 概览

### 页面操作

```rust
// 导航
page.get("https://example.com").await?;    // 自动等待加载
page.refresh().await?;
page.back().await?;
page.forward().await?;

// 页面信息
let title = page.title().await?;
let url = page.url().await?;
let html = page.html().await?;

// 截图
page.screenshot("shot.png").await?;

// 滚动
page.scroll_to(0, 500).await?;
page.scroll_to_top().await?;
page.scroll_to_bottom().await?;
page.scroll_down(300).await?;

// 标签页
let tabs = page.tabs().await?;
let new_tab = page.new_tab().await?;

// 对话框
page.handle_alert(true, None).await?;      // accept
page.handle_alert(false, None).await?;     // dismiss

// Cookie
let cookies = page.cookies().await?;
page.set_cookie(cookie).await?;
page.delete_cookie("name").await?;
page.clear_cookies().await?;

// iframe
let html = page.frame_html("iframe").await?;
page.frame_execute("iframe", "document.title").await?;

// JS
let val = page.execute("1 + 1").await?;
page.evaluate_on_new_document("Object.defineProperty(navigator, 'webdriver', {get: () => false})").await?;

// 生命周期
page.sleep(std::time::Duration::from_secs(1)).await;
page.close().await?;   // 关闭当前 tab
page.quit().await?;    // 关闭整个浏览器
```

### 元素定位

```rust
let el = page.ele("#id").await?;          // CSS 选择器
let el = page.ele("@class=btn").await?;   // 属性定位
let el = page.ele("@class*=btn").await?;  // 属性包含
let el = page.ele("text:登录").await?;    // 文本定位
let els = page.eles("h3").await?;        // 多元素
```

### 元素操作

```rust
// 基础操作
el.click().await?;              // 自动 fallback CDP→JS
el.fill("rust教程").await?;    // 清空+输入（支持中文）
el.input("追加文字").await?;   // 追加输入
el.clear().await?;
el.hover().await?;
el.submit().await?;

// 下拉框
el.select("选项文本").await?;
el.select_by_value("val").await?;

// 文件上传
el.upload_file("/path/to/file").await?;

// 属性
let v = el.attr("href");
let v = el.value().await?;     // input/textarea 值
let (x,y,w,h) = el.rect().await?;
let s = el.style("color").await?;
el.set_attr("class", "active").await?;

// 状态
el.is_displayed();
el.is_enabled();
el.is_selected().await?;

// 相对定位
let p = el.parent().await?;
let c = el.first_child().await?;
let n = el.next().await?;
let pv = el.prev().await?;

// JS
el.js("this.style.color='red'").await?;
```

### 模式切换

```rust
// 浏览器模式 → HTTP 模式（Cookie 自动同步）
page.to_session().await?;
page.get("https://api.example.com").await?;

// HTTP 模式 → 浏览器模式
page.to_chromium().await?;
```

### 接管已打开浏览器

```bash
# 先启动 Chrome（已登录的账号都在）
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
        ..Default::default()
    },
    ..Default::default()
};
let page = WebPage::with_options(opts).await?;
```

## 为什么不会触发验证码？

`WebPage::new()` 内部流程：
1. `std::process::Command` 启动 Chrome — 只传 `--remote-debugging-port`
2. 不传 `--enable-automation`、`--headless` 等任何自动化标记
3. 等待调试端口就绪 → 通过 CDP 连接接管

浏览器和用户手动打开的**完全一样**。

## 项目结构

```
rpage/
├── src/
│   ├── lib.rs           # 库入口
│   ├── chromium_page.rs # 浏览器控制（CDP）
│   ├── session_page.rs  # HTTP 会话
│   ├── web_page.rs      # 双模式页
│   ├── element.rs       # 统一元素（30+ 方法）
│   ├── cookie_hub.rs    # Cookie 同步
│   ├── config.rs        # 配置
│   ├── locator.rs       # 定位器
│   ├── network.rs       # 网络监控
│   ├── stealth.rs       # 反检测
│   ├── wait.rs          # 智能等待
│   ├── download.rs      # 下载管理
│   └── error.rs         # 错误类型
├── examples/
│   ├── baidu_search.rs     # 百度搜索
│   ├── baidu_visible.rs    # 非无头模式
│   ├── connect_existing.rs # 接管浏览器
│   └── basic.rs            # 基本用法
└── tests/
    └── integration.rs      # 52 个集成测试
```

## License

MIT
