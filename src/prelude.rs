//! Prelude module - re-exports all key types for convenient importing

pub use crate::config::{ChromiumOptions, SessionOptions, WebPageOptions};
pub use crate::cookie_hub::CookieHub;
pub use crate::download::DownloadManager;
pub use crate::element::Element;
pub use crate::element::ElementBatch;
pub use crate::error::{Error, Result};
pub use crate::locator::{parse_locator, IntoLocator, Locator};
pub use crate::network::NetworkMonitor;
pub use crate::stealth::StealthConfig;
pub use crate::wait::WaitOptions;
pub use crate::web_page::{PageMode, WebPage};

// Re-export ChromiumPage
pub use crate::chromium_page::ChromiumPage;
pub use crate::chromium_page::CookieInfo;
