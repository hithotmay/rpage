//! 抓取菜鸟教程 Rust 全部教程并生成 PDF
//!
//! 流程: 连接 Chrome → 遍历 29 个章节 → 提取正文 HTML → 合并 → 输出 PDF

use rpage::WebPage;

const BASE: &str = "https://www.runoob.com";

/// 教程目录 (名称 → 路径)
const CHAPTERS: &[(&str, &str)] = &[
    ("Rust 教程", "/rust/rust-tutorial.html"),
    ("Rust 简介", "/rust/rust-intro.html"),
    ("Rust 环境搭建", "/rust/rust-setup.html"),
    ("Cargo 教程", "/rust/cargo-tutorial.html"),
    ("Rust 输出到命令行", "/rust/rust-println.html"),
    ("Rust 基础语法", "/rust/rust-basic-syntax.html"),
    ("Rust 运算符", "/rust/rust-operators.html"),
    ("Rust 数据类型", "/rust/rust-data-types.html"),
    ("Rust 注释", "/rust/rust-comments.html"),
    ("Rust 函数", "/rust/rust-function.html"),
    ("Rust 条件语句", "/rust/rust-conditions.html"),
    ("Rust 循环", "/rust/rust-loop.html"),
    ("Rust 迭代器", "/rust/rust-iter.html"),
    ("Rust 闭包", "/rust/rust-closure.html"),
    ("Rust 所有权", "/rust/rust-ownership.html"),
    ("Rust Slice（切片）类型", "/rust/rust-slice.html"),
    ("Rust 结构体", "/rust/rust-struct.html"),
    ("Rust 枚举类", "/rust/rust-enum.html"),
    ("Rust 组织管理", "/rust/rust-project-management.html"),
    ("Rust 错误处理", "/rust/rust-error-handle.html"),
    ("Rust 泛型与特性", "/rust/rust-generics.html"),
    ("Rust 生命周期", "/rust/rust-lifetime.html"),
    ("Rust 文件与 IO", "/rust/rust-file-io.html"),
    ("Rust 集合与字符串", "/rust/rust-collection-string.html"),
    ("Rust 面向对象", "/rust/rust-object.html"),
    ("Rust 并发编程", "/rust/rust-concurrency.html"),
    ("Rust 宏", "/rust/rust-macros.html"),
    ("Rust 智能指针", "/rust/rust-smart-pointers.html"),
    ("Rust 异步编程", "/rust/rust-async-await.html"),
];

/// JS: 提取 #content 内的主要文章内容
const JS_EXTRACT: &str = r#"
(function() {
    // 菜鸟教程正文在 #content .design 或 #article .content-area
    var main = document.querySelector('#content .design')
            || document.querySelector('#article')
            || document.querySelector('.article-body')
            || document.querySelector('article')
            || document.querySelector('#content');
    if (!main) return '<p>未找到内容</p>';

    // 移除不需要的元素: 广告/导航/侧边栏/iframe/脚本
    var remove = main.querySelectorAll(
        'iframe, script, style, noscript, .ad, .sidebar, .navigation, ' +
        '#navbar, .header, .footer, ins, .bottom-ad, .recommend, ' +
        '.comments, .post-nav, .related, #partner, .text-center.mt20, ' +
        '[id*="ad"], [class*="ad-"], [class*="advertisement"]'
    );
    for (var i = 0; i < remove.length; i++) {
        remove[i].parentNode.removeChild(remove[i]);
    }

    var html = main.innerHTML;
    return html;
})()
"#;

#[tokio::main]
async fn main() {
    println!("📖 菜鸟教程 Rust 全教程 → PDF");
    println!("   自动启动 Chrome 浏览器 ...\n");

    let page = match WebPage::new().await {
        Ok(p) => p,
        Err(e) => { eprintln!("❌ 启动浏览器失败: {e}"); return; }
    };
    // WebPage 自动启动 Chrome 并连接，无需手动开浏览器
    // 通过 chromium() 获取底层 ChromiumPage 来使用 PDF 等浏览器专属功能
    // 但 WebPage 本身已有 get/ele/fill 等方法，可直接使用

    let total = CHAPTERS.len();
    let mut all_html = String::new();

    // CSS 样式 (嵌入 PDF 中)
    all_html.push_str(r#"<!DOCTYPE html>
<html lang="zh-CN">
<head>
<meta charset="UTF-8">
<title>菜鸟教程 - Rust 完整教程</title>
<style>
body { font-family: "Microsoft YaHei", "PingFang SC", sans-serif; max-width: 900px; margin: 0 auto; padding: 20px; color: #333; line-height: 1.8; }
h1 { color: #1a73e8; border-bottom: 3px solid #1a73e8; padding-bottom: 10px; font-size: 28px; }
h2 { color: #2c3e50; border-bottom: 1px solid #ddd; padding-bottom: 6px; margin-top: 30px; font-size: 22px; }
h3 { color: #34495e; margin-top: 20px; font-size: 18px; }
h4 { color: #555; font-size: 16px; }
pre { background: #f8f8f8; border: 1px solid #ddd; border-radius: 4px; padding: 12px; overflow-x: auto; font-size: 14px; line-height: 1.5; }
code { background: #f4f4f4; padding: 2px 6px; border-radius: 3px; font-size: 14px; }
pre code { background: none; padding: 0; }
table { border-collapse: collapse; width: 100%; margin: 10px 0; }
th, td { border: 1px solid #ddd; padding: 8px 12px; text-align: left; }
th { background: #f0f0f0; }
a { color: #1a73e8; text-decoration: none; }
blockquote { border-left: 4px solid #1a73e8; margin: 10px 0; padding: 10px 20px; background: #f0f7ff; }
.chapter-divider { page-break-before: always; border-top: 3px double #1a73e8; margin: 40px 0 20px 0; padding-top: 20px; }
.toc { background: #f9f9f9; border: 1px solid #ddd; padding: 20px 30px; border-radius: 8px; margin: 20px 0; }
.toc h2 { border: none; margin-top: 0; }
.toc ol { line-height: 2; }
.toc a { color: #333; }
.toc a:hover { color: #1a73e8; }
.cover { text-align: center; padding: 100px 0 50px 0; }
.cover h1 { font-size: 42px; border: none; }
.cover .subtitle { font-size: 18px; color: #666; margin-top: 20px; }
.cover .meta { font-size: 14px; color: #999; margin-top: 40px; }
@media print { .chapter-divider { page-break-before: always; } }
</style>
</head>
<body>
"#);

    // 封面
    all_html.push_str(r#"<div class="cover">
<h1>🦀 Rust 完整教程</h1>
<p class="subtitle">菜鸟教程 · 从入门到精通</p>
<p class="meta">来源: runoob.com · 共 29 章</p>
</div>
"#);

    // 目录
    all_html.push_str(r#"<div class="toc"><h2>📑 目录</h2><ol>"#);
    for (title, _) in CHAPTERS {
        all_html.push_str(&format!("<li>{}</li>\n", title));
    }
    all_html.push_str("</ol></div>\n");

    // 逐章抓取
    for (idx, (title, path)) in CHAPTERS.iter().enumerate() {
        let url = format!("{}{}", BASE, path);
        println!("  [{}/{}] 正在抓取: {} ...", idx + 1, total, title);

        match page.get(&url).await {
            Ok(_) => {
                // 等待页面加载
                page.wait_js("document.readyState === 'complete'", 8).await.ok();
                // 额外等待让动态内容加载
                page.sleep(std::time::Duration::from_millis(500)).await;

                match page.execute(JS_EXTRACT).await {
                    Ok(val) => {
                        let html = val.as_str().unwrap_or("<p>提取失败</p>");
                        println!("    ✅ 获取 {} chars", html.len());

                        all_html.push_str(&format!(
                            "<div class=\"chapter-divider\">\n<h1>第 {} 章 · {}</h1>\n{}\n</div>\n",
                            idx + 1, title, html
                        ));
                    }
                    Err(e) => {
                        println!("    ❌ JS 提取失败: {e}");
                        all_html.push_str(&format!(
                            "<div class=\"chapter-divider\"><h1>第 {} 章 · {}</h1><p>内容提取失败: {}</p></div>\n",
                            idx + 1, title, e
                        ));
                    }
                }
            }
            Err(e) => {
                println!("    ❌ 导航失败: {e}");
                all_html.push_str(&format!(
                    "<div class=\"chapter-divider\"><h1>第 {} 章 · {}</h1><p>页面加载失败: {}</p></div>\n",
                    idx + 1, title, e
                ));
            }
        }
    }

    all_html.push_str("\n</body></html>");

    // 保存合并后的 HTML
    let output_dir = "C:/Users/18824/rpage";
    std::fs::create_dir_all(output_dir).ok();
    let html_path = format!("{}/rust_tutorial.html", output_dir);
    let pdf_path = format!("{}/rust_tutorial.pdf", output_dir);
    std::fs::write(&html_path, &all_html).unwrap();
    println!("\n📄 合并 HTML 已保存: {} ({} bytes)", html_path, all_html.len());

    // 加载合并后的 HTML 并生成 PDF
    let file_url = format!("file:///{}", html_path);
    println!("🖨️  正在生成 PDF ...");

    page.get(&file_url).await.unwrap();
    page.wait_js("document.readyState === 'complete'", 10).await.ok();
    page.sleep(std::time::Duration::from_millis(1000)).await;

    match page.pdf(&pdf_path).await {
        Ok(_) => {
            let size = std::fs::metadata(&pdf_path).map(|m| m.len()).unwrap_or(0);
            println!("✅ PDF 已生成: {} ({} KB)", pdf_path, size / 1024);
        }
        Err(e) => {
            println!("❌ PDF 生成失败: {e}");
            println!("   可手动用 Chrome 打开 {html_path} 另存为 PDF");
        }
    }

    println!("\n🏁 完成!");
}
