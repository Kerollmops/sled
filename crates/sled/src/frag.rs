use super::*;

// NB correctness critical: never reorder or
// insert new variants or we will fail to load
// previously written databases.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) enum Frag {
    /// General-purpose structures
    // Metadata about the database and its collections
    Meta(Meta),

    // A monotonic ID generator for use in transactions etc...
    Counter(usize),

    /// Tree-related structures
    // The base tree node
    Base(Node),

    // Splits a tree node at a certain point
    // after the right side of the split was
    // already installed as a new `Base`
    ChildSplit(ChildSplit),

    // Tells an index node that a child split
    ParentSplit(ParentSplit),

    // Begins the merge of a child detected to
    // be too small
    InitialParentNodeMerge(PageId),

    // Marks a small node as ready to be merged
    // into the node to its immediate left
    RightNodeMerge,

    // Merges a small node directly to the
    // right into this node, adopting its
    // hi key and next pointer
    LeftNodeMerge(LeftMerge),

    // Clears the initial sign to merge a small
    // child
    FinalParentNodeMerge(PageId),

    // Insert a new key->version chain mapping
    InsertVersion(IVec, PageId),

    // Remove a key->version chain mapping
    RemoveVersion(IVec),

    // A multi-version value chain
    Versions(Versions),

    // Commits a pending version transaction
    VersionCommit(u64),

    // Insert a new value into a version chain
    // as part of a pending transaction
    VersionPendingSet(u64, IVec),

    // Merge a new value into a version chain
    // as part of a pending transaction
    VersionPendingMerge(u64, IVec),

    // Delete a value from a version chain
    // as part of a pending transaction
    VersionPendingDel(u64),
}

impl Frag {
    pub(super) fn unwrap_base(&self) -> &Node {
        if let Frag::Base(base, ..) = self {
            base
        } else {
            panic!("called unwrap_base on non-Base Frag!")
        }
    }

    pub(super) fn unwrap_versions(&self) -> &Versions {
        if let Frag::Versions(versions) = self {
            versions
        } else {
            panic!("called unwrap_versions on non-Base Frag!")
        }
    }

    pub(super) fn unwrap_meta(&self) -> &Meta {
        if let Frag::Meta(meta) = self {
            meta
        } else {
            panic!("called unwrap_meta on non-Base Frag!")
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct ParentSplit {
    pub(crate) at: IVec,
    pub(crate) to: PageId,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct ChildSplit {
    pub(crate) at: IVec,
    pub(crate) to: PageId,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct LeftMerge {
    pub(crate) new_hi: IVec,
    pub(crate) new_next: Option<PageId>,
    pub(crate) merged_items: Vec<(IVec, IVec)>,
}
