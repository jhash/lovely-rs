use crate::arena::{NewNode, Tree};
use crate::attrs::{AttrList, AttrName};
use crate::errors::TreeError;
use crate::tags::ElementTag;
use crate::types::ElementUuid;
use std::collections::HashMap;

/// Flat row as it comes off the database. The tree is reconstructed in
/// memory by [`Tree::from_db_rows`].
#[derive(Clone, Debug)]
pub struct ElementRow {
    pub id: ElementUuid,
    pub parent_id: Option<ElementUuid>,
    pub prev_sibling: Option<ElementUuid>,
    pub tag: String,
    pub attrs_json: serde_json::Value,
    pub text: Option<String>,
}

impl ElementRow {
    /// True when this row's tag matches the supplied canonical name.
    /// Prefer this over `row.tag == "literal"` so renames have one
    /// place to land.
    pub fn is_tag(&self, name: &str) -> bool {
        self.tag == name
    }

    /// True when this row is the inline `#text` node.
    pub fn is_text(&self) -> bool {
        crate::tags::is_text_tag(&self.tag)
    }
}

impl Tree {
    /// Build a `Tree` from a flat slice of rows. Order of input is irrelevant.
    /// Validates: exactly one root, no orphan parent_id references, no cycles,
    /// no duplicate uuids, sibling chain is consistent.
    pub fn from_db_rows(rows: &[ElementRow]) -> Result<Self, TreeError> {
        // Always synthesize an implicit `#body` root in memory. Every
        // persisted top-level row (parent_id = NULL) becomes a child
        // of this synthetic body. Pages with zero, one, or many
        // top-level elements all take the same code path. The body
        // is never persisted; the renderer emits its children only,
        // no wrapper tag.
        let body_id = ElementUuid::new_v4();
        let mut synth_rows: Vec<ElementRow> = Vec::with_capacity(rows.len() + 1);
        synth_rows.push(ElementRow {
            id: body_id,
            parent_id: None,
            prev_sibling: None,
            tag: ElementTag::BODY_NAME.to_string(),
            attrs_json: serde_json::Value::Object(Default::default()),
            text: None,
        });
        for r in rows {
            let mut copy = r.clone();
            if copy.parent_id.is_none() {
                copy.parent_id = Some(body_id);
            }
            synth_rows.push(copy);
        }
        let rows: &[ElementRow] = &synth_rows;

        // Validate uuid uniqueness on the (synthesized) row set.
        let mut by_id: HashMap<ElementUuid, &ElementRow> = HashMap::new();
        for row in rows {
            if by_id.insert(row.id, row).is_some() {
                return Err(TreeError::DuplicateUuid(row.id));
            }
        }
        let root_row: &ElementRow = by_id
            .get(&body_id)
            .copied()
            .expect("synthetic body row was just inserted");

        // Verify every parent_id references a known row.
        for row in rows {
            if let Some(p) = row.parent_id {
                if !by_id.contains_key(&p) {
                    return Err(TreeError::MalformedRow(format!(
                        "row {} has unknown parent {}",
                        row.id, p
                    )));
                }
            }
        }

        // Group rows by parent_id and order each sibling list via the
        // prev_sibling chain.
        let mut children_by_parent: HashMap<ElementUuid, Vec<&ElementRow>> = HashMap::new();
        for row in rows {
            if let Some(p) = row.parent_id {
                children_by_parent.entry(p).or_default().push(row);
            }
        }
        let mut ordered_children: HashMap<ElementUuid, Vec<&ElementRow>> = HashMap::new();
        for (parent, group) in children_by_parent {
            ordered_children.insert(parent, order_siblings(parent, &group)?);
        }

        // Build the tree by BFS from root. Detects cycles via visited set.
        let root_tag = parse_tag(&root_row.tag)?;
        let mut tree = Tree::new(root_row.id, root_tag);
        // Apply root attrs/text.
        if let Some(attrs) = parse_attrs(&root_row.attrs_json)? {
            patch_root(&mut tree, attrs, root_row.text.clone());
        } else {
            patch_root(&mut tree, AttrList::new(), root_row.text.clone());
        }

        let mut visited: std::collections::HashSet<ElementUuid> = std::collections::HashSet::new();
        visited.insert(root_row.id);
        let mut queue: std::collections::VecDeque<ElementUuid> = std::collections::VecDeque::new();
        queue.push_back(root_row.id);
        while let Some(parent_uuid) = queue.pop_front() {
            let parent_node_id = tree
                .get_by_uuid(parent_uuid)
                .ok_or(TreeError::UnknownUuid(parent_uuid))?;
            if let Some(children) = ordered_children.remove(&parent_uuid) {
                for child_row in children {
                    if !visited.insert(child_row.id) {
                        return Err(TreeError::MalformedRow(format!(
                            "cycle detected at row {}",
                            child_row.id
                        )));
                    }
                    let tag = parse_tag(&child_row.tag)?;
                    let attrs = parse_attrs(&child_row.attrs_json)?.unwrap_or_default();
                    let mut new = NewNode::new(child_row.id, tag);
                    new.attrs = attrs;
                    new.text = child_row.text.clone();
                    tree.append_child(parent_node_id, new)?;
                    queue.push_back(child_row.id);
                }
            }
        }

        // Any rows left unvisited mean orphans (e.g. parent points to a row
        // that itself is unreachable from root) — also a cycle / disconnected.
        if visited.len() != rows.len() {
            return Err(TreeError::MalformedRow(format!(
                "{} row(s) unreachable from root",
                rows.len() - visited.len()
            )));
        }
        Ok(tree)
    }
}

fn parse_tag(s: &str) -> Result<ElementTag, TreeError> {
    ElementTag::from_name(s).ok_or_else(|| TreeError::MalformedRow(format!("unknown tag: {s}")))
}

fn parse_attrs(json: &serde_json::Value) -> Result<Option<AttrList>, TreeError> {
    if json.is_null() {
        return Ok(None);
    }
    let obj = json
        .as_object()
        .ok_or_else(|| TreeError::MalformedRow(format!("attrs not an object: {json}")))?;
    let mut list = AttrList::new();
    for (k, v) in obj {
        // Build-time row parse trusts what's already in the row map:
        // user-supplied names were vetted by `AttrName::new` at PATCH
        // time, while server-injected names (`hx-post`, etc.) come from
        // post-load passes like `auto_wire_forms`. The permissive
        // validator still enforces grammar + length.
        let name = AttrName::from_server_trusted(k)?;
        let value = match v {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Null => String::new(),
            other => {
                return Err(TreeError::MalformedRow(format!(
                    "attr {k} has non-scalar value {other}"
                )))
            }
        };
        list.push(name, value);
    }
    Ok(Some(list))
}

fn patch_root(tree: &mut Tree, attrs: AttrList, text: Option<String>) {
    let root = tree.root();
    tree.update(
        root,
        crate::arena::NodePatch {
            attrs: Some(attrs),
            text: Some(text),
            tag: None,
        },
    )
    .expect("root always exists");
}

/// Sort sibling rows according to the prev_sibling linked list.
///
/// If the chain is broken (duplicate prev_sibling pointers, dangling
/// references, or a cycle), salvage what we can: walk from the head as
/// far as possible, then append unreached rows in a deterministic order
/// (uuid). A corrupt chain is a data bug we want to fix, but it must
/// never take down a page render.
fn order_siblings<'a>(
    _parent: ElementUuid,
    rows: &[&'a ElementRow],
) -> Result<Vec<&'a ElementRow>, TreeError> {
    use std::collections::HashSet;
    // First-row-per-prev: keep insertion-order winner; the rest are
    // appended at the end in uuid order.
    let mut next_of: HashMap<Option<ElementUuid>, &'a ElementRow> = HashMap::new();
    let mut overflow: Vec<&'a ElementRow> = Vec::new();
    for row in rows {
        if let std::collections::hash_map::Entry::Vacant(e) = next_of.entry(row.prev_sibling) {
            e.insert(*row);
        } else {
            // Duplicate prev_sibling — keep the earliest insertion in
            // the chain; the later row goes into overflow.
            overflow.push(*row);
        }
    }
    // Walk from the head (prev = None) as far as the chain takes us.
    let mut ordered: Vec<&'a ElementRow> = Vec::with_capacity(rows.len());
    let mut visited: HashSet<ElementUuid> = HashSet::new();
    let mut cursor: Option<ElementUuid> = None;
    while let Some(row) = next_of.remove(&cursor) {
        if !visited.insert(row.id) {
            // Cycle — bail out and let the salvage step pick up remainder.
            break;
        }
        ordered.push(row);
        cursor = Some(row.id);
    }
    // Append any rows the walk didn't reach (broken chain or no head)
    // and any duplicates from overflow, sorted by uuid for stability.
    let mut leftover: Vec<&'a ElementRow> = next_of
        .into_values()
        .chain(overflow)
        .filter(|r| !visited.contains(&r.id))
        .collect();
    leftover.sort_by_key(|r| r.id);
    ordered.extend(leftover);
    Ok(ordered)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn row(
        id: ElementUuid,
        parent: Option<ElementUuid>,
        prev: Option<ElementUuid>,
        tag: &str,
    ) -> ElementRow {
        ElementRow {
            id,
            parent_id: parent,
            prev_sibling: prev,
            tag: tag.into(),
            attrs_json: json!({}),
            text: None,
        }
    }

    #[test]
    fn builds_tree_from_rows_in_any_input_order() {
        let r = ElementUuid::new_v4();
        let a = ElementUuid::new_v4();
        let b = ElementUuid::new_v4();
        let c = ElementUuid::new_v4();
        let rows = vec![
            row(b, Some(r), Some(a), "p"), // shuffled
            row(c, Some(a), None, "span"),
            row(r, None, None, "div"),
            row(a, Some(r), None, "p"),
        ];
        let tree = Tree::from_db_rows(&rows).unwrap();
        // The synthetic `#body` is the tree root; the persisted "div"
        // is its only child.
        let body_id = tree.root();
        let body_kids: Vec<_> = tree.children(body_id).map(|(_, n)| n.uuid).collect();
        assert_eq!(body_kids, vec![r]);
        let r_id = tree.get_by_uuid(r).unwrap();
        let kids: Vec<_> = tree.children(r_id).map(|(_, n)| n.uuid).collect();
        assert_eq!(kids, vec![a, b]);
        let a_id = tree.get_by_uuid(a).unwrap();
        let a_kids: Vec<_> = tree.children(a_id).map(|(_, n)| n.uuid).collect();
        assert_eq!(a_kids, vec![c]);
    }

    #[test]
    fn rejects_rows_with_orphaned_parent_pointers() {
        // No top-level row, but two rows whose parents point at each
        // other — the body has no children to start from.
        let a = ElementUuid::new_v4();
        let b = ElementUuid::new_v4();
        let rows = vec![row(a, Some(b), None, "div"), row(b, Some(a), None, "div")];
        let err = Tree::from_db_rows(&rows).unwrap_err();
        assert!(matches!(err, TreeError::MalformedRow(_)));
    }

    #[test]
    fn multiple_top_level_rows_become_body_children() {
        // Both rows have parent_id = NULL — the implicit body wraps
        // them as siblings, in prev_sibling order.
        let a = ElementUuid::new_v4();
        let b = ElementUuid::new_v4();
        let rows = vec![row(a, None, None, "div"), row(b, None, Some(a), "p")];
        let tree = Tree::from_db_rows(&rows).expect("multi-root pages render via body");
        let body = tree.root();
        let kids: Vec<_> = tree.children(body).map(|(_, n)| n.uuid).collect();
        assert_eq!(kids, vec![a, b]);
    }

    #[test]
    fn empty_rows_render_to_an_empty_body() {
        // Zero rows is a perfectly valid page — the body has no
        // children. Renderer should emit nothing.
        let rows: Vec<ElementRow> = Vec::new();
        let tree = Tree::from_db_rows(&rows).expect("empty page builds");
        let body = tree.root();
        assert_eq!(tree.children(body).count(), 0);
    }

    #[test]
    fn rejects_rows_with_orphan_parent() {
        let r = ElementUuid::new_v4();
        let a = ElementUuid::new_v4();
        let ghost = ElementUuid::new_v4();
        let rows = vec![row(r, None, None, "div"), row(a, Some(ghost), None, "p")];
        let err = Tree::from_db_rows(&rows).unwrap_err();
        assert!(matches!(err, TreeError::MalformedRow(_)));
    }

    #[test]
    fn rejects_duplicate_uuid_in_input() {
        let r = ElementUuid::new_v4();
        let dup = ElementUuid::new_v4();
        let rows = vec![
            row(r, None, None, "div"),
            row(dup, Some(r), None, "p"),
            row(dup, Some(r), Some(dup), "p"),
        ];
        let err = Tree::from_db_rows(&rows).unwrap_err();
        assert!(matches!(err, TreeError::DuplicateUuid(_)));
    }

    #[test]
    fn rejects_unknown_tag() {
        let r = ElementUuid::new_v4();
        let rows = vec![row(r, None, None, "marquee")];
        let err = Tree::from_db_rows(&rows).unwrap_err();
        assert!(matches!(err, TreeError::MalformedRow(_)));
    }

    #[test]
    fn salvages_duplicate_prev_sibling_chain() {
        // Two children of root claim the same prev_sibling. We should
        // not 500 — render with one in the chain and the dup appended.
        let r = ElementUuid::new_v4();
        let a = ElementUuid::new_v4();
        let b = ElementUuid::new_v4();
        let c = ElementUuid::new_v4();
        let rows = vec![
            row(r, None, None, "div"),
            row(a, Some(r), None, "div"),
            row(b, Some(r), Some(a), "p"),        // chain: a -> b
            row(c, Some(r), Some(a), "textarea"), // dup: also claims prev=a
        ];
        let tree = Tree::from_db_rows(&rows).expect("should not error on dup prev_sibling");
        let r_id = tree.get_by_uuid(r).unwrap();
        let kids: Vec<_> = tree.children(r_id).map(|(_, n)| n.uuid).collect();
        assert_eq!(kids.len(), 3);
        assert_eq!(kids[0], a, "head must be `a` (prev=None)");
        // `b` and `c` both claimed prev=a — one wins the chain, the other
        // is appended. Either order is acceptable as long as both render.
        assert!(kids.contains(&b) && kids.contains(&c));
    }

    #[test]
    fn parses_string_attrs() {
        let r = ElementUuid::new_v4();
        let rows = vec![ElementRow {
            id: r,
            parent_id: None,
            prev_sibling: None,
            tag: "div".into(),
            attrs_json: json!({"id": "main", "class": "container"}),
            text: None,
        }];
        let tree = Tree::from_db_rows(&rows).unwrap();
        let r_id = tree.get_by_uuid(r).unwrap();
        let attrs = &tree.get(r_id).unwrap().attrs;
        assert_eq!(attrs.len(), 2);
    }
}
