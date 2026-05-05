use crate::attrs::AttrList;
use crate::errors::TreeError;
use crate::tags::ElementTag;
use crate::types::{ElementUuid, NodeId};
use slotmap::SlotMap;
use std::collections::HashMap;

/// Plain-data input used by [`Tree::append_child`], [`Tree::insert_before`],
/// and [`Tree::insert_after`]. Owned values, no references.
#[derive(Clone, Debug)]
pub struct NewNode {
    pub uuid: ElementUuid,
    pub tag: ElementTag,
    pub attrs: AttrList,
    pub text: Option<String>,
}

impl NewNode {
    pub fn new(uuid: ElementUuid, tag: ElementTag) -> Self {
        Self {
            uuid,
            tag,
            attrs: AttrList::new(),
            text: None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Node {
    pub uuid: ElementUuid,
    pub tag: ElementTag,
    pub attrs: AttrList,
    pub text: Option<String>,
    pub parent: Option<NodeId>,
    pub first_child: Option<NodeId>,
    pub last_child: Option<NodeId>,
    pub prev_sibling: Option<NodeId>,
    pub next_sibling: Option<NodeId>,
}

#[derive(Debug)]
pub struct Tree {
    nodes: SlotMap<NodeId, Node>,
    root: NodeId,
    by_uuid: HashMap<ElementUuid, NodeId>,
}

impl Tree {
    pub fn new(root_uuid: ElementUuid, root_tag: ElementTag) -> Self {
        let mut nodes = SlotMap::with_key();
        let root = nodes.insert(Node {
            uuid: root_uuid,
            tag: root_tag,
            attrs: AttrList::new(),
            text: None,
            parent: None,
            first_child: None,
            last_child: None,
            prev_sibling: None,
            next_sibling: None,
        });
        let mut by_uuid = HashMap::new();
        by_uuid.insert(root_uuid, root);
        Self {
            nodes,
            root,
            by_uuid,
        }
    }

    pub fn root(&self) -> NodeId {
        self.root
    }

    pub fn root_uuid(&self) -> ElementUuid {
        self.nodes[self.root].uuid
    }

    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id)
    }

    pub fn get_by_uuid(&self, uuid: ElementUuid) -> Option<NodeId> {
        self.by_uuid.get(&uuid).copied()
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// Debug-only invariant check. No-op in release.
    pub fn debug_assert_invariants(&self) {
        if cfg!(debug_assertions) {
            self.check_invariants().unwrap();
        }
    }

    pub fn append_child(&mut self, parent: NodeId, node: NewNode) -> Result<NodeId, TreeError> {
        self.node_exists(parent)?;
        self.uuid_unused(node.uuid)?;
        let prev = self.nodes[parent].last_child;
        let id = self.insert_node(node, Some(parent), prev, None);
        if let Some(p) = prev {
            self.nodes[p].next_sibling = Some(id);
        } else {
            self.nodes[parent].first_child = Some(id);
        }
        self.nodes[parent].last_child = Some(id);
        self.debug_assert_invariants();
        Ok(id)
    }

    pub fn insert_before(
        &mut self,
        sibling: NodeId,
        node: NewNode,
    ) -> Result<NodeId, TreeError> {
        let target = self.get(sibling).ok_or(TreeError::NotFound(sibling))?;
        let parent = target.parent.ok_or(TreeError::CannotMoveRoot)?;
        let prev = target.prev_sibling;
        self.uuid_unused(node.uuid)?;
        let id = self.insert_node(node, Some(parent), prev, Some(sibling));
        match prev {
            Some(p) => self.nodes[p].next_sibling = Some(id),
            None => self.nodes[parent].first_child = Some(id),
        }
        self.nodes[sibling].prev_sibling = Some(id);
        self.debug_assert_invariants();
        Ok(id)
    }

    pub fn insert_after(
        &mut self,
        sibling: NodeId,
        node: NewNode,
    ) -> Result<NodeId, TreeError> {
        let target = self.get(sibling).ok_or(TreeError::NotFound(sibling))?;
        let parent = target.parent.ok_or(TreeError::CannotMoveRoot)?;
        let next = target.next_sibling;
        self.uuid_unused(node.uuid)?;
        let id = self.insert_node(node, Some(parent), Some(sibling), next);
        match next {
            Some(n) => self.nodes[n].prev_sibling = Some(id),
            None => self.nodes[parent].last_child = Some(id),
        }
        self.nodes[sibling].next_sibling = Some(id);
        self.debug_assert_invariants();
        Ok(id)
    }

    fn insert_node(
        &mut self,
        n: NewNode,
        parent: Option<NodeId>,
        prev: Option<NodeId>,
        next: Option<NodeId>,
    ) -> NodeId {
        let uuid = n.uuid;
        let id = self.nodes.insert(Node {
            uuid: n.uuid,
            tag: n.tag,
            attrs: n.attrs,
            text: n.text,
            parent,
            first_child: None,
            last_child: None,
            prev_sibling: prev,
            next_sibling: next,
        });
        self.by_uuid.insert(uuid, id);
        id
    }

    fn node_exists(&self, id: NodeId) -> Result<(), TreeError> {
        if self.nodes.contains_key(id) {
            Ok(())
        } else {
            Err(TreeError::NotFound(id))
        }
    }

    fn uuid_unused(&self, uuid: ElementUuid) -> Result<(), TreeError> {
        if self.by_uuid.contains_key(&uuid) {
            Err(TreeError::DuplicateUuid(uuid))
        } else {
            Ok(())
        }
    }

    pub(crate) fn check_invariants(&self) -> Result<(), String> {
        for (uuid, id) in &self.by_uuid {
            let node = self
                .nodes
                .get(*id)
                .ok_or_else(|| format!("dangling id {id:?} for uuid {uuid}"))?;
            if node.uuid != *uuid {
                return Err(format!(
                    "uuid mismatch at {id:?}: node has {} but indexed under {}",
                    node.uuid, uuid
                ));
            }
        }
        for (id, node) in &self.nodes {
            if self.by_uuid.get(&node.uuid) != Some(&id) {
                return Err(format!("node {id:?} (uuid {}) not in by_uuid", node.uuid));
            }
            // Sibling links must be reciprocal.
            if let Some(p) = node.prev_sibling {
                let prev = self
                    .nodes
                    .get(p)
                    .ok_or_else(|| format!("{id:?} prev_sibling {p:?} dangling"))?;
                if prev.next_sibling != Some(id) {
                    return Err(format!("{id:?}.prev_sibling -> {p:?} not reciprocal"));
                }
                if prev.parent != node.parent {
                    return Err(format!("{id:?} and prev sibling {p:?} have different parents"));
                }
            }
            if let Some(n) = node.next_sibling {
                let next = self
                    .nodes
                    .get(n)
                    .ok_or_else(|| format!("{id:?} next_sibling {n:?} dangling"))?;
                if next.prev_sibling != Some(id) {
                    return Err(format!("{id:?}.next_sibling -> {n:?} not reciprocal"));
                }
            }
            // Parent's first/last child claims must be consistent.
            if let Some(p) = node.parent {
                let parent = self
                    .nodes
                    .get(p)
                    .ok_or_else(|| format!("{id:?} parent {p:?} dangling"))?;
                if node.prev_sibling.is_none() && parent.first_child != Some(id) {
                    return Err(format!("{id:?} has no prev_sibling but parent.first_child != self"));
                }
                if node.next_sibling.is_none() && parent.last_child != Some(id) {
                    return Err(format!("{id:?} has no next_sibling but parent.last_child != self"));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_tree_has_root() {
        let root = ElementUuid::new_v4();
        let tree = Tree::new(root, ElementTag::Div);
        assert_eq!(tree.root_uuid(), root);
        assert!(tree.get_by_uuid(root).is_some());
        assert_eq!(tree.len(), 1);
    }

    #[test]
    fn get_by_uuid_returns_none_for_unknown() {
        let tree = Tree::new(ElementUuid::new_v4(), ElementTag::Div);
        assert!(tree.get_by_uuid(ElementUuid::new_v4()).is_none());
    }

    #[test]
    fn root_node_has_no_parent_or_siblings() {
        let tree = Tree::new(ElementUuid::new_v4(), ElementTag::Div);
        let root_id = tree.root();
        let root = tree.get(root_id).unwrap();
        assert_eq!(root.parent, None);
        assert_eq!(root.first_child, None);
        assert_eq!(root.last_child, None);
        assert_eq!(root.prev_sibling, None);
        assert_eq!(root.next_sibling, None);
    }

    #[test]
    fn invariants_hold_on_fresh_tree() {
        let tree = Tree::new(ElementUuid::new_v4(), ElementTag::Div);
        tree.debug_assert_invariants();
    }

    fn fresh() -> (Tree, NodeId) {
        let tree = Tree::new(ElementUuid::new_v4(), ElementTag::Div);
        let root = tree.root();
        (tree, root)
    }

    fn nn(tag: ElementTag) -> NewNode {
        NewNode::new(ElementUuid::new_v4(), tag)
    }

    #[test]
    fn append_child_links_correctly() {
        let (mut tree, root) = fresh();
        let a = tree.append_child(root, nn(ElementTag::P)).unwrap();
        let b = tree.append_child(root, nn(ElementTag::P)).unwrap();
        assert_eq!(tree.get(root).unwrap().first_child, Some(a));
        assert_eq!(tree.get(root).unwrap().last_child, Some(b));
        assert_eq!(tree.get(a).unwrap().next_sibling, Some(b));
        assert_eq!(tree.get(b).unwrap().prev_sibling, Some(a));
        assert_eq!(tree.get(a).unwrap().parent, Some(root));
        tree.debug_assert_invariants();
    }

    #[test]
    fn insert_before_inserts_in_middle() {
        let (mut tree, root) = fresh();
        let a = tree.append_child(root, nn(ElementTag::P)).unwrap();
        let c = tree.append_child(root, nn(ElementTag::P)).unwrap();
        let b = tree.insert_before(c, nn(ElementTag::P)).unwrap();
        assert_eq!(tree.get(a).unwrap().next_sibling, Some(b));
        assert_eq!(tree.get(b).unwrap().prev_sibling, Some(a));
        assert_eq!(tree.get(b).unwrap().next_sibling, Some(c));
        assert_eq!(tree.get(c).unwrap().prev_sibling, Some(b));
        assert_eq!(tree.get(root).unwrap().first_child, Some(a));
        assert_eq!(tree.get(root).unwrap().last_child, Some(c));
        tree.debug_assert_invariants();
    }

    #[test]
    fn insert_before_at_head_updates_first_child() {
        let (mut tree, root) = fresh();
        let b = tree.append_child(root, nn(ElementTag::P)).unwrap();
        let a = tree.insert_before(b, nn(ElementTag::P)).unwrap();
        assert_eq!(tree.get(root).unwrap().first_child, Some(a));
        assert_eq!(tree.get(a).unwrap().prev_sibling, None);
        tree.debug_assert_invariants();
    }

    #[test]
    fn insert_after_at_end_updates_last_child() {
        let (mut tree, root) = fresh();
        let a = tree.append_child(root, nn(ElementTag::P)).unwrap();
        let b = tree.insert_after(a, nn(ElementTag::P)).unwrap();
        assert_eq!(tree.get(root).unwrap().last_child, Some(b));
        assert_eq!(tree.get(b).unwrap().next_sibling, None);
        tree.debug_assert_invariants();
    }

    #[test]
    fn append_child_rejects_duplicate_uuid() {
        let (mut tree, root) = fresh();
        let dup = ElementUuid::new_v4();
        tree.append_child(root, NewNode::new(dup, ElementTag::P)).unwrap();
        let err = tree
            .append_child(root, NewNode::new(dup, ElementTag::P))
            .unwrap_err();
        assert!(matches!(err, TreeError::DuplicateUuid(_)));
    }

    #[test]
    fn insert_before_root_fails() {
        let (mut tree, root) = fresh();
        let err = tree.insert_before(root, nn(ElementTag::P)).unwrap_err();
        assert!(matches!(err, TreeError::CannotMoveRoot));
    }

    #[test]
    fn many_children_append_in_order() {
        let (mut tree, root) = fresh();
        let ids: Vec<NodeId> = (0..50)
            .map(|_| tree.append_child(root, nn(ElementTag::Li)).unwrap())
            .collect();
        let mut walk = tree.get(root).unwrap().first_child;
        for expected in &ids {
            assert_eq!(walk, Some(*expected));
            walk = tree.get(walk.unwrap()).unwrap().next_sibling;
        }
        assert_eq!(walk, None);
        tree.debug_assert_invariants();
    }
}
