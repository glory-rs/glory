use std::fmt;

use crate::web::ClassPart;

/// Scoped CSS block plus the class name that activates it.
///
/// Use the value as a class part and render `css()` into a `<style>` tag:
///
/// ```ignore
/// let scope = glory_core::web::scoped_css("button { color: red; }");
/// style().text(scope.css().to_owned()).show_in(ctx);
/// button().class(scope).text("Save").show_in(ctx);
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScopedStyle {
    class_name: String,
    css: String,
}

impl ScopedStyle {
    pub fn new(css: impl AsRef<str>) -> Self {
        let css = css.as_ref();
        let class_name = format!("gly-scope-{:016x}", stable_hash(css.as_bytes()));
        let css = scope_css_rules(&class_name, css);
        Self { class_name, css }
    }

    pub fn class_name(&self) -> &str {
        &self.class_name
    }

    pub fn css(&self) -> &str {
        &self.css
    }
}

impl ClassPart for ScopedStyle {
    fn to_string(&self) -> Option<String> {
        Some(self.class_name.clone())
    }
}

/// Creates a scoped CSS block with a stable generated class name.
pub fn scoped_css(css: impl AsRef<str>) -> ScopedStyle {
    ScopedStyle::new(css)
}

fn stable_hash(bytes: &[u8]) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;
    bytes.iter().fold(OFFSET, |hash, byte| (hash ^ u64::from(*byte)).wrapping_mul(PRIME))
}

fn scope_css_rules(scope: &str, css: &str) -> String {
    let mut out = String::with_capacity(css.len() + scope.len() * 4);
    let mut rest = css;

    while let Some(open) = rest.find('{') {
        let selector = &rest[..open];
        let Some(close) = matching_brace(&rest[open..]) else {
            out.push_str(rest);
            return out;
        };
        let body = &rest[open + 1..open + close];
        write_rule(scope, selector, body, &mut out);
        rest = &rest[open + close + 1..];
    }

    out.push_str(rest);
    out
}

fn matching_brace(input: &str) -> Option<usize> {
    let mut depth = 0usize;
    for (index, ch) in input.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn write_rule(scope: &str, selector: &str, body: &str, out: &mut String) {
    let trimmed = selector.trim();
    if trimmed.is_empty() {
        out.push_str(selector);
        out.push('{');
        out.push_str(body);
        out.push('}');
        return;
    }

    if is_nested_scope_at_rule(trimmed) {
        out.push_str(selector);
        out.push('{');
        out.push_str(&scope_css_rules(scope, body));
        out.push('}');
    } else if trimmed.starts_with('@') {
        out.push_str(selector);
        out.push('{');
        out.push_str(body);
        out.push('}');
    } else {
        out.push_str(&prefix_selector_list(scope, selector));
        out.push('{');
        out.push_str(body);
        out.push('}');
    }
}

fn is_nested_scope_at_rule(selector: &str) -> bool {
    selector.starts_with("@media") || selector.starts_with("@supports") || selector.starts_with("@container")
}

fn prefix_selector_list(scope: &str, selector: &str) -> String {
    selector
        .split(',')
        .map(|part| prefix_selector(scope, part))
        .collect::<Vec<_>>()
        .join(", ")
}

fn prefix_selector(scope: &str, selector: &str) -> String {
    let selector = selector.trim();
    if let Some(global) = selector.strip_prefix(":global(").and_then(|value| value.strip_suffix(')')) {
        global.trim().to_owned()
    } else if let Some(rest) = selector.strip_prefix(":scope") {
        format!(".{scope}{rest}")
    } else {
        format!(".{scope} {selector}")
    }
}

impl fmt::Display for ScopedStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.class_name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scoped_css_prefixes_regular_selectors() {
        let style = scoped_css("button, a:hover { color: red; }");
        assert!(
            style
                .css()
                .contains(&format!(".{} button, .{} a:hover", style.class_name(), style.class_name()))
        );
    }

    #[test]
    fn scoped_css_supports_scope_global_and_nested_at_rules() {
        let style = scoped_css(":scope > button { color: red; } :global(body) { margin: 0; } @media (min-width: 1px) { p { color: blue; } }");
        assert!(style.css().contains(&format!(".{} > button", style.class_name())));
        assert!(style.css().contains("body{ margin: 0; }"));
        assert!(style.css().contains(&format!("@media (min-width: 1px) {{.{} p", style.class_name())));
    }

    #[test]
    fn scoped_css_hash_is_stable_for_same_input() {
        let a = scoped_css("button { color: red; }");
        let b = scoped_css("button { color: red; }");
        assert_eq!(a.class_name(), b.class_name());
        assert_eq!(a.css(), b.css());
    }
}
