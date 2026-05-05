use crate::attrs::AttrList;
use crate::errors::TreeError;
use crate::tags::ElementTag;
use crate::types::{ElementUuid, NodeId};
use slotmap::SlotMap;
use std::collections::HashMap;

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
                return Err(format!(
                    "node {id:?} (uuid {}) not in by_uuid",
                    node.uuid
                ));
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
}
