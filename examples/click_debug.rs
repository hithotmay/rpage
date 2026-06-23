//! Debug click on runoob.com "用户笔记" tab
use rpage::ChromiumPage;

#[tokio::main]
async fn main() -> rpage::Result<()> {
    let page = ChromiumPage::new().await?;
    page.get("https://www.runoob.com/").await?;

    let title = page.title().await?;
    println!("Page title: {title}");

    // 先看看菜鸟教程首页有哪些包含"笔记"文本的元素
    let js_find = r#"
        (function() {
            var results = [];
            var snap = document.evaluate(
                "//*[contains(text(),'笔记')]",
                document, null, XPathResult.ORDERED_NODE_SNAPSHOT_TYPE, null);
            for (var i = 0; i < snap.snapshotLength; i++) {
                var el = snap.snapshotItem(i);
                results.push({
                    tag: el.tagName,
                    text: el.textContent.substring(0, 50),
                    html: el.outerHTML.substring(0, 200),
                    href: el.getAttribute('href') || '',
                    id: el.id || '',
                    className: el.className || ''
                });
            }
            return JSON.stringify(results, null, 2);
        })()
    "#;
    let result = page.execute(js_find).await?;
    println!("\n=== 包含'笔记'的元素 ===");
    println!("{}", serde_json::to_string_pretty(&result)?);

    // 检查 #index-nav 结构
    println!("\n=== #index-nav 结构 ===");
    let nav_js = r#"
        (function() {
            var nav = document.querySelector('#index-nav');
            if (!nav) return "no #index-nav found";
            var items = [];
            nav.querySelectorAll('li a').forEach(function(el) {
                items.push({
                    tag: el.tagName,
                    text: (el.textContent || '').trim().substring(0, 30),
                    href: el.getAttribute('href') || '',
                    id: el.id || '',
                    onclick: el.getAttribute('onclick') || ''
                });
            });
            return JSON.stringify(items, null, 2);
        })()
    "#;
    let nav_info = page.execute(nav_js).await?;
    println!("{}", serde_json::to_string_pretty(&nav_info)?);

    // 用 JS 模拟完整鼠标事件点击
    println!("\n=== JS 模拟点击 ===");
    let js_do_click = r#"
        (function() {
            var snap = document.evaluate(
                "//*[text()='用户笔记']",
                document, null, XPathResult.FIRST_ORDERED_NODE_TYPE, null);
            var el = snap.singleNodeValue;
            if (!el) return "not found";
            var target = el.tagName === 'A' ? el : (el.closest('a') || el);
            target.scrollIntoView({block:'center'});
            
            ['mouseover','mousedown','mouseup','click'].forEach(function(type) {
                var evt = new MouseEvent(type, {
                    bubbles: true, cancelable: true, view: window
                });
                target.dispatchEvent(evt);
            });
            return "clicked: " + target.tagName + " href=" + (target.getAttribute('href') || 'none');
        })()
    "#;
    let click_result = page.execute(js_do_click).await?;
    println!("JS click 结果: {:?}", click_result);

    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    let after_url = page.url().await?;
    println!("点击后 URL: {after_url}");

    // 也试试 CDP dispatchMouseEvent 方式
    println!("\n=== 尝试 CDP 鼠标事件点击 ===");
    let cdp_js = r#"
        (function() {
            var snap = document.evaluate(
                "//*[@id='index-nav']/li[3]/a",
                document, null, XPathResult.FIRST_ORDERED_NODE_TYPE, null);
            var el = snap.singleNodeValue;
            if (!el) return "xpath not found";
            var rect = el.getBoundingClientRect();
            return JSON.stringify({
                x: rect.x + rect.width/2,
                y: rect.y + rect.height/2,
                text: el.textContent,
                href: el.getAttribute('href') || 'none'
            });
        })()
    "#;
    let pos = page.execute(cdp_js).await?;
    println!("元素位置信息: {:?}", pos);

    tokio::time::sleep(std::time::Duration::from_secs(3)).await;
    Ok(())
}
