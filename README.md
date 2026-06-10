# rpage 🦀🌐

> Rust 版 DrissionPage — 浏览器自动化 + HTTP 会话 + Cookie 互通，三合一。

`rpage` 是一个受 [DrissionPage](https://github.com/g1879/DrissionPage) 启发的 Rust 浏览器自动化库。

## ✨ 核心特性

- **一个函数启动** — `WebPage::new()` 自动检测 Chrome → 启动子进程 → CDP 连接
- **零自动化标记** — 不传 `--enable-automation`，永不触发验证码
- **接管已打开浏览器** — `WebPage::connect("http://localhost:9222")`
- **Cookie 互通** — 浏览器 ↔ HTTP 会话共享 Cookie
- **智能等待** — `get()` 自动等加载，`ele()`/`eles()` 自动重试 5 秒
- **中文完美** — `fill()` 用 JS `nativeInputValueSetter`，中文/Unicode 无损
- **鲁棒交互** — `click()` 自动 fallback CDP→JS
- **40+ 元素操作** — click, fill, select, upload, screenshot, drag...
- **批量操作** — `eles().texts()` 一行获取所有文本
- **链式定位** — `#form@@text=Login` 逐步缩小范围
- **随机端口** — 多实例不冲突

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

## 📖 API 完整参考

### 页面导航

```rust
page.get("https://example.com").await?;    // 自动等 DOMContentLoaded
page.refresh().await?;                      // 智能等待
page.back().await?;                         // 智能等待
page.forward().await?;                      // 智能等待
```

### 页面信息

```rust
let title = page.title().await?;
let url = page.url().await?;
let html = page.html().await?;
```

### 元素定位（自动重试 5 秒）

```rust
let el = page.ele("#id").await?;           // CSS
let el = page.ele("@class=btn").await?;    // 属性精确
let el = page.ele("@class*=btn").await?;   // 属性包含
let el = page.ele("@class^=btn").await?;   // 属性前缀
let el = page.ele("text:登录").await?;     // 文本精确
let el = page.ele("text*:登录").await?;    // 文本包含
let els = page.eles("h3").await?;          // 多元素（也自动重试）
```

### 链式定位（逐步缩小范围）

```rust
// 先找 form，再在其内部找文本为"登录"的元素
let btn = page.ele("tag:form@@text:登录").await?;
```

### 元素操作（40+ 方法）

```rust
// 基础
el.click().await?;                  // 自动 fallback CDP→JS
el.fill("rust教程").await?;        // 清空+输入（中文OK）
el.input("追加").await?;           // 追加输入
el.clear().await?;
el.hover().await?;
el.submit().await?;
el.right_click().await?;
el.double_click().await?;

// 下拉框
el.select("选项文本").await?;
el.select_by_value("val").await?;

// 文件上传
el.upload_file("/path/to/file").await?;

// 截图（元素级别）
el.screenshot("element.png").await?;

// 属性
let v = el.attr("href");           // 同步，Option<&str>
let v = el.value().await?;         // input/textarea 值
let (x,y,w,h) = el.rect().await?; // 位置和尺寸
let s = el.style("color").await?;  // 计算样式
el.set_attr("class", "active").await?;

// 状态
el.is_displayed();                  // 同步
el.is_enabled();                    // 同步
el.is_selected().await?;           // checkbox/radio

// 相对定位
let p = el.parent().await?;
let c = el.first_child().await?;
let n = el.next().await?;
let pv = el.prev().await?;

// 子元素搜索
let child = el.ele("a")?;
let children = el.eles("li")?;

// JS
el.js("this.style.color='red'").await?;
```

### 批量操作（ElementBatch trait）

```rust
use rpage::prelude::*;

let els = page.eles("h3").await?;
let texts = els.texts();                          // Vec<&str>
let hrefs = els.attr_values("href");              // Vec<Option<&str>>
let visible = els.displayed();                    // Vec<&Element>
```

### 页面操作

```rust
// 滚动
page.scroll_to(0, 500).await?;
page.scroll_to_top().await?;
page.scroll_to_bottom().await?;
page.scroll_down(300).await?;
page.scroll_up(300).await?;

// 截图 / PDF
page.screenshot("shot.png").await?;
page.pdf("page.pdf").await?;

// 键盘
page.press("Enter").await?;

// 视口
page.set_viewport(1920, 1080).await?;

// 标签页
let tabs = page.tabs().await?;
let new_tab = page.new_tab().await?;

// 对话框
page.handle_alert(true, None).await?;     // accept
page.handle_alert(false, None).await?;    // dismiss

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
page.evaluate_on_new_document("...").await?;

// 生命周期
page.sleep(Duration::from_secs(1)).await;
page.close().await?;    // 关闭当前 tab
page.quit().await?;     // 关闭整个浏览器
```

### 模式切换

```rust
// 浏览器 → HTTP（Cookie 自动同步）
page.to_session().await?;
page.get("https://api.example.com").await?;

// HTTP → 浏览器
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

`WebPage::new()` 用 `std::process::Command` 启动 Chrome，只传 `--remote-debugging-port`。
不传 `--enable-automation`、`--headless` 等任何自动化标记。
浏览器和手动打开的**完全一样**。

## 项目结构

```
rpage/
├── src/
│   ├── lib.rs           # 库入口
│   ├── prelude.rs       # 方便导入（use rpage::prelude::*）
│   ├── chromium_page.rs # 浏览器控制（CDP）
│   ├── session_page.rs  # HTTP 会话
│   ├── web_page.rs      # 双模式页
│   ├── element.rs       # 统一元素（40+ 方法 + ElementBatch）
│   ├── cookie_hub.rs    # Cookie 同步
│   ├── config.rs        # 配置
│   ├── locator.rs       # 定位器
│   ├── network.rs       # 网络监控
│   ├── stealth.rs       # 反检测
│   ├── wait.rs          # 智能等待
│   ├── download.rs      # 下载管理
│   └── error.rs         # 错误类型
├── examples/
│   ├── baidu_search.rs     # 百度搜索（极简 5 行）
│   ├── baidu_visible.rs    # 非无头模式
│   ├── connect_existing.rs # 接管浏览器
│   └── basic.rs            # 基本用法
└── tests/
    └── integration.rs      # 72 个测试
```

## License

MIT
