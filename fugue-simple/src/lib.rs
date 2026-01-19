//! Fugue-Simple: A minimal port of the Fugue CRDT algorithm
//!
//! This is a direct port of the TypeScript implementation from:
//! https://github.com/bxff/fugue/blob/main/fugue-simple/src/index.ts
//!
//! The Fugue algorithm uses a tree-based CRDT for collaborative text editing.
//! Each character is a node in a tree, with ordering determined by parent/side
//! relationships and replica IDs for tie-breaking.

use std::collections::HashMap;

/// Unique identifier for a node in the Fugue tree
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ID {
    pub sender: String,
    pub counter: usize,
}

impl ID {
    pub fn new(sender: impl Into<String>, counter: usize) -> Self {
        Self {
            sender: sender.into(),
            counter,
        }
    }

    /// Root node ID (empty sender, counter 0)
    pub fn root() -> Self {
        Self {
            sender: String::new(),
            counter: 0,
        }
    }

    pub fn is_root(&self) -> bool {
        self.sender.is_empty() && self.counter == 0
    }
}

/// Side of parent to attach (Left or Right)
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    L,
    R,
}

/// A node in the Fugue tree
#[derive(Clone, Debug)]
pub struct Node<T> {
    pub id: ID,
    pub value: Option<T>,
    pub is_deleted: bool,
    pub parent: Option<ID>,
    pub side: Side,
    pub left_children: Vec<ID>,
    pub right_children: Vec<ID>,
    /// The non-deleted size of the subtree rooted at this node
    pub size: usize,
}

impl<T> Node<T> {
    fn new(id: ID, value: Option<T>, parent: Option<ID>, side: Side) -> Self {
        let is_deleted = parent.is_none();
        // Initial size is 0; update_size will add 1 to this node and all ancestors
        let size = 0;
        Self {
            id,
            value,
            is_deleted,
            parent,
            side,
            left_children: Vec::new(),
            right_children: Vec::new(),
            size,
        }
    }
}

/// The Fugue CRDT tree structure
pub struct Tree<T> {
    /// Map from sender to array of nodes by counter
    nodes_by_id: HashMap<String, Vec<Node<T>>>,
}

impl<T: Clone> Tree<T> {
    pub fn new() -> Self {
        let root = Node::new(ID::root(), None, None, Side::R);
        let mut nodes_by_id = HashMap::new();
        nodes_by_id.insert(String::new(), vec![root]);
        Self { nodes_by_id }
    }

    /// Get a node by ID
    pub fn get_by_id(&self, id: &ID) -> Option<&Node<T>> {
        self.nodes_by_id
            .get(&id.sender)
            .and_then(|nodes| nodes.get(id.counter))
    }

    /// Get a mutable node by ID
    pub fn get_by_id_mut(&mut self, id: &ID) -> Option<&mut Node<T>> {
        self.nodes_by_id
            .get_mut(&id.sender)
            .and_then(|nodes| nodes.get_mut(id.counter))
    }

    /// Get the root node
    pub fn root(&self) -> &Node<T> {
        self.get_by_id(&ID::root())
            .expect("Root should always exist")
    }

    /// Get the root node mutably
    pub fn root_mut(&mut self) -> &mut Node<T> {
        self.get_by_id_mut(&ID::root())
            .expect("Root should always exist")
    }

    /// Add a new node to the tree
    pub fn add_node(&mut self, id: ID, value: T, parent_id: &ID, side: Side) {
        let node = Node::new(id.clone(), Some(value), Some(parent_id.clone()), side);

        // Add to nodes_by_id
        let by_sender = self.nodes_by_id.entry(id.sender.clone()).or_default();
        // Ensure we have space up to this counter
        while by_sender.len() <= id.counter {
            // This shouldn't happen in normal operation, but handle it gracefully
            by_sender.push(Node::new(
                ID::new(id.sender.clone(), by_sender.len()),
                None,
                None,
                Side::R,
            ));
        }
        by_sender[id.counter] = node;

        // Insert into parent's siblings (sorted by sender for determinism)
        self.insert_into_siblings(&id);

        // Update sizes up the tree
        self.update_size(&id, 1);
    }

    /// Insert a node into its parent's children list, sorted by sender
    fn insert_into_siblings(&mut self, node_id: &ID) {
        let (parent_id, side, sender) = {
            let node = self.get_by_id(node_id).expect("Node should exist");
            (
                node.parent.clone().expect("Node should have parent"),
                node.side,
                node.id.sender.clone(),
            )
        };

        // First, collect sibling senders to determine position
        let siblings_senders: Vec<String> = {
            let parent = self.get_by_id(&parent_id).expect("Parent should exist");
            let siblings = match side {
                Side::L => &parent.left_children,
                Side::R => &parent.right_children,
            };
            siblings.iter().map(|s| s.sender.clone()).collect()
        };

        // Find insertion position (sorted by sender)
        let pos = siblings_senders
            .iter()
            .position(|s| sender <= *s)
            .unwrap_or(siblings_senders.len());

        // Now insert
        let parent = self.get_by_id_mut(&parent_id).expect("Parent should exist");
        let siblings = match side {
            Side::L => &mut parent.left_children,
            Side::R => &mut parent.right_children,
        };
        siblings.insert(pos, node_id.clone());
    }

    /// Update size of node and all ancestors
    pub fn update_size(&mut self, node_id: &ID, delta: isize) {
        let mut current = Some(node_id.clone());
        while let Some(id) = current {
            if let Some(node) = self.get_by_id_mut(&id) {
                node.size = (node.size as isize + delta) as usize;
                current = node.parent.clone();
            } else {
                break;
            }
        }
    }

    /// Get node at index within subtree rooted at given node
    pub fn get_by_index(&self, start: &ID, index: usize) -> Option<&Node<T>> {
        let root_node = self.get_by_id(start)?;
        if index >= root_node.size {
            return None;
        }

        self.get_by_index_recursive(start, index)
    }

    fn get_by_index_recursive(&self, node_id: &ID, mut index: usize) -> Option<&Node<T>> {
        let node = self.get_by_id(node_id)?;

        // Check left children first
        for child_id in &node.left_children {
            if let Some(child) = self.get_by_id(child_id) {
                if index < child.size {
                    return self.get_by_index_recursive(child_id, index);
                }
                index -= child.size;
            }
        }

        // Check this node
        if !node.is_deleted {
            if index == 0 {
                return Some(node);
            }
            index -= 1;
        }

        // Check right children
        for child_id in &node.right_children {
            if let Some(child) = self.get_by_id(child_id) {
                if index < child.size {
                    return self.get_by_index_recursive(child_id, index);
                }
                index -= child.size;
            }
        }

        None
    }

    /// Get the leftmost left-only descendant of a node
    pub fn leftmost_descendant(&self, start: &ID) -> ID {
        let mut current = start.clone();
        loop {
            let node = match self.get_by_id(&current) {
                Some(n) => n,
                None => break,
            };
            if node.left_children.is_empty() {
                break;
            }
            current = node.left_children[0].clone();
        }
        current
    }

    /// Traverse the tree in order, yielding non-deleted values
    pub fn traverse(&self, start: &ID) -> Vec<&T> {
        let mut result = Vec::new();
        self.traverse_recursive(start, &mut result);
        result
    }

    fn traverse_recursive<'a>(&'a self, node_id: &ID, result: &mut Vec<&'a T>) {
        let node = match self.get_by_id(node_id) {
            Some(n) => n,
            None => return,
        };

        // Only traverse if there's something to find
        if node.size == 0 {
            return;
        }

        // Traverse left children
        for child_id in &node.left_children {
            self.traverse_recursive(child_id, result);
        }

        // Visit this node
        if !node.is_deleted {
            if let Some(ref value) = node.value {
                result.push(value);
            }
        }

        // Traverse right children
        for child_id in &node.right_children {
            self.traverse_recursive(child_id, result);
        }
    }
}

impl<T: Clone> Default for Tree<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Insert message for syncing between replicas
#[derive(Clone, Debug)]
pub struct InsertMessage<T> {
    pub id: ID,
    pub value: T,
    pub parent: ID,
    pub side: Side,
}

/// Delete message for syncing between replicas
#[derive(Clone, Debug)]
pub struct DeleteMessage {
    pub id: ID,
}

/// Message types for syncing
#[derive(Clone, Debug)]
pub enum Message<T> {
    Insert(InsertMessage<T>),
    Delete(DeleteMessage),
}

/// The main Fugue-Simple document
pub struct FugueSimple<T> {
    replica_id: String,
    counter: usize,
    tree: Tree<T>,
    /// Pending messages waiting for their parent to be added
    pending: Vec<InsertMessage<T>>,
}

impl<T: Clone> FugueSimple<T> {
    /// Create a new Fugue document with the given replica ID
    pub fn new(replica_id: impl Into<String>) -> Self {
        Self {
            replica_id: replica_id.into(),
            counter: 0,
            tree: Tree::new(),
            pending: Vec::new(),
        }
    }

    /// Get the replica ID
    pub fn replica_id(&self) -> &str {
        &self.replica_id
    }

    /// Get the current document length
    pub fn len(&self) -> usize {
        self.tree.root().size
    }

    /// Check if document is empty
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Insert a value at the given index, returns message for syncing
    pub fn insert(&mut self, index: usize, value: T) -> Message<T> {
        let id = ID::new(&self.replica_id, self.counter);
        self.counter += 1;

        // Clamp index to valid range
        let index = index.min(self.len());

        // Find the leftOrigin: the node immediately before the insertion point
        // If index is 0, leftOrigin is the root
        // Otherwise, it's the node at index - 1
        let left_origin = if index == 0 {
            ID::root()
        } else {
            match self.tree.get_by_index(&ID::root(), index - 1) {
                Some(node) => node.id.clone(),
                None => {
                    // Index is out of bounds - clamp to end
                    if let Some(last) = self
                        .tree
                        .get_by_index(&ID::root(), self.len().saturating_sub(1))
                    {
                        last.id.clone()
                    } else {
                        // Empty document, insert at root
                        ID::root()
                    }
                }
            }
        };

        let left_origin_node = self.tree.get_by_id(&left_origin).unwrap();

        let (parent, side) = if left_origin_node.right_children.is_empty() {
            // leftOrigin has no right children, so the new node becomes
            // a right child of leftOrigin
            (left_origin, Side::R)
        } else {
            // Otherwise, the new node is added as a left child of rightOrigin,
            // which is the leftmost descendant of leftOrigin's first right child
            let first_right = left_origin_node.right_children[0].clone();
            let right_origin = self.tree.leftmost_descendant(&first_right);
            (right_origin, Side::L)
        };

        let msg = InsertMessage {
            id: id.clone(),
            value: value.clone(),
            parent: parent.clone(),
            side,
        };

        // Apply locally
        self.tree.add_node(id, value, &parent, side);

        Message::Insert(msg)
    }

    /// Delete the value at the given index, returns message for syncing
    pub fn delete(&mut self, index: usize) -> Message<T> {
        let len = self.len();
        if len == 0 {
            // Nothing to delete - return a no-op delete
            return Message::Delete(DeleteMessage {
                id: ID::new("__noop__", 0),
            });
        }

        let index = index.min(len - 1);
        let node = self.tree.get_by_index(&ID::root(), index);

        let id = match node {
            Some(n) => n.id.clone(),
            None => {
                // Index not found, try to delete last element
                match self.tree.get_by_index(&ID::root(), len.saturating_sub(1)) {
                    Some(n) => n.id.clone(),
                    None => {
                        return Message::Delete(DeleteMessage {
                            id: ID::new("__noop__", 0),
                        })
                    }
                }
            }
        };

        let msg = DeleteMessage { id: id.clone() };

        // Apply locally
        self.apply_delete(&id);

        Message::Delete(msg)
    }

    /// Apply a delete operation
    fn apply_delete(&mut self, id: &ID) {
        if let Some(node) = self.tree.get_by_id_mut(id) {
            if !node.is_deleted {
                node.is_deleted = true;
                node.value = None;
                self.tree.update_size(id, -1);
            }
        }
    }

    /// Apply a message from another replica
    pub fn apply(&mut self, msg: Message<T>) {
        match msg {
            Message::Insert(insert) => {
                self.apply_insert(insert);
            }
            Message::Delete(delete) => {
                self.apply_delete(&delete.id);
            }
        }
    }

    /// Apply an insert message, handling out-of-order delivery
    fn apply_insert(&mut self, insert: InsertMessage<T>) {
        // Check if we already have this node
        if self.tree.get_by_id(&insert.id).is_some() {
            // Check if it's a real node or a placeholder
            let existing = self.tree.get_by_id(&insert.id).unwrap();
            if existing.parent.is_some() {
                return; // Already applied
            }
        }

        // Check if already pending
        if self.pending.iter().any(|p| p.id == insert.id) {
            return; // Already queued
        }

        // Check if parent exists
        let parent_exists = insert.parent.is_root()
            || self
                .tree
                .get_by_id(&insert.parent)
                .map(|n| n.parent.is_some())
                .unwrap_or(false);

        if parent_exists {
            self.tree
                .add_node(insert.id.clone(), insert.value, &insert.parent, insert.side);

            // Try to apply any pending messages that were waiting for this node
            self.try_apply_pending();
        } else {
            // Parent doesn't exist yet, queue this message
            self.pending.push(insert);
        }
    }

    /// Try to apply any pending messages whose parents now exist
    fn try_apply_pending(&mut self) {
        // Keep trying until no more progress is made
        loop {
            let mut made_progress = false;
            let mut still_pending = Vec::new();

            for insert in std::mem::take(&mut self.pending) {
                // Check if we already have this node (fully applied)
                if let Some(existing) = self.tree.get_by_id(&insert.id) {
                    if existing.parent.is_some() {
                        // Already applied, skip
                        continue;
                    }
                }

                let parent_exists = insert.parent.is_root()
                    || self
                        .tree
                        .get_by_id(&insert.parent)
                        .map(|n| n.parent.is_some())
                        .unwrap_or(false);

                if parent_exists {
                    self.tree.add_node(
                        insert.id.clone(),
                        insert.value,
                        &insert.parent,
                        insert.side,
                    );
                    made_progress = true;
                } else {
                    still_pending.push(insert);
                }
            }

            self.pending = still_pending;

            if !made_progress {
                break;
            }
        }
    }

    /// Get all values in document order
    pub fn values(&self) -> Vec<&T> {
        self.tree.traverse(&ID::root())
    }

    /// Get value at index
    pub fn get(&self, index: usize) -> Option<&T> {
        self.tree
            .get_by_index(&ID::root(), index)
            .and_then(|n| n.value.as_ref())
    }
}

impl FugueSimple<char> {
    /// Convert the document to a string (for char type)
    pub fn to_string(&self) -> String {
        self.values().into_iter().collect()
    }

    /// Debug: get tree structure description
    pub fn tree_debug(&self) -> String {
        let mut result = String::new();
        self.tree_debug_node(&ID::root(), &mut result, 0);
        result
    }

    fn tree_debug_node(&self, id: &ID, result: &mut String, depth: usize) {
        if let Some(node) = self.tree.get_by_id(id) {
            let indent = "  ".repeat(depth);
            let value_str = if id.is_root() {
                "ROOT".to_string()
            } else {
                match &node.value {
                    Some(v) => format!("'{}'", v),
                    None => "(deleted)".to_string(),
                }
            };
            result.push_str(&format!(
                "{}[{},{}] {} (side={:?}, size={})\n",
                indent, id.sender, id.counter, value_str, node.side, node.size
            ));
            for child_id in &node.left_children {
                result.push_str(&format!("{}  L:\n", indent));
                self.tree_debug_node(child_id, result, depth + 2);
            }
            for child_id in &node.right_children {
                result.push_str(&format!("{}  R:\n", indent));
                self.tree_debug_node(child_id, result, depth + 2);
            }
        }
    }

    /// Debug: get node at index
    pub fn get_node_at(&self, index: usize) -> Option<String> {
        self.tree
            .get_by_index(&ID::root(), index)
            .map(|n| format!("({},{}) = {:?}", n.id.sender, n.id.counter, n.value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_at_end_of_document() {
        // This tests the exact sequence from the failing fuzz test
        let mut doc = FugueSimple::new("1");

        // Step 2: Insert Z at pos 0 (empty doc)
        doc.insert(0, 'Z');
        println!("After Z at 0: '{}'", doc.to_string());
        println!("  Tree: {:?}", doc.tree_debug());
        assert_eq!(doc.to_string(), "Z", "After inserting Z at 0");

        // Step 3: Insert J at pos 0 (before Z)
        doc.insert(0, 'J');
        println!("After J at 0: '{}'", doc.to_string());
        println!("  Tree: {:?}", doc.tree_debug());
        assert_eq!(doc.to_string(), "JZ", "After inserting J at 0");

        // Step 4: Insert K at pos 2 (end of document, len=2)
        // This should find the left_origin at index 1 (which is Z)
        // K should become a right child of Z
        println!("Document length before K: {}", doc.len());
        println!("  Getting node at index 1: {:?}", doc.get_node_at(1));
        doc.insert(2, 'K');
        println!("After K at 2: '{}'", doc.to_string());
        println!("  Tree: {:?}", doc.tree_debug());
        assert_eq!(doc.to_string(), "JZK", "After inserting K at 2 (end)");
    }

    #[test]
    fn test_insert_at_end_simple() {
        // Simplest case: insert at end
        let mut doc = FugueSimple::new("1");
        doc.insert(0, 'A');
        assert_eq!(doc.to_string(), "A");
        doc.insert(1, 'B');
        assert_eq!(doc.to_string(), "AB");
        doc.insert(2, 'C');
        assert_eq!(doc.to_string(), "ABC");
    }

    #[test]
    fn test_basic_insert() {
        let mut doc = FugueSimple::new("0");
        doc.insert(0, 'A');
        doc.insert(1, 'B');
        doc.insert(2, 'C');
        assert_eq!(doc.to_string(), "ABC");
    }

    #[test]
    fn test_insert_at_beginning() {
        let mut doc = FugueSimple::new("0");
        doc.insert(0, 'C');
        doc.insert(0, 'B');
        doc.insert(0, 'A');
        assert_eq!(doc.to_string(), "ABC");
    }

    #[test]
    fn test_insert_in_middle() {
        let mut doc = FugueSimple::new("0");
        doc.insert(0, 'A');
        doc.insert(1, 'C');
        doc.insert(1, 'B');
        assert_eq!(doc.to_string(), "ABC");
    }

    #[test]
    fn test_delete() {
        let mut doc = FugueSimple::new("0");
        doc.insert(0, 'A');
        doc.insert(1, 'B');
        doc.insert(2, 'C');
        doc.delete(1); // Delete B
        assert_eq!(doc.to_string(), "AC");
    }

    #[test]
    fn test_sync_two_replicas() {
        let mut doc1 = FugueSimple::new("0");
        let mut doc2 = FugueSimple::new("1");

        // Doc1 inserts A
        let msg1 = doc1.insert(0, 'A');
        doc2.apply(msg1);

        // Doc2 inserts B
        let msg2 = doc2.insert(1, 'B');
        doc1.apply(msg2);

        assert_eq!(doc1.to_string(), doc2.to_string());
        assert_eq!(doc1.to_string(), "AB");
    }

    #[test]
    fn test_concurrent_inserts() {
        let mut doc1 = FugueSimple::new("0");
        let mut doc2 = FugueSimple::new("1");

        // Both insert concurrently at position 0
        let msg1 = doc1.insert(0, 'A');
        let msg2 = doc2.insert(0, 'B');

        // Cross-apply
        doc1.apply(msg2);
        doc2.apply(msg1);

        // Should converge to same result
        assert_eq!(doc1.to_string(), doc2.to_string());
        // Order depends on replica ID comparison ("0" < "1" -> A before B)
        assert_eq!(doc1.to_string(), "AB");
    }

    #[test]
    fn test_figure7_scenario() {
        // Figure 7 from Fugue paper: Three concurrent inserts A, B, C
        // Then X inserted between A and C (by replica seeing A, C)
        // And Y inserted between A and B (by replica seeing A, B)
        // Expected final result: AXYBC

        let mut doc1 = FugueSimple::new("0"); // Will insert A
        let mut doc2 = FugueSimple::new("1"); // Will insert B
        let mut doc3 = FugueSimple::new("2"); // Will insert C

        // Concurrent inserts
        let msg_a = doc1.insert(0, 'A');
        let msg_b = doc2.insert(0, 'B');
        let msg_c = doc3.insert(0, 'C');

        // doc1 receives C, inserts X between A and C
        doc1.apply(msg_c.clone());
        // doc1 now has "AC" (A < C by replica ID)
        let msg_x = doc1.insert(1, 'X'); // Insert X at position 1 (between A and C)

        // doc2 receives A, inserts Y between A and B
        doc2.apply(msg_a.clone());
        // doc2 now has "AB" (A < B by replica ID)
        let msg_y = doc2.insert(1, 'Y'); // Insert Y at position 1 (between A and B)

        // Merge all messages into doc1
        doc1.apply(msg_b);
        doc1.apply(msg_y);

        // Merge all messages into doc2
        doc2.apply(msg_c);
        doc2.apply(msg_x);

        // Both should converge
        assert_eq!(doc1.to_string(), doc2.to_string());
        assert_eq!(doc1.to_string(), "AXYBC");
    }
}
