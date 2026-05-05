use crate::arena::Tree;
use crate::tags::ElementTag;
use crate::types::NodeId;
use maud::{Markup, PreEscaped};

impl Tree {
    pub fn render(&self) -> Markup {
        self.render_subtree(self.root())
    }

    pub fn render_subtree(&self, root: NodeId) -> Markup {
        let mut out = String::new();
        render_iter(self, root, &mut out);
        PreEscaped(out)
    }
}

enum Step {
    Open(NodeId),
    Close(&'static str),
}

fn render_iter(tree: &Tree, root: NodeId, out: &mut String) {
    let mut stack: Vec<Step> = Vec::with_capacity(64);
    stack.push(Step::Open(root));
    while let Some(step) = stack.pop() {
        match step {
            Step::Open(id) => {
                let node = match tree.get(id) {
                    Some(n) => n,
                    None => continue,
                };
                // #text nodes carry escaped text and nothing else.
                if matches!(node.tag, ElementTag::Text) {
                    if let Some(t) = &node.text {
                        push_escaped(t, out);
                    }
                    continue;
                }
                let tag = node.tag.name();
                out.push('<');
                out.push_str(tag);
                for (name, value) in node.attrs.iter() {
                    out.push(' ');
                    out.push_str(name.as_str());
                    out.push_str("=\"");
                    push_escaped(value, out);
                    out.push('"');
                }
                if is_void(node.tag) {
                    out.push_str(" />");
                    continue;
                }
                out.push('>');
                if let Some(t) = &node.text {
                    push_escaped(t, out);
                }
                stack.push(Step::Close(tag));
                let mut children: Vec<NodeId> = Vec::new();
                let mut c = node.first_child;
                while let Some(cid) = c {
                    children.push(cid);
                    c = tree.get(cid).and_then(|n| n.next_sibling);
                }
                for cid in children.into_iter().rev() {
                    stack.push(Step::Open(cid));
                }
            }
            Step::Close(tag) => {
                out.push_str("</");
                out.push_str(tag);
                out.push('>');
            }
        }
    }
}

fn is_void(tag: ElementTag) -> bool {
    matches!(
        tag,
        ElementTag::Img | ElementTag::Br | ElementTag::Hr | ElementTag::Input
    )
}

fn push_escaped(s: &str, out: &mut String) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#x27;"),
            _ => out.push(c),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::*;

    fn nn(tag: ElementTag) -> NewNode {
        NewNode::new(ElementUuid::new_v4(), tag)
    }

    #[test]
    fn renders_simple_tree() {
        let mut tree = Tree::new(ElementUuid::new_v4(), ElementTag::Div);
        let mut node = nn(ElementTag::P);
        node.text = Some("hello".into());
        let mut attrs = AttrList::new();
        attrs.push(AttrName::new("class").unwrap(), "container");
        node.attrs = attrs;
        tree.append_child(tree.root(), node).unwrap();
        let html = tree.render().into_string();
        assert!(html.starts_with("<div"));
        assert!(html.contains("<p class=\"container\">hello</p>"));
        assert!(html.ends_with("</div>"));
    }

    #[test]
    fn escapes_text_content() {
        let mut tree = Tree::new(ElementUuid::new_v4(), ElementTag::Div);
        let mut n = nn(ElementTag::P);
        n.text = Some("<script>alert(1)</script>".into());
        tree.append_child(tree.root(), n).unwrap();
        let html = tree.render().into_string();
        assert!(!html.contains("<script>"));
        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    }

    #[test]
    fn escapes_attribute_values() {
        let mut tree = Tree::new(ElementUuid::new_v4(), ElementTag::Div);
        let mut node = nn(ElementTag::A);
        let mut attrs = AttrList::new();
        attrs.push(
            AttrName::new("href").unwrap(),
            "https://example.com/?a=\"b\"&c=<d>",
        );
        node.attrs = attrs;
        tree.append_child(tree.root(), node).unwrap();
        let html = tree.render().into_string();
        assert!(html.contains("&quot;b&quot;"));
        assert!(html.contains("&amp;c=&lt;d&gt;"));
    }

    #[test]
    fn renders_void_tags_self_closed() {
        let mut tree = Tree::new(ElementUuid::new_v4(), ElementTag::Div);
        tree.append_child(tree.root(), nn(ElementTag::Br)).unwrap();
        tree.append_child(tree.root(), nn(ElementTag::Hr)).unwrap();
        let html = tree.render().into_string();
        assert!(html.contains("<br />"));
        assert!(html.contains("<hr />"));
    }

    #[test]
    fn iterative_render_does_not_overflow_on_deep_tree() {
        let mut tree = Tree::new(ElementUuid::new_v4(), ElementTag::Div);
        let mut parent = tree.root();
        for _ in 0..10_000 {
            parent = tree.append_child(parent, nn(ElementTag::Div)).unwrap();
        }
        let html = tree.render().into_string();
        // Should be 10001 open tags + matching closes.
        let open_count = html.matches("<div>").count();
        assert!(open_count >= 10_000);
    }

    #[test]
    fn render_subtree_only_renders_subtree() {
        let mut tree = Tree::new(ElementUuid::new_v4(), ElementTag::Div);
        let a = tree
            .append_child(tree.root(), nn(ElementTag::Section))
            .unwrap();
        let mut b = nn(ElementTag::P);
        b.text = Some("inside".into());
        tree.append_child(a, b).unwrap();
        let mut c = nn(ElementTag::P);
        c.text = Some("outside".into());
        tree.append_child(tree.root(), c).unwrap();
        let html = tree.render_subtree(a).into_string();
        assert!(html.contains("inside"));
        assert!(!html.contains("outside"));
    }
}
