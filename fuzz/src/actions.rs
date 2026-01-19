//! Fuzz actions for testing Mako vs Fugue-Simple

use arbitrary::Arbitrary;

/// Actions that can be performed during fuzz testing
#[derive(Arbitrary, Clone, Debug)]
pub enum FuzzAction {
    /// Insert a character at a position in a replica's local view
    Insert {
        /// Which replica performs the insert (will be mod num_sites)
        site: u8,
        /// Position to insert at (will be clamped to document length)
        pos: u8,
        /// Character to insert (will be mapped to A-Z)
        char: u8,
    },

    /// Delete a character at a position in a replica's local view
    Delete {
        /// Which replica performs the delete (will be mod num_sites)
        site: u8,
        /// Position to delete (will be mod document length)
        pos: u8,
    },

    /// Sync updates from one replica to another
    Sync {
        /// Source replica (will be mod num_sites)
        from: u8,
        /// Destination replica (will be mod num_sites)
        to: u8,
    },

    /// Sync all replicas (everyone receives everyone's updates)
    SyncAll,
}

impl FuzzAction {
    /// Map a byte to a character A-Z
    pub fn byte_to_char(b: u8) -> char {
        (b'A' + (b % 26)) as char
    }
}
