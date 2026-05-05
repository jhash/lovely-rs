use crate::types::{ElementUuid, NodeId};

/// All error conditions [`crate::Tree`] operations can return.
#[derive(thiserror::Error, Debug)]
pub enum TreeError {
    #[error("node {0:?} not found")]
    NotFound(NodeId),

    #[error("uuid {0} not in tree")]
    UnknownUuid(ElementUuid),

    #[error("would create cycle: moving {child:?} into {ancestor:?}")]
    WouldCycle { child: NodeId, ancestor: NodeId },

    #[error("invalid attribute name: {0:?}")]
    InvalidAttribute(String),

    #[error("duplicate uuid: {0}")]
    DuplicateUuid(ElementUuid),

    #[error("cannot move or remove root node")]
    CannotMoveRoot,

    #[error("malformed db row: {0}")]
    MalformedRow(String),
}
