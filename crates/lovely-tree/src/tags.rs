/// Generates the [`ElementTag`] enum, name lookup, and the `ALL` slice from a
/// flat list of `Variant => "html-name"` entries. Single source of truth for
/// the whitelist of HTML tags this crate knows how to emit.
#[macro_export]
macro_rules! define_tags {
    ( $( $variant:ident => $name:literal ),* $(,)? ) => {
        #[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
        pub enum ElementTag { $( $variant ),* }

        impl ElementTag {
            pub fn from_name(s: &str) -> Option<Self> {
                match s {
                    $( $name => Some(Self::$variant), )*
                    _ => None,
                }
            }

            pub fn name(self) -> &'static str {
                match self {
                    $( Self::$variant => $name, )*
                }
            }

            pub const ALL: &'static [Self] = &[ $( Self::$variant ),* ];
        }
    };
}

impl ElementTag {
    /// True for tags that can't have children: void HTML elements,
    /// form-control elements (input, textarea, select), and the inline
    /// `#text` node. Mirrors lovely Swift's leaf-element constraint.
    pub fn is_leaf(self) -> bool {
        matches!(
            self,
            ElementTag::Text
                | ElementTag::Img
                | ElementTag::Br
                | ElementTag::Hr
                | ElementTag::Input
                | ElementTag::Textarea
                | ElementTag::Select
        )
    }
}

define_tags! {
    // Inline text node — no wrapping element. Renders just the
    // (escaped) `text` payload. Lets authors mix loose text in among
    // their elements: `<p>read <a/> the docs</p>`.
    Text => "#text",
    Div => "div", Section => "section", Article => "article",
    Header => "header", Footer => "footer", Nav => "nav",
    Main => "main", Aside => "aside",
    H1 => "h1", H2 => "h2", H3 => "h3", H4 => "h4", H5 => "h5", H6 => "h6",
    P => "p", Span => "span", Strong => "strong", Em => "em",
    Blockquote => "blockquote", Code => "code", Pre => "pre",
    A => "a", Ul => "ul", Ol => "ol", Li => "li",
    Img => "img", Figure => "figure", Figcaption => "figcaption",
    Table => "table", Thead => "thead", Tbody => "tbody",
    Tr => "tr", Th => "th", Td => "td",
    Form => "form", Input => "input", Textarea => "textarea",
    Select => "select", Button => "button", Label => "label",
    Hr => "hr", Br => "br",
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_tag() {
        assert_eq!(ElementTag::from_name("div"), Some(ElementTag::Div));
        assert_eq!(ElementTag::from_name("section"), Some(ElementTag::Section));
    }

    #[test]
    fn rejects_unknown_tag() {
        assert_eq!(ElementTag::from_name("nonexistent"), None);
    }

    #[test]
    fn renders_tag_name() {
        assert_eq!(ElementTag::Section.name(), "section");
        assert_eq!(ElementTag::Br.name(), "br");
    }

    #[test]
    fn all_tags_roundtrip_through_name() {
        for &tag in ElementTag::ALL {
            assert_eq!(ElementTag::from_name(tag.name()), Some(tag));
        }
        assert!(ElementTag::ALL.len() >= 40);
    }

    #[test]
    fn rejects_dangerous_tags() {
        for bad in ["script", "iframe", "object", "embed", "style"] {
            assert_eq!(
                ElementTag::from_name(bad),
                None,
                "{} should be rejected by whitelist",
                bad
            );
        }
    }
}
