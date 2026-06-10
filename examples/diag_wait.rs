use rpage::prelude::*;

#[tokio::main]
async fn main() -> Result<()> {
    let page = ChromiumPage::connect("http://127.0.0.1:9222").await?;
    page.get("https://www.baidu.com").await?;
    
    // Set viewport first to ensure elements are laid out
    page.set_viewport(1280, 800).await?;
    page.execute("location.reload()").await?;
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    
    println!("Finding #su...");
    let btn = page.wait_ele("#su", 5).await?;
    println!("Found: tag={}, text={}", btn.tag(), btn.text());
    
    println!("Checking visibility...");
    let vis = btn.is_visible().await;
    println!("is_visible: {vis}");
    
    println!("Checking displayed...");
    println!("is_displayed: {}", btn.is_displayed());
    
    println!("Trying wait_for_visible (3s)...");
    match btn.wait_for_visible_with_timeout(std::time::Duration::from_secs(3)).await {
        Ok(()) => println!("wait_for_visible: OK"),
        Err(e) => println!("wait_for_visible: ERR {e}"),
    }
    
    println!("Trying wait_for_clickable (3s)...");
    match btn.wait_for_clickable_with_timeout(std::time::Duration::from_secs(3)).await {
        Ok(()) => println!("wait_for_clickable: OK"),
        Err(e) => println!("wait_for_clickable: ERR {e}"),
    }
    
    Ok(())
}
