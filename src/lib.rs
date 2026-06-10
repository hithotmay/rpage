//! rpage - A Rust browser automation library inspired by DrissionPage
//!
//! Provides three core objects:
//! - `ChromiumPage`: Browser automation via Chrome DevTools Protocol (CDP)
//! - `SessionPage`: Pure HTTP requests using reqwest
//! - `WebPage`: Combines both, with seamless mode switching and cookie sync

pub mod config;
pub mod cookie_hub;
pub mod download;
pub mod element;
pub mod error;
pub mod locator;
pub mod network;
pub mod session_page;
pub mod stealth;
pub mod wait;

pub mod chromium_page;
pub mod web_page;

pub mod prelude;

// Re-export key types at crate root
pub use chromium_page::ChromiumPage;
pub use chromium_page::{
    ActionChain, CookieInfo, FrameContext, InterceptGuard, InterceptedRequest,
};
pub use config::{ChromiumOptions, SessionOptions, WebPageOptions};
pub use cookie_hub::CookieHub;
pub use download::DownloadManager;
pub use element::Element;
pub use element::ElementBatch;
pub use error::{Error, Result};
pub use locator::{parse_locator, IntoLocator, Locator};
pub use network::NetworkMonitor;
pub use session_page::SessionPage;
pub use stealth::StealthConfig;
pub use wait::WaitOptions;
pub use web_page::{PageMode, WebPage};
