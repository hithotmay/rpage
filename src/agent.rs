//! Agent — high-level data structures for AI-agent–driven browser automation.
//!
//! These types describe the information an LLM / AI agent needs to understand a
//! page (summary, snapshot) and the result of an action it requested. They are
//! pure data carriers with no behaviour of their own; methods on
//! [`ChromiumPage`](crate::ChromiumPage) fill them in.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Interactive element
// ---------------------------------------------------------------------------

/// A single interactive (or informative) element on the page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractiveElement {
    /// HTML tag name (lower-case), e.g. `"a"`, `"button"`, `"input"`.
    pub tag: String,
    /// Visible text content of the element (trimmed, max 256 chars).
    pub text: String,
    /// Input type attribute (empty string for non-input elements).
    #[serde(rename = "type")]
    pub input_type: String,
    /// The `name` attribute.
    pub name: String,
    /// Current value of the element.
    pub value: String,
    /// The `placeholder` attribute.
    pub placeholder: String,
    /// The `href` attribute (for links).
    pub href: String,
    /// Whether the element is currently visible.
    pub is_visible: bool,
    /// Bounding box `{x, y, w, h}` in CSS pixels.
    pub rect: Rect,
    /// All HTML attributes of the element.
    pub attributes: HashMap<String, String>,
}

/// Bounding box of an element.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

// ---------------------------------------------------------------------------
// Form structures
// ---------------------------------------------------------------------------

/// A single field inside a [`FormInfo`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormField {
    /// The `name` attribute of the field.
    pub name: String,
    /// Input type (`"text"`, `"email"`, `"password"`, `"checkbox"`, …).
    #[serde(rename = "type")]
    pub field_type: String,
    /// Human-readable label associated with the field (via `<label>`, `aria-label`, …).
    #[serde(default)]
    pub label: String,
    /// Current value of the field.
    #[serde(default)]
    pub value: String,
    /// Whether the field has the `required` attribute.
    #[serde(default)]
    pub required: bool,
    /// For `<select>` elements: the list of option values / texts.
    #[serde(default)]
    pub options: Vec<String>,
}

/// A `<form>` element on the page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormInfo {
    /// The form's `action` URL (resolved to absolute).
    pub action: String,
    /// HTTP method (`"GET"` / `"POST"`).
    pub method: String,
    /// Fields contained in the form.
    pub fields: Vec<FormField>,
}

// ---------------------------------------------------------------------------
// Page-level summaries
// ---------------------------------------------------------------------------

/// Link pair: (visible_text, href).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkInfo {
    pub text: String,
    pub href: String,
}

/// A comprehensive summary of the page — everything an AI agent needs to decide
/// what to do next.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageSummary {
    /// The current URL.
    pub url: String,
    /// Document title.
    pub title: String,
    /// Content of the `<meta name="description">` tag, if present.
    #[serde(default)]
    pub description: Option<String>,
    /// All links on the page as (visible_text, href) pairs.
    pub links: Vec<LinkInfo>,
    /// Forms present on the page.
    pub forms: Vec<FormInfo>,
}

/// A lightweight snapshot of the visible page state, optimised for quick
/// consumption by an LLM context window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageSnapshot {
    /// The current URL.
    pub url: String,
    /// Document title.
    pub title: String,
    /// Viewport size string, e.g. `"1920x1080"`.
    pub viewport_size: String,
    /// Scroll position string, e.g. `"x=0 y=300/5000"`.
    pub scroll_position: String,
    /// Interactive elements visible in the current viewport.
    pub interactive_elements: Vec<InteractiveElement>,
    /// Up to the first 2000 characters of visible body text.
    pub visible_text: String,
}

// ---------------------------------------------------------------------------
// Action result
// ---------------------------------------------------------------------------

/// Outcome of an action attempted by the agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActionAttempt {
    /// Whether the action completed without errors.
    pub success: bool,
    /// Error message if `success` is `false`.
    pub error: Option<String>,
    /// URL before the action was executed.
    pub before_url: String,
    /// URL after the action was executed.
    pub after_url: String,
}
