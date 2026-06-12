//! JavaScript code snippets for agent functionality.
//!
//! Each constant/function returns a `&str` or `String` containing JavaScript
//! that can be executed in the browser via `ChromiumPage::execute()`.
//! All snippets return JSON-serialisable data.

/// Traverse all interactive elements on the page and return a JSON array.
///
/// Each item contains: `{tag, text, type, name, value, placeholder, href,
/// is_visible, rect:{x,y,w,h}, attributes:{...}}`.
pub const JS_INTERACTIVE_ELEMENTS: &str = r#"
(function() {
    var selector = 'a, button, input, select, textarea, [role="button"], [onclick]';
    var els = document.querySelectorAll(selector);
    var result = [];
    for (var i = 0; i < els.length; i++) {
        var el = els[i];
        var tag = el.tagName.toLowerCase();
        var text = (el.textContent || '').trim().substring(0, 256);
        var rect = el.getBoundingClientRect();
        var isVisible = (el.offsetParent !== null || rect.height > 0);
        var attrs = {};
        for (var j = 0; j < el.attributes.length; j++) {
            var a = el.attributes[j];
            if (typeof el[a.name] !== 'function') {
                attrs[a.name] = a.value;
            }
        }
        result.push({
            tag: tag,
            text: text,
            type: el.getAttribute('type') || '',
            name: el.getAttribute('name') || '',
            value: el.value || '',
            placeholder: el.getAttribute('placeholder') || '',
            href: el.getAttribute('href') || '',
            is_visible: isVisible,
            rect: {
                x: rect.x,
                y: rect.y,
                w: rect.width,
                h: rect.height
            },
            attributes: attrs
        });
    }
    return JSON.stringify(result);
})();
"#;

/// Return a page summary: `{url, title, description, links, forms}`.
pub const JS_PAGE_SUMMARY: &str = r#"
(function() {
    var desc = '';
    var meta = document.querySelector('meta[name="description"]');
    if (meta) desc = meta.getAttribute('content') || '';

    var links = [];
    var anchors = document.querySelectorAll('a[href]');
    for (var i = 0; i < anchors.length; i++) {
        var a = anchors[i];
        links.push({
            text: (a.textContent || '').trim().substring(0, 256),
            href: a.getAttribute('href') || ''
        });
    }

    var forms = [];
    var formEls = document.querySelectorAll('form');
    for (var f = 0; f < formEls.length; f++) {
        var form = formEls[f];
        var fields = [];
        var inputs = form.querySelectorAll('input, select, textarea');
        for (var k = 0; k < inputs.length; k++) {
            fields.push({
                name: inputs[k].getAttribute('name') || '',
                type: inputs[k].getAttribute('type') || inputs[k].tagName.toLowerCase()
            });
        }
        forms.push({
            action: form.getAttribute('action') || '',
            method: (form.getAttribute('method') || 'GET').toUpperCase(),
            fields: fields
        });
    }

    return JSON.stringify({
        url: window.location.href,
        title: document.title || '',
        description: desc,
        links: links,
        forms: forms
    });
})();
"#;

/// Extract form field details including labels and select options.
///
/// Returns a JSON array of `{name, type, label, value, required, options}`.
pub const JS_FORM_FIELDS: &str = r#"
(function() {
    var fields = [];
    var inputs = document.querySelectorAll('input, select, textarea');
    for (var i = 0; i < inputs.length; i++) {
        var el = inputs[i];
        var name = el.getAttribute('name') || '';
        var type = el.getAttribute('type') || el.tagName.toLowerCase();
        var value = el.value || '';
        var required = el.hasAttribute('required');

        // Resolve label via for-attribute or parent <label>
        var label = '';
        var id = el.getAttribute('id');
        if (id) {
            var lbl = document.querySelector('label[for="' + id + '"]');
            if (lbl) label = (lbl.textContent || '').trim();
        }
        if (!label) {
            var parent = el.parentElement;
            if (parent && parent.tagName && parent.tagName.toLowerCase() === 'label') {
                label = (parent.textContent || '').trim();
            }
        }

        // Collect <option> elements for <select>
        var options = [];
        if (el.tagName.toLowerCase() === 'select') {
            var opts = el.querySelectorAll('option');
            for (var j = 0; j < opts.length; j++) {
                options.push({
                    value: opts[j].getAttribute('value') || '',
                    text: (opts[j].textContent || '').trim()
                });
            }
        }

        fields.push({
            name: name,
            type: type,
            label: label,
            value: value,
            required: required,
            options: options
        });
    }
    return JSON.stringify(fields);
})();
"#;

// NOTE: The helpers below embed the search text as a safe JS string literal
//       and then use it inside an XPath expression at runtime.

/// Return the first element whose text content contains `text`.
///
/// Uses `document.evaluate` with XPath `contains(text(), ...)`.
pub fn js_find_by_text(text: &str) -> String {
    let escaped = text.replace('\\', "\\\\").replace('\'', "\\'").replace('"', "\\\"");
    format!(
        r#"(function() {{
    var txt = "{escaped}";
    var xpath = "//*[contains(text(), " + JSON.stringify(txt) + ")]";
    var result = document.evaluate(xpath, document, null, XPathResult.FIRST_ORDERED_NODE_TYPE, null);
    var el = result.singleNodeValue;
    return el ? el.outerHTML : null;
}})();"#
    )
}

/// Return all elements whose text content contains `text`.
///
/// Uses `document.evaluate` with `XPathResult.ORDERED_NODE_SNAPSHOT_TYPE`.
pub fn js_find_all_by_text(text: &str) -> String {
    let escaped = text.replace('\\', "\\\\").replace('"', "\\\"");
    format!(
        r#"(function() {{
    var txt = "{escaped}";
    var xpath = "//*[contains(text(), " + JSON.stringify(txt) + ")]";
    var result = document.evaluate(xpath, document, null, XPathResult.ORDERED_NODE_SNAPSHOT_TYPE, null);
    var items = [];
    for (var i = 0; i < result.snapshotLength; i++) {{
        items.push(result.snapshotItem(i).outerHTML);
    }}
    return JSON.stringify(items);
}})();"#
    )
}

/// Return elements whose attribute `attr` contains `value`.
pub fn js_find_by_attr(attr: &str, value: &str) -> String {
    let escaped_attr = attr.replace('\\', "\\\\").replace('"', "\\\"");
    let escaped_value = value.replace('\\', "\\\\").replace('"', "\\\"");
    format!(
        r#"(function() {{
    var attr = "{escaped_attr}";
    var val = "{escaped_value}";
    var sel = "[" + attr + "*=" + JSON.stringify(val) + "]";
    var els = document.querySelectorAll(sel);
    var items = [];
    for (var i = 0; i < els.length; i++) {{
        items.push({{
            tag: els[i].tagName.toLowerCase(),
            outerHTML: els[i].outerHTML,
            attrValue: els[i].getAttribute(attr)
        }});
    }}
    return JSON.stringify(items);
}})();"#
    )
}

/// Extract visible text from `<body>`, excluding `<script>` and `<style>`.
///
/// Returns at most `max_len` characters.
pub fn js_visible_text(max_len: usize) -> String {
    format!(
        r#"(function() {{
    var clone = document.body.cloneNode(true);
    var remove = clone.querySelectorAll('script, style, noscript');
    for (var i = 0; i < remove.length; i++) {{
        remove[i].parentNode.removeChild(remove[i]);
    }}
    var text = (clone.textContent || '').replace(/\s+/g, ' ').trim();
    return text.substring(0, {max_len});
}})();"#
    )
}

/// Return the current scroll state: `{scrollX, scrollY, scrollHeight, clientHeight}`.
pub const JS_SCROLL_STATE: &str = r#"
(function() {
    return JSON.stringify({
        scrollX: window.pageXOffset || document.documentElement.scrollLeft,
        scrollY: window.pageYOffset || document.documentElement.scrollTop,
        scrollHeight: document.documentElement.scrollHeight,
        clientHeight: document.documentElement.clientHeight
    });
})();
"#;

/// Safely click an element by dispatching a `mousedown` → `mouseup` → `click`
/// event sequence.
///
/// The caller should supply the element reference or a selector that resolves
/// to a single element.  This constant expects a variable named `__el` in scope
/// that already points to the target DOM element.
pub const JS_CLICK_ELEMENT: &str = r#"
(function(el) {
    if (!el) return false;
    var events = ['mousedown', 'mouseup', 'click'];
    for (var i = 0; i < events.length; i++) {
        var evt = new MouseEvent(events[i], {
            bubbles: true,
            cancelable: true,
            view: window
        });
        el.dispatchEvent(evt);
    }
    return true;
})(arguments[0]);
"#;

/// Safely type text into an input element: focus → set value → dispatch
/// `input` + `change` events.
///
/// Expects `arguments[0]` = the element and `arguments[1]` = the text to type.
pub const JS_SAFE_TYPE: &str = r#"
(function(el, text) {
    if (!el) return false;
    el.focus();
    el.value = text;
    var inputEvt = new Event('input', { bubbles: true });
    el.dispatchEvent(inputEvt);
    var changeEvt = new Event('change', { bubbles: true });
    el.dispatchEvent(changeEvt);
    return true;
})(arguments[0], arguments[1]);
"#;
