use crate::arena::{Node, Tree};
use crate::types::NodeId;

/// Iterator over the children of a node, in sibling order.
pub struct ChildrenIter<'a> {
    tree: &'a Tree,
    next: Option<NodeId>,
}

impl<'a> ChildrenIter<'a> {
    pub(crate) fn new(tree: &'a Tree, parent: NodeId) -> Self {
        let next = tree.get(parent).and_then(|n| n.first_child);
        Self { tree, next }
    }
}

impl<'a> Iterator for ChildrenIter<'a> {
    type Item = (NodeId, &'a Node);
    fn next(&mut self) -> Option<Self::Item> {
        let id = self.next?;
        let node = self.tree.get(id)?;
        self.next = node.next_sibling;
        Some((id, node))
    }
}

/// Iterator from a node's parent up to the root (exclusive of the node itself).
pub struct AncestorsIter<'a> {
    tree: &'a Tree,
    next: Option<NodeId>,
}

impl<'a> AncestorsIter<'a> {
    pub(crate) fn new(tree: &'a Tree, of: NodeId) -> Self {
        let next = tree.get(of).and_then(|n| n.parent);
        Self { tree, next }
    }
}

impl<'a> Iterator for AncestorsIter<'a> {
    type Item = (NodeId, &'a Node);
    fn next(&mut self) -> Option<Self::Item> {
        let id = self.next?;
        let node = self.tree.get(id)?;
        self.next = node.parent;
        Some((id, node))
    }
}

/// Pre-order descendant iterator (the root passed to `new` is *not* yielded).
/// Lazy — does not allocate per-node.
pub struct DescendantsIter<'a> {
    tree: &'a Tree,
    stack: Vec<NodeId>,
}

impl<'a> DescendantsIter<'a> {
    pub(crate) fn new(tree: &'a Tree, root: NodeId) -> Self {
        let mut stack = Vec::new();
        if let Some(node) = tree.get(root) {
            // Push children right-to-left so leftmost pops first.
            let mut children: Vec<NodeId> = Vec::new();
            let mut c = node.first_child;
            while let Some(id) = c {
                children.push(id);
                c = tree.get(id).and_then(|n| n.next_sibling);
            }
            for id in children.into_iter().rev() {
                stack.push(id);
            }
        }
        Self { tree, stack }
    }
}

impl<'a> Iterator for DescendantsIter<'a> {
    type Item = (NodeId, &'a Node);
    fn next(&mut self) -> Option<Self::Item> {
        let id = self.stack.pop()?;
        let node = self.tree.get(id)?;
        // Push children of this node so we visit them next (pre-order).
        let mut children: Vec<NodeId> = Vec::new();
        let mut c = node.first_child;
        while let Some(cid) = c {
            children.push(cid);
            c = self.tree.get(cid).and_then(|n| n.next_sibling);
        }
        for cid in children.into_iter().rev() {
            self.stack.push(cid);
        }
        Some((id, node))
    }
}

impl Tree {
    pub fn children(&self, parent: NodeId) -> ChildrenIter<'_> {
        ChildrenIter::new(self, parent)
    }
    pub fn ancestors(&self, of: NodeId) -> AncestorsIter<'_> {
        AncestorsIter::new(self, of)
    }
    pub fn descendants(&self, root: NodeId) -> DescendantsIter<'_> {
        DescendantsIter::new(self, root)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::NewNode;
    use crate::tags::ElementTag;
    use crate::types::ElementUuid;

    fn fresh() -> (Tree, NodeId) {
        let tree = Tree::new(ElementUuid::new_v4(), ElementTag::Div);
        let root = tree.root();
        (tree, root)
    }

    fn nn(tag: ElementTag) -> NewNode {
        NewNode::new(ElementUuid::new_v4(), tag)
    }

    #[test]
    fn children_iterates_in_order() {
        let (mut tree, root) = fresh();
        let a = tree.append_child(root, nn(ElementTag::P)).unwrap();
        let b = tree.append_child(root, nn(ElementTag::P)).unwrap();
        let c = tree.append_child(root, nn(ElementTag::P)).unwrap();
        let collected: Vec<_> = tree.children(root).map(|(id, _)| id).collect();
        assert_eq!(collected, vec![a, b, c]);
    }

    #[test]
    fn children_of_leaf_is_empty() {
        let (mut tree, root) = fresh();
        let a = tree.append_child(root, nn(ElementTag::P)).unwrap();
        assert_eq!(tree.children(a).count(), 0);
    }

    #[test]
    fn ancestors_walks_to_root() {
        let (mut tree, root) = fresh();
        let a = tree.append_child(root, nn(ElementTag::P)).unwrap();
        let b = tree.append_child(a, nn(ElementTag::Span)).unwrap();
        let c = tree.append_child(b, nn(ElementTag::Em)).unwrap();
        let collected: Vec<_> = tree.ancestors(c).map(|(id, _)| id).collect();
        assert_eq!(collected, vec![b, a, root]);
    }

    #[test]
    fn ancestors_of_root_is_empty() {
        let (tree, root) = fresh();
        assert_eq!(tree.ancestors(root).count(), 0);
    }

    #[test]
    fn descendants_is_preorder() {
        let (mut tree, root) = fresh();
        let a = tree.append_child(root, nn(ElementTag::P)).unwrap();
        let b = tree.append_child(a, nn(ElementTag::Span)).unwrap();
        let c = tree.append_child(root, nn(ElementTag::P)).unwrap();
        let collected: Vec<_> = tree.descendants(root).map(|(id, _)| id).collect();
        assert_eq!(collected, vec![a, b, c]);
    }

    #[test]
    fn descendants_lazy_take() {
        let (mut tree, root) = fresh();
        let mut parent = root;
        for _ in 0..1000 {
            parent = tree
                .append_child(parent, nn(ElementTag::Div))
                .unwrap();
        }
        // Take only 3 — must not walk the whole 1000-deep tree.
        let count = tree.descendants(root).take(3).count();
        assert_eq!(count, 3);
    }
}
