//! Test harness for comparing Mako and Fugue-Simple implementations
//!
//! This module provides a test harness that applies the same operations
//! to both implementations and verifies they produce identical results.

use crate::actions::FuzzAction;
use fugue_simple::{FugueSimple, Message};
use mako::{oplist_to_string, Graph, Op, OpList};

/// A wrapper around Fugue-Simple for the fuzz harness
pub struct FugueReplica {
    pub id: String,
    pub doc: FugueSimple<char>,
    /// Pending messages to send to other replicas
    pub pending_messages: Vec<Message<char>>,
}

impl FugueReplica {
    pub fn new(id: String) -> Self {
        Self {
            doc: FugueSimple::new(&id),
            id,
            pending_messages: Vec::new(),
        }
    }

    pub fn insert(&mut self, pos: usize, c: char) {
        let msg = self.doc.insert(pos, c);
        self.pending_messages.push(msg);
    }

    pub fn delete(&mut self, pos: usize) {
        let msg = self.doc.delete(pos);
        self.pending_messages.push(msg);
    }

    pub fn len(&self) -> usize {
        self.doc.len()
    }

    pub fn to_string(&self) -> String {
        self.doc.to_string()
    }

    pub fn apply(&mut self, msg: Message<char>) {
        self.doc.apply(msg);
    }
}

/// A wrapper around Mako's Graph for the fuzz harness
///
/// This models a single replica that maintains a Graph of operations.
/// Each operation creates a new node in the graph.
pub struct MakoReplica {
    pub id: String,
    pub replica_num: usize,
    pub graph: Graph,
    pub next_local_counter: usize,
    /// Current frontier (leaf nodes) that new ops depend on
    pub frontier: Vec<usize>,
    /// Pending updates (node specs) to send to other replicas
    pub pending_updates: Vec<MakoUpdate>,
}

/// An update that can be sent to other replicas
#[derive(Clone, Debug)]
pub struct MakoUpdate {
    pub node_id: usize,
    pub op: OpList,
    pub parents: Vec<usize>,
}

impl MakoReplica {
    /// Node ID offset for each replica (to ensure unique IDs across replicas)
    const REPLICA_ID_MULTIPLIER: usize = 1_000_000;

    pub fn new(id: String) -> Self {
        let replica_num: usize = id.parse().unwrap_or(0);

        // Create graph with empty root node (root is always 0)
        let root_id = 0;
        let graph = Graph::new(
            root_id,
            OpList {
                ops: Vec::new(),
                test_op: None,
            },
        );

        Self {
            id,
            replica_num,
            graph,
            next_local_counter: 1, // Root is 0
            frontier: vec![0],     // Start at root
            pending_updates: Vec::new(),
        }
    }

    fn recompute_frontier(&mut self) {
        use std::collections::{HashSet, VecDeque};

        // Only include nodes whose full parent chain is reachable from the root. This prevents
        // buffering out-of-order updates (missing causal deps) from contaminating the parent set
        // for new local operations.
        let mut reachable: HashSet<usize> = HashSet::new();
        let mut queue: VecDeque<usize> = VecDeque::new();
        reachable.insert(self.graph.root);
        queue.push_back(self.graph.root);

        while let Some(node_id) = queue.pop_front() {
            let node = self.graph.nodes.get(&node_id).expect("Node not found");
            for &child_id in &node.children {
                if reachable.contains(&child_id) {
                    continue;
                }
                let child = self.graph.nodes.get(&child_id).expect("Child node not found");
                if child.parents.iter().all(|parent_id| reachable.contains(parent_id)) {
                    reachable.insert(child_id);
                    queue.push_back(child_id);
                }
            }
        }

        let mut frontier: Vec<usize> = Vec::new();
        for &node_id in &reachable {
            let node = self.graph.nodes.get(&node_id).expect("Node not found");
            let has_reachable_child =
                node.children.iter().any(|child_id| reachable.contains(child_id));
            if !has_reachable_child {
                frontier.push(node_id);
            }
        }
        frontier.sort_unstable();
        frontier.dedup();
        self.frontier = frontier;
    }

    /// Generate a globally unique node ID for this replica
    fn next_node_id(&mut self) -> usize {
        let local_counter = self.next_local_counter;
        self.next_local_counter += 1;
        // Offset by replica number to ensure uniqueness
        Self::REPLICA_ID_MULTIPLIER * (self.replica_num + 1) + local_counter
    }

    /// Get the current document as a string
    pub fn to_string(&self) -> String {
        let merged = self.graph.merge_graph();
        oplist_to_string(&merged)
    }

    /// Debug: get the merged OpList
    pub fn debug_merged_ops(&self) -> String {
        format!("{:?}", self.graph.merge_graph().ops)
    }

    /// Get the current document length
    pub fn len(&self) -> usize {
        self.to_string().len()
    }

    /// Debug: print graph structure
    pub fn debug_graph(&self) -> String {
        let mut result = String::new();
        result.push_str(&format!("Frontier: {:?}\n", self.frontier));
        result.push_str("Nodes:\n");
        for (id, node) in &self.graph.nodes {
            result.push_str(&format!(
                "  {} <- {:?} : {:?}\n",
                id, node.parents, node.op.ops
            ));
        }
        result
    }

    /// Insert a character at a position
    pub fn insert(&mut self, pos: usize, c: char) {
        let node_id = self.next_node_id();

        let op = OpList {
            ops: vec![Op::Insert {
                ins: pos as i32,
                content: c.to_string(),
            }],
            test_op: None,
        };

        let parents = self.frontier.clone();
        self.graph.add_node(node_id, op.clone(), parents.clone());
        self.frontier = vec![node_id];

        self.pending_updates.push(MakoUpdate {
            node_id,
            op,
            parents,
        });
    }

    /// Delete a character at a position
    pub fn delete(&mut self, pos: usize) {
        let node_id = self.next_node_id();

        // Delete is represented as negative length
        // Position is the end of the deletion range
        let op = OpList {
            ops: vec![Op::Delete {
                ins: (pos + 1) as i32, // Position after the character to delete
                len: -1,               // Delete 1 character backwards
            }],
            test_op: None,
        };

        let parents = self.frontier.clone();
        self.graph.add_node(node_id, op.clone(), parents.clone());
        self.frontier = vec![node_id];

        self.pending_updates.push(MakoUpdate {
            node_id,
            op,
            parents,
        });
    }

    /// Apply an update from another replica
    pub fn apply(&mut self, update: MakoUpdate) {
        // Check if we already have this node
        if self.graph.nodes.contains_key(&update.node_id) {
            return;
        }

        // Add the node (Graph handles missing parents via pending_children)
        self.graph
            .add_node(update.node_id, update.op, update.parents.clone());

        self.recompute_frontier();
    }
}

/// The main test harness for comparing Mako and Fugue-Simple
pub struct TestHarness {
    pub num_sites: u8,
    pub mako_replicas: Vec<MakoReplica>,
    pub fugue_replicas: Vec<FugueReplica>,
}

impl TestHarness {
    /// Create a new test harness with the given number of replicas
    pub fn new(num_sites: u8) -> Self {
        let mut mako_replicas = Vec::new();
        let mut fugue_replicas = Vec::new();

        for i in 0..num_sites {
            let id = i.to_string();
            mako_replicas.push(MakoReplica::new(id.clone()));
            fugue_replicas.push(FugueReplica::new(id));
        }

        Self {
            num_sites,
            mako_replicas,
            fugue_replicas,
        }
    }

    /// Apply a fuzz action to both implementations
    pub fn apply(&mut self, action: &FuzzAction) {
        match action {
            FuzzAction::Insert { site, pos, char } => {
                let site = (*site % self.num_sites) as usize;
                let c = FuzzAction::byte_to_char(*char);

                // Use minimum length to ensure both can handle the position
                let mako_len = self.mako_replicas[site].len();
                let fugue_len = self.fugue_replicas[site].len();
                let min_len = mako_len.min(fugue_len);

                // Use same position for both - clamp to minimum length
                let pos = (*pos as usize).min(min_len);

                self.mako_replicas[site].insert(pos, c);
                self.fugue_replicas[site].insert(pos, c);
            }
            FuzzAction::Delete { site, pos } => {
                let site = (*site % self.num_sites) as usize;

                // Use minimum length to ensure both can handle the position
                let mako_len = self.mako_replicas[site].len();
                let fugue_len = self.fugue_replicas[site].len();
                let min_len = mako_len.min(fugue_len);

                if min_len > 0 {
                    // Use same position for both
                    let pos = (*pos as usize) % min_len;

                    self.mako_replicas[site].delete(pos);
                    self.fugue_replicas[site].delete(pos);
                }
            }
            FuzzAction::Sync { from, to } => {
                let from = (*from % self.num_sites) as usize;
                let to = (*to % self.num_sites) as usize;

                if from != to {
                    self.sync_mako(from, to);
                    self.sync_fugue(from, to);
                }
            }
            FuzzAction::SyncAll => {
                self.sync_all_mako();
                self.sync_all_fugue();
            }
        }
    }

    /// Sync Mako updates from one replica to another
    fn sync_mako(&mut self, from: usize, to: usize) {
        let updates: Vec<MakoUpdate> = self.mako_replicas[from].pending_updates.clone();
        for update in updates {
            self.mako_replicas[to].apply(update);
        }
    }

    /// Sync Fugue messages from one replica to another
    fn sync_fugue(&mut self, from: usize, to: usize) {
        let messages: Vec<Message<char>> = self.fugue_replicas[from].pending_messages.clone();
        for msg in messages {
            self.fugue_replicas[to].apply(msg);
        }
    }

    /// Sync all Mako replicas
    fn sync_all_mako(&mut self) {
        // Collect all updates from all replicas
        let all_updates: Vec<(usize, Vec<MakoUpdate>)> = self
            .mako_replicas
            .iter()
            .enumerate()
            .map(|(i, r)| (i, r.pending_updates.clone()))
            .collect();

        // Apply all updates to all replicas
        for (source, updates) in all_updates {
            for (dest, replica) in self.mako_replicas.iter_mut().enumerate() {
                if source != dest {
                    for update in &updates {
                        replica.apply(update.clone());
                    }
                }
            }
        }
    }

    /// Sync all Fugue replicas
    fn sync_all_fugue(&mut self) {
        // Keep syncing until all replicas have received all messages
        // This is needed because messages may depend on other messages
        // that haven't been received yet

        let mut synced = true;
        while synced {
            synced = false;

            // Collect all messages from all replicas
            let all_messages: Vec<(usize, Vec<Message<char>>)> = self
                .fugue_replicas
                .iter()
                .enumerate()
                .map(|(i, r)| (i, r.pending_messages.clone()))
                .collect();

            // Apply all messages to all replicas
            for (source, messages) in all_messages {
                for (dest, replica) in self.fugue_replicas.iter_mut().enumerate() {
                    if source != dest {
                        for msg in &messages {
                            let old_len = replica.len();
                            replica.apply(msg.clone());
                            if replica.len() != old_len {
                                synced = true;
                            }
                        }
                    }
                }
            }
        }
    }

    /// Check if all replicas converged to the same state
    /// Returns Ok(()) if they match, Err with description if they don't
    pub fn check_equal(&self) -> Result<(), String> {
        // Check that all Mako replicas converged
        let mako_states: Vec<String> = self.mako_replicas.iter().map(|r| r.to_string()).collect();
        for i in 1..mako_states.len() {
            if mako_states[0] != mako_states[i] {
                return Err(format!(
                    "Mako replicas diverged!\n  Replica 0: '{}'\n  Replica {}: '{}'",
                    mako_states[0], i, mako_states[i]
                ));
            }
        }

        // Check that all Fugue replicas converged
        let fugue_states: Vec<String> = self.fugue_replicas.iter().map(|r| r.to_string()).collect();
        for i in 1..fugue_states.len() {
            if fugue_states[0] != fugue_states[i] {
                return Err(format!(
                    "Fugue replicas diverged!\n  Replica 0: '{}'\n  Replica {}: '{}'",
                    fugue_states[0], i, fugue_states[i]
                ));
            }
        }

        // Check that Mako and Fugue match
        if !mako_states.is_empty() && !fugue_states.is_empty() {
            if mako_states[0] != fugue_states[0] {
                return Err(format!(
                    "DIVERGENCE between Mako and Fugue!\n  Mako:  '{}'\n  Fugue: '{}'",
                    mako_states[0], fugue_states[0]
                ));
            }
        }

        Ok(())
    }

    /// Check that each implementation converges internally
    /// (all replicas of the same type agree)
    /// Does NOT check cross-implementation agreement
    pub fn check_internal_convergence(&self) -> Result<(), String> {
        // Check that all Mako replicas converged
        let mako_states: Vec<String> = self.mako_replicas.iter().map(|r| r.to_string()).collect();
        for i in 1..mako_states.len() {
            if mako_states[0] != mako_states[i] {
                return Err(format!(
                    "Mako replicas diverged!\n  Replica 0: '{}'\n  Replica {}: '{}'",
                    mako_states[0], i, mako_states[i]
                ));
            }
        }

        // Check that all Fugue replicas converged
        let fugue_states: Vec<String> = self.fugue_replicas.iter().map(|r| r.to_string()).collect();
        for i in 1..fugue_states.len() {
            if fugue_states[0] != fugue_states[i] {
                return Err(format!(
                    "Fugue replicas diverged!\n  Replica 0: '{}'\n  Replica {}: '{}'",
                    fugue_states[0], i, fugue_states[i]
                ));
            }
        }

        Ok(())
    }

    /// Check that both implementations have the same set of characters
    /// (regardless of order - useful for checking correctness of inserts/deletes)
    pub fn check_same_chars(&self) -> Result<(), String> {
        let mako_states: Vec<String> = self.mako_replicas.iter().map(|r| r.to_string()).collect();
        let fugue_states: Vec<String> = self.fugue_replicas.iter().map(|r| r.to_string()).collect();

        if !mako_states.is_empty() && !fugue_states.is_empty() {
            let mut mako_chars: Vec<char> = mako_states[0].chars().collect();
            let mut fugue_chars: Vec<char> = fugue_states[0].chars().collect();
            mako_chars.sort();
            fugue_chars.sort();

            if mako_chars != fugue_chars {
                return Err(format!(
                    "Different character sets!\n  Mako chars:  {:?}\n  Fugue chars: {:?}",
                    mako_chars, fugue_chars
                ));
            }
        }

        Ok(())
    }

    /// Get the current state of all replicas for debugging
    pub fn debug_state(&self) -> String {
        let mut result = String::new();
        result.push_str("=== Mako Replicas ===\n");
        for (i, r) in self.mako_replicas.iter().enumerate() {
            result.push_str(&format!("  Replica {}: '{}'\n", i, r.to_string()));
        }
        result.push_str("=== Fugue Replicas ===\n");
        for (i, r) in self.fugue_replicas.iter().enumerate() {
            result.push_str(&format!("  Replica {}: '{}'\n", i, r.to_string()));
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_harness_basic_insert() {
        let mut harness = TestHarness::new(2);

        // Site 0 inserts A
        harness.apply(&FuzzAction::Insert {
            site: 0,
            pos: 0,
            char: 0,
        });

        // Sync all
        harness.apply(&FuzzAction::SyncAll);

        // Both should have A
        harness.check_equal().expect("Should converge");
    }

    #[test]
    fn test_harness_concurrent_inserts() {
        let mut harness = TestHarness::new(2);

        // Site 0 inserts A, Site 1 inserts B (concurrently)
        harness.apply(&FuzzAction::Insert {
            site: 0,
            pos: 0,
            char: 0,
        }); // A
        harness.apply(&FuzzAction::Insert {
            site: 1,
            pos: 0,
            char: 1,
        }); // B

        // Sync all
        harness.apply(&FuzzAction::SyncAll);

        // Should converge (order may vary but should be consistent)
        harness.check_equal().expect("Should converge");
    }
}
