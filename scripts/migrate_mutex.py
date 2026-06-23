#!/usr/bin/env python3
"""Migrate ChromiumPage.page from Page to Mutex<Page> — context-aware."""
import re

FILE = r"E:\AICoding\rpage\src\chromium_page.rs"

with open(FILE, "r", encoding="utf-8") as f:
    content = f.read()

lines = content.split('\n')
new_lines = []
brace_depth = 0
in_chromium_page_impl = False
impl_brace_depth = 0
added_helper = False
# Track function scope for launch_and_connect (uses local `page` var)
in_launch_and_connect = False
fn_brace_depth = 0

for i, line in enumerate(lines):
    # Track brace depth
    old_depth = brace_depth
    brace_depth += line.count('{') - line.count('}')

    # Detect impl ChromiumPage {
    if re.match(r'\s*impl ChromiumPage \{', line):
        in_chromium_page_impl = True
        impl_brace_depth = brace_depth
        new_lines.append(line)
        if not added_helper:
            new_lines.append("    /// Returns a clone of the current active page.")
            new_lines.append("    /// Page is Arc<PageInner> internally, so clone is cheap (just Arc refcount++).")
            new_lines.append("    fn page(&self) -> Page {")
            new_lines.append("        self.page.lock().unwrap().clone()")
            new_lines.append("    }")
            new_lines.append("")
            new_lines.append("    /// Replace the current active page with a new one.")
            new_lines.append("    fn set_page(&self, new_page: Page) {")
            new_lines.append("        *self.page.lock().unwrap() = new_page;")
            new_lines.append("    }")
            new_lines.append("")
            added_helper = True
        continue

    # End of impl ChromiumPage block
    if in_chromium_page_impl and brace_depth < impl_brace_depth:
        in_chromium_page_impl = False

    # Detect async fn launch_and_connect (uses local var `page: Self`)
    if re.match(r'\s*async fn launch_and_connect', line):
        in_launch_and_connect = True
        fn_brace_depth = brace_depth
    if in_launch_and_connect and brace_depth < fn_brace_depth:
        in_launch_and_connect = False

    # ── Apply transformations ──

    # Fix struct field definition
    if "pub struct ChromiumPage {" in line:
        new_lines.append(line)
        continue
    if line.strip() == "page: Page," and i > 0 and "browser: Browser," in lines[i-1]:
        new_lines.append("    /// Wrapped in Mutex to allow switching the active tab via `activate_tab`.")
        new_lines.append("    page: std::sync::Mutex<Page>,")
        continue

    # Fix constructor field init: "            page,"
    if line.strip() == "page," and i > 0:
        prev = lines[i-1].strip()
        if prev.startswith("browser:") or prev == "browser,":
            new_lines.append("            page: std::sync::Mutex::new(page),")
            continue

    # Fix assignment "self.page = new.page;"
    if "self.page = new.page;" in line:
        new_lines.append(line.replace("self.page = new.page;", 
            "self.set_page(new.page.lock().unwrap().clone());"))
        continue

    # Fix "page: self.page.clone()," (ClosureData constructors)
    if "page: self.page.clone()," in line:
        new_lines.append(line.replace("page: self.page.clone(),", "page: self.page(),"))
        continue

    # Only transform self.page / .page inside impl ChromiumPage
    if in_chromium_page_impl:
        # Handle "    .page\n" (multi-line chain within ChromiumPage methods)
        if re.match(r'^\s+\.page\s*$', line):
            new_lines.append(line.replace('.page', '.page()'))
            continue

        # Handle "self.page.xxx" same line
        if 'self.page.' in line:
            line = line.replace('self.page.', 'self.page().')

        # Handle "self.page\n" (chain start on same self.page line)
        if re.search(r'self\.page\s*$', line):
            line = re.sub(r'self\.page\s*$', 'self.page()', line)

        # Handle "&self.page" → "&self.page()"
        if '&self.page' in line and '&self.page()' not in line:
            line = line.replace('&self.page', '&self.page()')

        # Handle "self.page)" → "self.page())"
        if 'self.page)' in line and 'self.page())' not in line:
            line = line.replace('self.page)', 'self.page())')

    # Handle launch_and_connect local var: page.page → page.page()
    if in_launch_and_connect:
        if 'page.page.' in line:
            line = line.replace('page.page.', 'page.page().')
        if re.search(r'page\.page\s*$', line):
            line = re.sub(r'page\.page\s*$', 'page.page()', line)
        if '&page.page' in line and '&page.page()' not in line:
            line = line.replace('&page.page', '&page.page()')

    new_lines.append(line)

with open(FILE, "w", encoding="utf-8") as f:
    f.write('\n'.join(new_lines))

print(f"Done! Total lines: {len(new_lines)}")
