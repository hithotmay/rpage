//! Smoke test for the synchronous API (`SyncPage`/`SyncElement`).
//!
//! Runs in a plain `fn main` (no tokio) against a local server, to verify the
//! `block_on` wrappers — including ones that await + sleep internally like
//! `wait_data_packet` — don't deadlock and the newly-added methods work.
//!
//! Run with: `cargo run --example sync_smoke`

use std::io::{Read, Write};
use std::net::TcpListener;
use std::thread;

use rpage::config::ChromiumOptions;
use rpage::sync::SyncPage;

const HTML: &str = r#"<!doctype html><meta charset="utf-8"><title>Sync Smoke</title>
<h1 id="title">RPage 测试</h1>
<input id="name"><button id="btn">L</button><div id="result"></div>
<script>
document.getElementById('btn').onclick = async () => {
  const r = await fetch('/api/data');
  const j = await r.json();
  document.getElementById('result').textContent = j.msg;
};
console.log('sync-smoke-ready');
</script>"#;

fn start_server() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
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
                ("application/json", r#"{"msg":"hello-sync"}"#.to_string())
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

fn main() -> rpage::Result<()> {
    let port = start_server();
    let base = format!("http://127.0.0.1:{port}");
    let chrome = std::env::var("RPAGE_CHROME_PATH")
        .unwrap_or_else(|_| r"C:\Program Files\Google\Chrome\Application\chrome.exe".into());

    let opts = ChromiumOptions::builder()
        .headless(true)
        .browser_path(chrome)
        .debug_port(9872)
        .user_data_dir(std::env::temp_dir().join("rpage-sync-smoke"))
        .build();

    let p = SyncPage::with_options(opts)?;
    let (mut pass, mut fail) = (0u32, 0u32);
    let mut check = |name: &str, ok: bool, detail: String| {
        if ok {
            pass += 1;
            println!("  PASS  {name}  ({detail})");
        } else {
            fail += 1;
            println!("  FAIL  {name}  ({detail})");
        }
    };

    p.get(&base)?;
    check("title", p.title()? == "Sync Smoke", p.title()?);
    check(
        "ele.text",
        p.ele("#title")?.text() == "RPage 测试",
        p.ele("#title")?.text().into(),
    );
    check(
        "s_ele.text",
        p.s_ele("#title")?.text() == "RPage 测试",
        p.s_ele("#title")?.text().into(),
    );
    check(
        "tab_count",
        p.tab_count()? >= 1,
        format!("{}", p.tab_count()?),
    );
    check(
        "load_strategy",
        !p.load_strategy().is_empty(),
        p.load_strategy().into(),
    );
    check(
        "is_in_viewport",
        p.ele("#title")?.is_in_viewport(),
        "#title".into(),
    );
    check("is_alive", p.ele("#title")?.is_alive(), "#title".into());

    // console capture (the page logs on load)
    let logs = p.console_log();
    check(
        "console_log",
        logs.iter().any(|l| l.text.contains("sync-smoke-ready")),
        format!("{} entries", logs.len()),
    );

    // click → fetch → wait_data_packet (the deadlock-risk path)
    p.ele("#btn")?.click()?;
    match p.wait_data_packet("/api/data", 5) {
        Ok(pkt) => check(
            "wait_data_packet",
            pkt.body_text().contains("hello-sync"),
            format!("{} {}", pkt.status, pkt.body_text()),
        ),
        Err(e) => check("wait_data_packet", false, e.to_string()),
    }

    // SyncInterceptGuard: reload (clears #result), then block + reject the
    // /api/ request so the fetch fails and #result stays empty.
    p.get(&base)?;
    let guard = p.enable_intercept("*://*/api/*")?;
    p.ele("#btn")?.click()?;
    let mut blocked = false;
    for _ in 0..50 {
        if let Some(req) = guard.paused_requests().first() {
            guard.fail_request(req.request_id.as_ref())?;
            blocked = true;
            break;
        }
        p.sleep(std::time::Duration::from_millis(100));
    }
    p.sleep(std::time::Duration::from_millis(300));
    let blocked_result = p.ele("#result")?.text().to_string();
    check(
        "intercept fail_request",
        blocked && blocked_result.is_empty(),
        format!("blocked={blocked} result='{blocked_result}'"),
    );
    guard.disable()?;

    p.quit()?;
    println!("\n=== {pass} passed, {fail} failed ===");
    if fail > 0 {
        std::process::exit(1);
    }
    Ok(())
}
