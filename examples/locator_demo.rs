//! Locator syntax demo
use rpage::locator::parse_locator;

fn main() -> rpage::Result<()> {
    // CSS selectors
    let loc = parse_locator("#myid")?;
    println!("{loc:?}");

    let loc = parse_locator(".container > p")?;
    println!("{loc:?}");

    // XPath
    let loc = parse_locator("xpath://div[@class='content']")?;
    println!("{loc:?}");

    // Text match
    let loc = parse_locator("text=Login")?;
    println!("{loc:?}");

    // Text contains
    let loc = parse_locator("text*=Submit")?;
    println!("{loc:?}");

    // Attribute equals
    let loc = parse_locator("@name=username")?;
    println!("{loc:?}");

    // Attribute contains
    let loc = parse_locator("@href*=logout")?;
    println!("{loc:?}");

    // Chained locators
    let loc = parse_locator("tag:form@@text=Submit")?;
    println!("{loc:?}");

    // Convert to XPath
    let loc = parse_locator("#foo")?;
    if let Some(xpath) = loc.to_xpath() {
        println!("CSS -> XPath: {xpath}");
    }

    Ok(())
}
