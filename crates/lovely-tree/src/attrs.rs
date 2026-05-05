use crate::errors::TreeError;
use smol_str::SmolStr;

/// Validated HTML attribute name. Constructed only via [`AttrName::new`],
/// which enforces the grammar and a denylist for `on*` event handlers and
/// `hx-*` (htmx is server-controlled, never user-author-controlled).
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct AttrName(SmolStr);

impl AttrName {
    pub fn new(s: &str) -> Result<Self, TreeError> {
        if s.is_empty() || s.len() > 64 {
            return Err(TreeError::InvalidAttribute(s.into()));
        }
        let mut chars = s.chars();
        let first = chars.next().unwrap();
        if !first.is_ascii_alphabetic() {
            return Err(TreeError::InvalidAttribute(s.into()));
        }
        for c in chars {
            if !(c.is_ascii_alphanumeric() || c == '-' || c == '_') {
                return Err(TreeError::InvalidAttribute(s.into()));
            }
        }
        let lower = s.to_ascii_lowercase();
        if lower.starts_with("on") || lower.starts_with("hx-") {
            return Err(TreeError::InvalidAttribute(s.into()));
        }
        Ok(Self(SmolStr::new(s)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Ordered list of `(name, value)` attribute pairs. Order is preserved at
/// render time so authors can rely on, e.g., `class` appearing before
/// `style` if they want to.
#[derive(Clone, Debug, Default)]
pub struct AttrList {
    entries: Vec<(AttrName, String)>,
}

impl AttrList {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, name: AttrName, value: impl Into<String>) {
        self.entries.push((name, value.into()));
    }

    pub fn iter(&self) -> impl Iterator<Item = (&AttrName, &str)> {
        self.entries.iter().map(|(n, v)| (n, v.as_str()))
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_attr_names() {
        for n in ["id", "class", "data-foo", "aria-label", "x-1", "rel"] {
            assert!(AttrName::new(n).is_ok(), "{} should be accepted", n);
        }
    }

    #[test]
    fn rejects_invalid_attr_names() {
        for n in ["", "1foo", "foo bar", "foo<", "javascript:url", "&"] {
            assert!(AttrName::new(n).is_err(), "{} should be rejected", n);
        }
    }

    #[test]
    fn rejects_overlong_names() {
        let s = "a".repeat(65);
        assert!(AttrName::new(&s).is_err());
    }

    #[test]
    fn denies_event_handler_attrs() {
        for n in ["onclick", "onload", "onerror", "onmouseover", "ONCLICK"] {
            assert!(AttrName::new(n).is_err(), "{} should be denied", n);
        }
    }

    #[test]
    fn denies_htmx_attrs_in_user_provided() {
        for n in ["hx-get", "hx-post", "hx-swap", "hx-target", "HX-GET"] {
            assert!(AttrName::new(n).is_err(), "{} should be denied", n);
        }
    }

    #[test]
    fn list_preserves_order() {
        let mut a = AttrList::new();
        a.push(AttrName::new("id").unwrap(), "x");
        a.push(AttrName::new("class").unwrap(), "y");
        let collected: Vec<_> = a.iter().map(|(n, v)| (n.as_str(), v)).collect();
        assert_eq!(collected, vec![("id", "x"), ("class", "y")]);
    }
}
