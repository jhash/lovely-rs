//! Tiny shared view fragments. One-purpose helpers — keep them small.

use maud::{html, Markup};

/// Inline label + checkbox. Renders as a single row regardless of the
/// surrounding form's flex direction (the parent `.inspector-form`
/// stacks labels vertically by default; this overrides that).
///
/// `name` is the form field, `label` is the visible text, `checked`
/// pre-selects it. The input value is the canonical `"on"` so existing
/// `truthy()` helpers parse it.
pub fn labeled_checkbox(name: &str, label: &str, checked: bool) -> Markup {
    html! {
        label .checkbox-row {
            input type="checkbox" name=(name) value="on" checked[checked];
            span { (label) }
        }
    }
}
