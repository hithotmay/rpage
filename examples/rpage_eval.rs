//! End-to-end verification harness for rpage.
//!
//! Spins up a tiny local HTTP server (so the checks are deterministic and
//! offline) and drives a headless Chrome through rpage, asserting the core
//! capabilities plus the network-capture / `run_cdp` / `set_offline` /
//! element-state features. Prints a PASS/FAIL line per check and exits
//! non-zero if anything fails.
//!
//! Run with: `cargo run --example rpage_eval`

use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use rpage::ChromiumPage;
use rpage::config::ChromiumOptions;

const HTML: &str = r##"<!doctype html><html><head><meta charset="utf-8"><title>RPage Eval</title></head>
<body>
<h1 id="title">RPage 测试</h1>
<a id="link" href="#">点击我</a>
<input id="name" type="text">
<button id="btn">Load</button>
<div id="result"></div>
<div id="rc">rc-init</div>
<div id="dc">dc-init</div>
<input id="ph" placeholder="搜索关键词">
<div data-testid="tid">TID-VAL</div>
<button id="rolebtn">RoleBtn</button>
<label for="lbl">用户名</label><input id="lbl">
<div class="card" data-kind="x">CARD</div>
<button id="delayed" disabled>Delayed</button>
<div id="dresult"></div>
<div id="bottom" style="margin-top:3000px">BOTTOM</div>
<script>
document.getElementById('btn').addEventListener('click', async () => {
  const r = await fetch('/api/data');
  const j = await r.json();
  document.getElementById('result').textContent = j.msg + j.n;
});
setTimeout(function(){ document.getElementById('delayed').disabled = false; }, 600);
document.getElementById('delayed').addEventListener('click', function(){
  document.getElementById('dresult').textContent = 'CLICKED';
});
document.getElementById('rc').addEventListener('contextmenu', e => {
  e.preventDefault();
  document.getElementById('rc').textContent = 'RIGHT';
});
document.getElementById('dc').addEventListener('dblclick', () => {
  document.getElementById('dc').textContent = 'DOUBLE';
});
</script>
</body></html>"##;

fn start_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
    let port = listener.local_addr().expect("addr").port();
    thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = match stream {
                Ok(s) => s,
                Err(_) => continue,
            };
            let mut buf = [0u8; 8192];
            let n = stream.read(&mut buf).unwrap_or(0);
            let req = String::from_utf8_lossy(&buf[..n]);
            let path = req
                .lines()
                .next()
                .and_then(|l| l.split_whitespace().nth(1))
                .unwrap_or("/");
            let (ctype, body) = if path.starts_with("/api/data") {
                ("application/json", r#"{"msg":"hello","n":42}"#.to_string())
            } else {
                ("text/html; charset=utf-8", HTML.to_string())
            };
            let resp = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                ctype,
                body.len(),
                body
            );
            let _ = stream.write_all(resp.as_bytes());
        }
    });
    port
}

struct Report {
    pass: usize,
    fail: usize,
}

impl Report {
    fn check(&mut self, name: &str, ok: bool, detail: impl std::fmt::Display) {
        if ok {
            self.pass += 1;
            println!("  PASS  {name}  ({detail})");
        } else {
            self.fail += 1;
            println!("  FAIL  {name}  ({detail})");
        }
    }
}

#[tokio::main]
async fn main() -> rpage::Result<()> {
    let port = start_server();
    let base = format!("http://127.0.0.1:{port}");
    println!("local test server on {base}");

    let chrome = std::env::var("RPAGE_CHROME_PATH")
        .unwrap_or_else(|_| r"C:\Program Files\Google\Chrome\Application\chrome.exe".into());

    // Distinct port + profile so the harness never collides with a Chrome a
    // developer already has open on the default debug port.
    let opts = ChromiumOptions::builder()
        .headless(true)
        .browser_path(chrome)
        .debug_port(9871)
        .user_data_dir(std::env::temp_dir().join("rpage-eval-profile"))
        .build();

    let page = ChromiumPage::with_options(opts).await?;
    let mut r = Report { pass: 0, fail: 0 };

    // Capture every response so we can assert on_response now actually fires.
    let resp_count = Arc::new(AtomicUsize::new(0));
    let rc2 = resp_count.clone();
    page.on_response(move |_| {
        rc2.fetch_add(1, Ordering::SeqCst);
    })?;

    // ── Navigation + page info ──
    page.get(&base).await?;
    let title = page.title().await?;
    r.check("nav/title", title == "RPage Eval", title.clone());
    let html = page.html().await?;
    r.check("html/source", html.contains("RPage 测试"), "contains 测试");

    // ── Locators: CSS / text: / xpath: ──
    let t = page.ele("#title").await?;
    r.check("ele(css).text", t.text() == "RPage 测试", t.text().to_string());
    match page.ele("text:点击我").await {
        Ok(e) => r.check("ele(text:)", e.tag() == "a", format!("tag={}", e.tag())),
        Err(e) => r.check("ele(text:)", false, e.to_string()),
    }
    match page.ele("xpath://h1").await {
        Ok(e) => r.check("ele(xpath:)", e.text().contains("测试"), e.text().to_string()),
        Err(e) => r.check("ele(xpath:)", false, e.to_string()),
    }

    // ── Chinese input via insertText ──
    page.ele("#name").await?.fill("你好世界").await?;
    let v = page.ele("#name").await?.value().await?;
    r.check("fill(chinese)", v == "你好世界", v.clone());

    // ── Click → triggers fetch; verifies click + response capture ──
    page.ele("#btn").await?.click().await?;
    let _ = page
        .wait_js("document.getElementById('result').textContent.length>0", 5)
        .await;
    let result_txt = page.ele("#result").await?.text().to_string();
    r.check("click→fetch result", result_txt == "hello42", result_txt);
    r.check(
        "on_response fired",
        resp_count.load(Ordering::SeqCst) > 0,
        format!("count={}", resp_count.load(Ordering::SeqCst)),
    );
    let api_resps = page.get_responses("/api/data");
    r.check(
        "responses() populated",
        !api_resps.is_empty(),
        format!("{} api responses", api_resps.len()),
    );

    // ── get_response_body (DataPacket body parity) ──
    if let Some(rec) = api_resps.first() {
        match page.get_response_body(&rec.request_id).await {
            Ok(body) => r.check(
                "get_response_body",
                body.text().contains("hello"),
                body.text(),
            ),
            Err(e) => r.check("get_response_body", false, e.to_string()),
        }
    } else {
        r.check("get_response_body", false, "no api response to fetch");
    }

    // ── wait_data_packet (DrissionPage listen.wait() parity) ──
    match page.wait_data_packet("/api/data", 5).await {
        Ok(pkt) => r.check(
            "wait_data_packet",
            pkt.status == 200 && pkt.body_text().contains("hello"),
            format!("status={} body={}", pkt.status, pkt.body_text()),
        ),
        Err(e) => r.check("wait_data_packet", false, e.to_string()),
    }

    // ── right_click / double_click (real CDP mouse events) ──
    page.ele("#rc").await?.right_click().await?;
    page.sleep(Duration::from_millis(150)).await;
    let rc_txt = page.ele("#rc").await?.text().to_string();
    r.check("right_click", rc_txt == "RIGHT", rc_txt);

    page.ele("#dc").await?.double_click().await?;
    page.sleep(Duration::from_millis(150)).await;
    let dc_txt = page.ele("#dc").await?.text().to_string();
    r.check("double_click", dc_txt == "DOUBLE", dc_txt);

    // ── run_cdp escape hatch ──
    match page
        .run_cdp(
            "Runtime.evaluate",
            serde_json::json!({"expression":"6*7","returnByValue":true}),
        )
        .await
    {
        Ok(val) => {
            let n = val
                .get("result")
                .and_then(|r| r.get("value"))
                .and_then(|v| v.as_i64());
            r.check("run_cdp", n == Some(42), format!("{n:?}"));
        }
        Err(e) => r.check("run_cdp", false, e.to_string()),
    }

    // ── set_offline (the deprecation-fix path: Network.overrideNetworkState) ──
    page.set_offline(true).await?;
    let online_off = page.execute("navigator.onLine").await?;
    r.check(
        "set_offline(true)",
        online_off.as_bool() == Some(false),
        format!("navigator.onLine={online_off}"),
    );
    page.set_offline(false).await?;
    let online_on = page.execute("navigator.onLine").await?;
    r.check(
        "set_offline(false)",
        online_on.as_bool() == Some(true),
        format!("navigator.onLine={online_on}"),
    );

    // ── element states ──
    r.check(
        "is_in_viewport(top)",
        page.ele("#title").await?.is_in_viewport().await,
        "#title visible",
    );
    r.check(
        "is_in_viewport(bottom)=false",
        !page.ele("#bottom").await?.is_in_viewport().await,
        "#bottom off-screen",
    );
    r.check(
        "is_alive",
        page.ele("#title").await?.is_alive().await,
        "#title attached",
    );

    // ── static-snapshot elements (DrissionPage s_ele/s_eles) ──
    match page.s_ele("#title").await {
        Ok(e) => r.check("s_ele", e.text() == "RPage 测试", e.text().to_string()),
        Err(e) => r.check("s_ele", false, e.to_string()),
    }
    match page.s_eles("div").await {
        Ok(v) => r.check("s_eles", v.len() >= 4, format!("{} divs", v.len())),
        Err(e) => r.check("s_eles", false, e.to_string()),
    }

    // ── data_packets (multi-packet harvest, DrissionPage listen.steps()) ──
    let pkts = page.data_packets("/api/data").await;
    r.check(
        "data_packets",
        !pkts.is_empty() && pkts.iter().any(|p| p.body_text().contains("hello")),
        format!("{} packets", pkts.len()),
    );

    // ── @@ same-element AND / @| OR locators (DrissionPage) ──
    match page.ele("tag:div@@class=card").await {
        Ok(e) => r.check("locator @@(AND)", e.text() == "CARD", e.text().to_string()),
        Err(e) => r.check("locator @@(AND)", false, e.to_string()),
    }
    match page.ele("@|id=nope@|data-testid=tid").await {
        Ok(e) => r.check("locator @|(OR)", e.text() == "TID-VAL", e.text().to_string()),
        Err(e) => r.check("locator @|(OR)", false, e.to_string()),
    }

    // ── get_by_* semantic locators (Playwright) ──
    match page.get_by_placeholder("搜索").await {
        Ok(e) => r.check("get_by_placeholder", e.attr("id") == Some("ph"), format!("{:?}", e.attr("id"))),
        Err(e) => r.check("get_by_placeholder", false, e.to_string()),
    }
    match page.get_by_test_id("tid").await {
        Ok(e) => r.check("get_by_test_id", e.text() == "TID-VAL", e.text().to_string()),
        Err(e) => r.check("get_by_test_id", false, e.to_string()),
    }
    match page.get_by_role("button").await {
        Ok(e) => r.check("get_by_role", e.tag() == "button", format!("tag={}", e.tag())),
        Err(e) => r.check("get_by_role", false, e.to_string()),
    }
    match page.get_by_label("用户名").await {
        Ok(e) => r.check("get_by_label", e.attr("id") == Some("lbl"), format!("{:?}", e.attr("id"))),
        Err(e) => r.check("get_by_label", false, e.to_string()),
    }

    // ── actionability: #delayed starts disabled, enables after 600ms;
    //    click() waits for it to become actionable before acting ──
    page.ele("#delayed").await?.click().await?;
    page.sleep(Duration::from_millis(150)).await;
    let dres = page.ele("#dresult").await?.text().to_string();
    r.check("wait_actionable click", dres == "CLICKED", dres);

    // ── route.fulfill: intercept /api/ and fabricate the response (Playwright) ──
    page.get(&base).await?; // reload to clear #result
    let guard = page.enable_intercept("*://*/api/*").await?;
    page.ele("#btn").await?.click().await?;
    let mut filled = false;
    for _ in 0..50 {
        if let Some(req) = guard.paused_requests().first() {
            guard
                .fulfill_json(req.request_id.as_ref(), r#"{"msg":"MOCK","n":7}"#)
                .await?;
            filled = true;
            break;
        }
        page.sleep(Duration::from_millis(100)).await;
    }
    let _ = page
        .wait_js("document.getElementById('result').textContent.length>0", 5)
        .await;
    let fr = page.ele("#result").await?.text().to_string();
    r.check(
        "fulfill_json mock",
        filled && fr == "MOCK7",
        format!("filled={filled} result={fr}"),
    );
    guard.disable().await?;

    page.quit().await?;

    println!("\n=== {} passed, {} failed ===", r.pass, r.fail);
    if r.fail > 0 {
        std::process::exit(1);
    }
    Ok(())
}
