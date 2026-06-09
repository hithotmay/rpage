//! Stealth - anti-detection profiles for browser automation
//!
//! Applies patches to make Chromium less detectable by bot-detection systems:
//! - Realistic User-Agent strings
//! - WebDriver flag removal
//! - Plugin/mime-type spoofing
//! - WebGL vendor spoofing
//! - Navigator properties patching
//! - Chrome-specific runtime patches

use chromiumoxide::Page;
use tracing::{debug, info};

use crate::error::{Error, Result};

/// Stealth profile configuration
#[derive(Debug, Clone)]
pub struct StealthConfig {
    /// Override the User-Agent string
    pub user_agent: Option<String>,
    /// Remove navigator.webdriver property
    pub remove_webdriver: bool,
    /// Spoof navigator.plugins
    pub spoof_plugins: bool,
    /// Spoof navigator.languages
    pub languages: Vec<String>,
    /// Override WebGL vendor/renderer
    pub spoof_webgl: bool,
    /// Hide automation indicators (e.g. chrome.runtime)
    pub hide_automation: bool,
    /// Randomize viewport within bounds
    pub viewport: Option<(u32, u32)>,
}

impl Default for StealthConfig {
    fn default() -> Self {
        Self {
            user_agent: None,
            remove_webdriver: true,
            spoof_plugins: true,
            languages: vec!["en-US".into(), "en".into()],
            spoof_webgl: true,
            hide_automation: true,
            viewport: None,
        }
    }
}

impl StealthConfig {
    /// Create a new stealth config with defaults
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a custom user agent
    pub fn user_agent(mut self, ua: &str) -> Self {
        self.user_agent = Some(ua.to_string());
        self
    }

    /// Set languages
    pub fn languages(mut self, langs: Vec<&str>) -> Self {
        self.languages = langs.iter().map(|s| s.to_string()).collect();
        self
    }

    /// Set viewport size
    pub fn viewport(mut self, width: u32, height: u32) -> Self {
        self.viewport = Some((width, height));
        self
    }
}

/// Default user agent strings by platform
pub mod user_agents {
    /// Chrome on Windows (latest)
    pub const CHROME_WINDOWS: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
    /// Chrome on macOS
    pub const CHROME_MAC: &str = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
    /// Chrome on Linux
    pub const CHROME_LINUX: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
}

/// Apply stealth patches to a Chromium page
pub async fn apply_stealth(page: &Page, config: &StealthConfig) -> Result<()> {
    info!("Applying stealth patches");

    // 1. Remove navigator.webdriver
    if config.remove_webdriver {
        let js = r#"
            Object.defineProperty(navigator, 'webdriver', {
                get: () => false,
                configurable: true
            });
        "#;
        page.evaluate(js)
            .await
            .map_err(|e| Error::Browser(format!("stealth webdriver: {e}")))?;
        debug!("Removed navigator.webdriver");
    }

    // 2. Spoof plugins
    if config.spoof_plugins {
        let js = r#"
            Object.defineProperty(navigator, 'plugins', {
                get: () => {
                    const plugins = [
                        { name: 'Chrome PDF Plugin', filename: 'internal-pdf-viewer', description: 'Portable Document Format' },
                        { name: 'Chrome PDF Viewer', filename: 'mhjfbmdgcfjbbpaeojofohoefgiehjai', description: '' },
                        { name: 'Native Client', filename: 'internal-nacl-plugin', description: '' }
                    ];
                    plugins.length = 3;
                    return plugins;
                },
                configurable: true
            });
        "#;
        page.evaluate(js)
            .await
            .map_err(|e| Error::Browser(format!("stealth plugins: {e}")))?;
        debug!("Spoofed navigator.plugins");
    }

    // 3. Override languages
    if !config.languages.is_empty() {
        let langs = serde_json::to_string(&config.languages)
            .unwrap_or_else(|_| r#"["en-US","en"]"#.to_string());
        let js = format!(
            r#"
            Object.defineProperty(navigator, 'languages', {{
                get: () => {langs},
                configurable: true
            }});
            "#
        );
        page.evaluate(js.as_str())
            .await
            .map_err(|e| Error::Browser(format!("stealth languages: {e}")))?;
        debug!("Override navigator.languages: {:?}", config.languages);
    }

    // 4. Spoof WebGL vendor/renderer
    if config.spoof_webgl {
        let js = r#"
            (function() {
                const getParameter = WebGLRenderingContext.prototype.getParameter;
                WebGLRenderingContext.prototype.getParameter = function(param) {
                    if (param === 37445) return 'Google Inc. (NVIDIA)';
                    if (param === 37446) return 'ANGLE (NVIDIA, NVIDIA GeForce GTX 1060, OpenGL 4.5)';
                    return getParameter.call(this, param);
                };
            })();
        "#;
        page.evaluate(js)
            .await
            .map_err(|e| Error::Browser(format!("stealth webgl: {e}")))?;
        debug!("Spoofed WebGL vendor/renderer");
    }

    // 5. Hide automation indicators
    if config.hide_automation {
        let js = r#"
            (function() {
                // Remove cdc_ variables
                for (let key in document) {
                    if (key.match(/^cdc_/)) {
                        delete document[key];
                    }
                }
                // Override chrome.runtime
                if (!window.chrome) window.chrome = {};
                if (!window.chrome.runtime) {
                    window.chrome.runtime = {
                        connect: function() {},
                        sendMessage: function() {}
                    };
                }
                // Fix permissions query
                const originalQuery = window.navigator.permissions.query;
                window.navigator.permissions.query = (parameters) => (
                    parameters.name === 'notifications' ?
                        Promise.resolve({ state: Notification.permission }) :
                        originalQuery(parameters)
                );
            })();
        "#;
        page.evaluate(js)
            .await
            .map_err(|e| Error::Browser(format!("stealth automation: {e}")))?;
        debug!("Hidden automation indicators");
    }

    // 6. Set user agent if provided
    if let Some(ref ua) = config.user_agent {
        crate::network::set_user_agent(page, ua).await?;
        debug!("Set custom User-Agent");
    }

    info!("Stealth patches applied successfully");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stealth_config_default() {
        let config = StealthConfig::default();
        assert!(config.remove_webdriver);
        assert!(config.spoof_plugins);
        assert!(config.spoof_webgl);
        assert!(config.hide_automation);
        assert_eq!(config.languages, vec!["en-US", "en"]);
    }

    #[test]
    fn test_stealth_config_builder() {
        let config = StealthConfig::new()
            .user_agent(user_agents::CHROME_WINDOWS)
            .viewport(1920, 1080)
            .languages(vec!["zh-CN", "zh", "en"]);

        assert_eq!(
            config.user_agent,
            Some(user_agents::CHROME_WINDOWS.to_string())
        );
        assert_eq!(config.viewport, Some((1920, 1080)));
        assert_eq!(config.languages.len(), 3);
    }
}
