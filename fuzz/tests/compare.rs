//! Comparison tests between Mako and Fugue-Simple using arbtest
//!
//! These tests verify that both implementations produce IDENTICAL results.
//! Fugue-Simple is the reference implementation - any divergence is a bug in Mako.

use arbtest::arbtest;
use mako_fuzz::{FuzzAction, TestHarness};

/// KNOWN BUG: Mako delete does not work after complex sync scenarios
///
/// This test documents a bug found by fuzz testing (seed: 0xd0374745000000bb).
/// After a series of inserts, deletes, and syncs across 3 replicas, Mako's
/// delete operation fails to take effect while Fugue's works correctly.
///
/// The issue appears to be in Mako's OT merge logic - the delete node is
/// added to the graph but when merge_graph() is called, the delete isn't
/// properly applied to the merged result.
///
/// Expected: Both Mako and Fugue produce 'O' after all operations
/// Actual: Mako produces 'OO', Fugue produces 'O'
#[test]
#[ignore] // Remove this when the bug is fixed
fn test_mako_delete_bug_after_complex_sync() {
    let mut harness = TestHarness::new(3);

    // Step 0: Site 1 inserts 'N' at pos 0
    harness.apply(&FuzzAction::Insert {
        site: 1,
        pos: 0,
        char: 143,
    }); // N

    // Step 1: Sync all - everyone has 'N'
    harness.apply(&FuzzAction::SyncAll);

    // Step 2: Site 1 inserts 'N' at pos 1 (end)
    harness.apply(&FuzzAction::Insert {
        site: 1,
        pos: 1,
        char: 195,
    }); // N (another N)

    // Step 3: Sync all - everyone has 'NN'
    harness.apply(&FuzzAction::SyncAll);

    // Step 4: Site 1 deletes at pos 1
    harness.apply(&FuzzAction::Delete { site: 1, pos: 1 });

    // Step 5: Site 2 inserts 'P' at pos 2 (end)
    harness.apply(&FuzzAction::Insert {
        site: 2,
        pos: 2,
        char: 223,
    }); // P

    // Step 6: Site 1 inserts 'Y' at pos 1
    harness.apply(&FuzzAction::Insert {
        site: 1,
        pos: 1,
        char: 24,
    }); // Y

    // Step 7: Sync all - should have 'NYP'
    harness.apply(&FuzzAction::SyncAll);

    // Step 8: Site 1 inserts 'B' at pos 3 (end)
    harness.apply(&FuzzAction::Insert {
        site: 1,
        pos: 3,
        char: 183,
    }); // B

    // Step 9: Site 2 deletes at pos 1 (Y)
    harness.apply(&FuzzAction::Delete { site: 2, pos: 1 });

    // Step 10: Site 0 deletes at pos 0 (N)
    harness.apply(&FuzzAction::Delete { site: 0, pos: 0 });

    // Step 11: Site 2 deletes at pos 1 (P)
    harness.apply(&FuzzAction::Delete { site: 2, pos: 1 });

    // Step 12: Sync all - should have 'B'
    harness.apply(&FuzzAction::SyncAll);

    // Verify intermediate state: all replicas should have 'B'
    assert_eq!(
        harness.mako_replicas[0].to_string(),
        "B",
        "Mako[0] should be 'B' before step 13"
    );
    assert_eq!(
        harness.fugue_replicas[0].to_string(),
        "B",
        "Fugue[0] should be 'B' before step 13"
    );

    println!("=== State before step 13 delete ===");
    println!("Mako[0] graph:\n{}", harness.mako_replicas[0].debug_graph());

    // Step 13: Site 0 deletes at pos 0 (B) - THIS IS WHERE MAKO FAILS
    harness.apply(&FuzzAction::Delete { site: 0, pos: 0 });

    println!("=== State after step 13 delete ===");
    println!("Mako[0] graph:\n{}", harness.mako_replicas[0].debug_graph());
    println!(
        "Mako[0] merged ops: {}",
        harness.mako_replicas[0].debug_merged_ops()
    );
    println!("Mako[0] result: '{}'", harness.mako_replicas[0].to_string());

    // BUG: Mako[0] stays 'B' but Fugue[0] becomes ''
    // This shows Mako's delete is not working
    assert_eq!(
        harness.fugue_replicas[0].to_string(),
        "",
        "Fugue[0] should be '' after delete"
    );
    // Uncomment this when bug is fixed:
    // assert_eq!(harness.mako_replicas[0].to_string(), "", "Mako[0] should be '' after delete");

    // Step 14: Site 0 deletes (skipped because Fugue is empty, min_len=0)
    harness.apply(&FuzzAction::Delete { site: 0, pos: 0 });

    // Step 15: Site 1 inserts 'O' at pos 0
    harness.apply(&FuzzAction::Insert {
        site: 1,
        pos: 0,
        char: 14,
    }); // O

    // Step 16: Site 0 deletes (skipped for Fugue, but Mako might try)
    harness.apply(&FuzzAction::Delete { site: 0, pos: 0 });

    // Final sync
    harness.apply(&FuzzAction::SyncAll);

    // Document the bug: Mako has 'OO', Fugue has 'O'
    let mako_result = harness.mako_replicas[0].to_string();
    let fugue_result = harness.fugue_replicas[0].to_string();

    println!("Mako result: '{}'", mako_result);
    println!("Fugue result: '{}'", fugue_result);

    // This assertion documents the expected behavior (currently fails)
    assert_eq!(
        mako_result, fugue_result,
        "BUG: Mako produces '{}' but Fugue (reference) produces '{}'",
        mako_result, fugue_result
    );
}

/// Minimal reproducer for the Mako delete bug
///
/// This is a simplified version that isolates the core issue:
/// After syncing operations from multiple replicas, Mako's delete
/// fails to take effect on the merged document.
#[test]
#[ignore] // Remove this when the bug is fixed
fn test_mako_delete_bug_minimal() {
    let mut harness = TestHarness::new(2);

    // Setup: Create content on site 0, sync to site 1
    harness.apply(&FuzzAction::Insert {
        site: 0,
        pos: 0,
        char: 0,
    }); // A
    harness.apply(&FuzzAction::SyncAll);

    // Both sites insert concurrently
    harness.apply(&FuzzAction::Insert {
        site: 0,
        pos: 1,
        char: 1,
    }); // B on site 0
    harness.apply(&FuzzAction::Insert {
        site: 1,
        pos: 1,
        char: 2,
    }); // C on site 1

    // Sync - both should have same content (order may vary)
    harness.apply(&FuzzAction::SyncAll);

    // Delete on site 0
    harness.apply(&FuzzAction::Delete { site: 0, pos: 0 });

    // Sync
    harness.apply(&FuzzAction::SyncAll);

    // Both should have same content after sync
    harness
        .check_equal()
        .expect("Mako should match Fugue after delete and sync");
}

/// Test that Mako and Fugue-Simple produce IDENTICAL results
/// for random sequences of operations.
#[test]
fn mako_matches_fugue_1s() {
    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        let mut harness = TestHarness::new(num_sites);

        for _ in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            harness.apply(&action);
        }

        harness.apply(&FuzzAction::SyncAll);

        // Both implementations must produce identical results
        if let Err(e) = harness.check_equal() {
            panic!(
                "Mako diverged from Fugue reference:\n{}\n\nState:\n{}",
                e,
                harness.debug_state()
            );
        }

        Ok(())
    })
    .budget_ms(1000);
}

/// Second 1-second run for more coverage
#[test]
fn mako_matches_fugue_1s_2() {
    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        let mut harness = TestHarness::new(num_sites);

        for _ in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            harness.apply(&action);
        }

        harness.apply(&FuzzAction::SyncAll);

        if let Err(e) = harness.check_equal() {
            panic!(
                "Mako diverged from Fugue reference:\n{}\n\nState:\n{}",
                e,
                harness.debug_state()
            );
        }

        Ok(())
    })
    .budget_ms(1000);
}

/// Test with only insert operations (simpler case)
#[test]
fn mako_matches_fugue_inserts_only() {
    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=3)?;
        let num_inserts: usize = u.int_in_range(2..=10)?;

        let mut harness = TestHarness::new(num_sites);

        for _ in 0..num_inserts {
            let site: u8 = u.arbitrary()?;
            let pos: u8 = u.arbitrary()?;
            let char: u8 = u.arbitrary()?;
            harness.apply(&FuzzAction::Insert { site, pos, char });
        }

        harness.apply(&FuzzAction::SyncAll);

        if let Err(e) = harness.check_equal() {
            panic!(
                "Mako diverged from Fugue reference:\n{}\n\nState:\n{}",
                e,
                harness.debug_state()
            );
        }

        Ok(())
    })
    .budget_ms(1000);
}

/// Test Figure 7 scenario explicitly
#[test]
fn test_figure7_through_harness() {
    let mut harness = TestHarness::new(3);

    // Three concurrent inserts at position 0
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
    harness.apply(&FuzzAction::Insert {
        site: 2,
        pos: 0,
        char: 2,
    }); // C

    // Sync some updates to create the partial parent scenario
    harness.apply(&FuzzAction::Sync { from: 2, to: 0 });
    harness.apply(&FuzzAction::Sync { from: 0, to: 1 });

    // Replica 0 inserts X at position 1
    harness.apply(&FuzzAction::Insert {
        site: 0,
        pos: 1,
        char: 23,
    }); // X
        // Replica 1 inserts Y at position 1
    harness.apply(&FuzzAction::Insert {
        site: 1,
        pos: 1,
        char: 24,
    }); // Y

    // Sync all
    harness.apply(&FuzzAction::SyncAll);

    // Both implementations must match
    harness
        .check_equal()
        .expect("Figure 7 scenario: Mako must match Fugue");
}

/// Test concurrent deletes
#[test]
fn test_concurrent_deletes() {
    let mut harness = TestHarness::new(2);

    // Insert ABC on both replicas
    harness.apply(&FuzzAction::Insert {
        site: 0,
        pos: 0,
        char: 0,
    }); // A
    harness.apply(&FuzzAction::Insert {
        site: 0,
        pos: 1,
        char: 1,
    }); // B
    harness.apply(&FuzzAction::Insert {
        site: 0,
        pos: 2,
        char: 2,
    }); // C
    harness.apply(&FuzzAction::SyncAll);

    // Both delete B concurrently
    harness.apply(&FuzzAction::Delete { site: 0, pos: 1 });
    harness.apply(&FuzzAction::Delete { site: 1, pos: 1 });

    // Sync
    harness.apply(&FuzzAction::SyncAll);

    // Both should produce "AC"
    harness
        .check_equal()
        .expect("Concurrent deletes: Mako must match Fugue");
}

/// Test insert and delete at same position
#[test]
fn test_insert_delete_same_position() {
    let mut harness = TestHarness::new(2);

    // Insert ABC
    harness.apply(&FuzzAction::Insert {
        site: 0,
        pos: 0,
        char: 0,
    }); // A
    harness.apply(&FuzzAction::Insert {
        site: 0,
        pos: 1,
        char: 1,
    }); // B
    harness.apply(&FuzzAction::Insert {
        site: 0,
        pos: 2,
        char: 2,
    }); // C
    harness.apply(&FuzzAction::SyncAll);

    // Replica 0 deletes B, Replica 1 inserts X at position 1
    harness.apply(&FuzzAction::Delete { site: 0, pos: 1 });
    harness.apply(&FuzzAction::Insert {
        site: 1,
        pos: 1,
        char: 23,
    }); // Insert X

    // Sync
    harness.apply(&FuzzAction::SyncAll);

    // Should have AXC - Mako must match Fugue
    harness
        .check_equal()
        .expect("Insert/delete at same position: Mako must match Fugue");
}

/// Debug test to reproduce the second failing seed
#[test]
#[ignore] // Use test_mako_delete_bug_after_complex_sync instead
fn debug_divergence_seed2() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        let mut harness = TestHarness::new(num_sites);

        println!("=== Starting test with {} sites, {} actions ===", num_sites, num_actions);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            
            // Debug lengths before delete
            match &action {
                FuzzAction::Delete { site, pos } => {
                    let actual_site = (*site % num_sites) as usize;
                    let mako_len = harness.mako_replicas[actual_site].len();
                    let fugue_len = harness.fugue_replicas[actual_site].len();
                    println!("Step {}: Delete on site {} - mako_len={}, fugue_len={}, min_len={}, pos_mod={}", 
                             i, actual_site, mako_len, fugue_len, mako_len.min(fugue_len),
                             if mako_len.min(fugue_len) > 0 { (*pos as usize) % mako_len.min(fugue_len) } else { 0 });
                }
                _ => {
                    println!("Step {}: {:?}", i, action);
                }
            }
            
            harness.apply(&action);

            // Show intermediate state
            println!("  After action:");
            for (j, r) in harness.mako_replicas.iter().enumerate() {
                println!("    Mako[{}]: '{}'", j, r.to_string());
            }
            for (j, r) in harness.fugue_replicas.iter().enumerate() {
                println!("    Fugue[{}]: '{}'", j, r.to_string());
            }
        }

        println!("\n=== SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("After SyncAll:");
        for (j, r) in harness.mako_replicas.iter().enumerate() {
            println!("  Mako[{}]: '{}'", j, r.to_string());
        }
        for (j, r) in harness.fugue_replicas.iter().enumerate() {
            println!("  Fugue[{}]: '{}'", j, r.to_string());
        }

        if let Err(e) = harness.check_equal() {
            panic!(
                "Mako diverged from Fugue reference:\n{}\n\nState:\n{}",
                e,
                harness.debug_state()
            );
        }

        Ok(())
    })
    .seed(0xd0374745000000bb)  // The second failing seed
    .run();
}

/// Debug test to reproduce the failing seed (insert-only, now passes with Fugue fix)
#[test]
#[ignore] // This was for debugging, now fixed
fn debug_divergence_seed() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=3)?;
        let num_inserts: usize = u.int_in_range(2..=10)?;

        let mut harness = TestHarness::new(num_sites);

        println!(
            "=== Starting test with {} sites, {} inserts ===",
            num_sites, num_inserts
        );

        for i in 0..num_inserts {
            let site: u8 = u.arbitrary()?;
            let pos: u8 = u.arbitrary()?;
            let char: u8 = u.arbitrary()?;
            let action = FuzzAction::Insert { site, pos, char };

            let site_actual = (site % num_sites) as usize;
            let mako_len = harness.mako_replicas[site_actual].len();
            let fugue_len = harness.fugue_replicas[site_actual].len();
            let pos_actual = (pos as usize).min(mako_len.min(fugue_len));
            let c = FuzzAction::byte_to_char(char);

            println!(
                "Step {}: Insert '{}' at pos {} on site {} (mako_len={}, fugue_len={})",
                i, c, pos_actual, site_actual, mako_len, fugue_len
            );

            harness.apply(&action);

            // Show intermediate state
            println!("  After insert:");
            for (j, r) in harness.mako_replicas.iter().enumerate() {
                println!("    Mako[{}]: '{}'", j, r.to_string());
            }
            for (j, r) in harness.fugue_replicas.iter().enumerate() {
                println!("    Fugue[{}]: '{}'", j, r.to_string());
            }
        }

        println!("\n=== SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("After SyncAll:");
        for (j, r) in harness.mako_replicas.iter().enumerate() {
            println!("  Mako[{}]: '{}'", j, r.to_string());
        }
        for (j, r) in harness.fugue_replicas.iter().enumerate() {
            println!("  Fugue[{}]: '{}'", j, r.to_string());
        }

        if let Err(e) = harness.check_equal() {
            panic!(
                "Mako diverged from Fugue reference:\n{}\n\nState:\n{}",
                e,
                harness.debug_state()
            );
        }

        Ok(())
    })
    .seed(0x4d965bb30000004d) // The failing seed
    .run();
}

/// Test delete at position 0
#[test]
fn test_delete_single_char() {
    let mut harness = TestHarness::new(1);

    // Insert B at position 0
    harness.apply(&FuzzAction::Insert {
        site: 0,
        pos: 0,
        char: 1,
    }); // B

    println!("After B at 0:");
    println!("  Mako[0]: '{}'", harness.mako_replicas[0].to_string());
    println!("  Fugue[0]: '{}'", harness.fugue_replicas[0].to_string());
    println!("  Mako graph:\n{}", harness.mako_replicas[0].debug_graph());

    // Delete at position 0
    harness.apply(&FuzzAction::Delete { site: 0, pos: 0 });

    println!("After delete at 0:");
    println!("  Mako[0]: '{}'", harness.mako_replicas[0].to_string());
    println!("  Fugue[0]: '{}'", harness.fugue_replicas[0].to_string());
    println!("  Mako graph:\n{}", harness.mako_replicas[0].debug_graph());

    // Should produce ""
    harness.check_equal().expect("Delete should match");
    assert_eq!(harness.mako_replicas[0].to_string(), "");
}

/// Test insert at position 0 with existing content
#[test]
fn test_insert_at_beginning_single_replica() {
    let mut harness = TestHarness::new(1);

    // Insert B at position 0
    harness.apply(&FuzzAction::Insert {
        site: 0,
        pos: 0,
        char: 1,
    }); // B

    println!("After B at 0:");
    println!("  Mako[0]: '{}'", harness.mako_replicas[0].to_string());
    println!("  Fugue[0]: '{}'", harness.fugue_replicas[0].to_string());

    // Insert O at position 0 (before B)
    harness.apply(&FuzzAction::Insert {
        site: 0,
        pos: 0,
        char: 14,
    }); // O

    println!("After O at 0:");
    println!("  Mako[0]: '{}'", harness.mako_replicas[0].to_string());
    println!("  Fugue[0]: '{}'", harness.fugue_replicas[0].to_string());

    // Should produce "OB"
    harness
        .check_equal()
        .expect("Insert at beginning should match");
    assert_eq!(harness.mako_replicas[0].to_string(), "OB");
}

/// Test sequential operations (no concurrency)
#[test]
fn test_sequential_operations() {
    let mut harness = TestHarness::new(2);

    harness.apply(&FuzzAction::Insert {
        site: 0,
        pos: 0,
        char: 0,
    }); // A
    harness.apply(&FuzzAction::SyncAll);

    harness.apply(&FuzzAction::Insert {
        site: 0,
        pos: 1,
        char: 1,
    }); // B
    harness.apply(&FuzzAction::SyncAll);

    harness.apply(&FuzzAction::Insert {
        site: 0,
        pos: 2,
        char: 2,
    }); // C
    harness.apply(&FuzzAction::SyncAll);

    harness.apply(&FuzzAction::Delete { site: 0, pos: 1 }); // Delete B
    harness.apply(&FuzzAction::SyncAll);

    // Should have "AC"
    harness
        .check_equal()
        .expect("Sequential operations: Mako must match Fugue");
}

/// Deterministic replay of failing seed 0xa79bb49300000060
/// Mako produces 'FV', Fugue produces 'VE'
///
/// Bug Analysis:
/// - Step 6: After sync, Mako[2] has 'FVE' but Fugue[2] has 'VEF' (ordering differs!)
/// - Step 7: Delete at position 1 removes different chars (E in Mako vs F in Fugue)
///
/// Root cause: Concurrent insert ordering differs between Mako and Fugue
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_a79bb493() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0xa79bb49300000060 ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            // Print state after each action
            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
        }

        Ok(())
    })
    .seed(0xa79bb49300000060)
    .run();
}

/// Deterministic replay of failing seed 0x5ef5926200000078
/// Mako produces 'II', Fugue produces 'HII'
///
/// Bug Analysis:
/// - Step 19 (final SyncAll): Mako incorrectly deletes 'H', Fugue preserves it
/// - The 'H' was inserted by site 3 at step 10, then multiple deletes and syncs happen
/// - Mako's delete operations are not correctly tracking position shifts
///
/// Root cause: Delete position not properly transformed against concurrent operations
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_5ef59262() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0x5ef5926200000078 ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            // Print state after each action
            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
        }

        Ok(())
    })
    .seed(0x5ef5926200000078)
    .run();
}

/// Deterministic replay of fuzz-found divergence (seed 0x2f5ae5de00000096).
///
/// Run:
/// `cargo test -p mako-fuzz --test compare replay_seed_2f5ae5de -- --ignored --nocapture`
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_2f5ae5de() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0x2f5ae5de00000096 ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
        }

        Ok(())
    })
    .seed(0x2f5ae5de00000096)
    .run();
}

/// Deterministic replay of fuzz-found divergence (seed 0x5b98d17000000078).
///
/// Run:
/// `cargo test -p mako-fuzz --test compare replay_seed_5b98d170 -- --ignored --nocapture`
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_5b98d170() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0x5b98d17000000078 ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
        }

        Ok(())
    })
    .seed(0x5b98d17000000078)
    .run();
}

/// Deterministic replay of fuzz-found divergence (seed 0x8fd60c990000003e).
///
/// Run:
/// `cargo test -p mako-fuzz --test compare replay_seed_8fd60c99 -- --ignored --nocapture`
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_8fd60c99() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0x8fd60c990000003e ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
        }

        Ok(())
    })
    .seed(0x8fd60c990000003e)
    .run();
}

/// Deterministic replay of fuzz-found divergence (seed 0x4bcfa4530000004d).
///
/// Run:
/// `cargo test -p mako-fuzz --test compare replay_seed_4bcfa453 -- --ignored --nocapture`
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_4bcfa453() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0x4bcfa4530000004d ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
        }

        Ok(())
    })
    .seed(0x4bcfa4530000004d)
    .run();
}

/// Deterministic replay of insert-only fuzz divergence (seed 0xae97dc9700000020).
///
/// Run:
/// `cargo test -p mako-fuzz --test compare replay_seed_ae97dc97 -- --ignored --nocapture`
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_ae97dc97() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=3)?;
        let num_inserts: usize = u.int_in_range(2..=10)?;

        println!("=== Seed 0xae97dc9700000020 ===");
        println!("num_sites: {}", num_sites);
        println!("num_inserts: {}", num_inserts);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_inserts {
            let site: u8 = u.arbitrary()?;
            let pos: u8 = u.arbitrary()?;
            let char: u8 = u.arbitrary()?;
            let action = FuzzAction::Insert { site, pos, char };

            println!("Insert {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
        }

        Ok(())
    })
    .seed(0xae97dc9700000020)
    .run();
}

/// Deterministic replay of fuzz-found divergence (seed 0x899e4f670000003e).
///
/// Run:
/// `cargo test -p mako-fuzz --test compare replay_seed_899e4f67 -- --ignored --nocapture`
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_899e4f67() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0x899e4f670000003e ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
        }

        Ok(())
    })
    .seed(0x899e4f670000003e)
    .run();
}

/// Deterministic replay of fuzz-found divergence (seed 0xee0b97e30000003e).
///
/// Run:
/// `cargo test -p mako-fuzz --test compare replay_seed_ee0b97e3 -- --ignored --nocapture`
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_ee0b97e3() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0xee0b97e30000003e ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
        }

        Ok(())
    })
    .seed(0xee0b97e30000003e)
    .run();
}

/// Deterministic replay of fuzz-found divergence (seed 0x6c2b3e5d00000028).
///
/// Run:
/// `cargo test -p mako-fuzz --test compare replay_seed_6c2b3e5d -- --ignored --nocapture`
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_6c2b3e5d() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0x6c2b3e5d00000028 ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
        }

        Ok(())
    })
    .seed(0x6c2b3e5d00000028)
    .run();
}

/// Deterministic replay of fuzz-found divergence (seed 0xfd77a57d00000078).
///
/// Run:
/// `cargo test -p mako-fuzz --test compare replay_seed_fd77a57d -- --ignored --nocapture`
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_fd77a57d() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0xfd77a57d00000078 ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
        }

        Ok(())
    })
    .seed(0xfd77a57d00000078)
    .run();
}

/// Deterministic replay of fuzz-found divergence (seed 0x5662092700000060).
///
/// Run:
/// `cargo test -p mako-fuzz --test compare replay_seed_56620927 -- --ignored --nocapture`
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_56620927() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0x5662092700000060 ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
        }

        Ok(())
    })
    .seed(0x5662092700000060)
    .run();
}

/// Deterministic replay of fuzz-found divergence (seed 0xc333fd5300000060).
///
/// Run:
/// `cargo test -p mako-fuzz --test compare replay_seed_c333fd53 -- --ignored --nocapture`
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_c333fd53() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0xc333fd5300000060 ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }

            if i == 12 || i == 13 || i == 14 {
                println!("\n--- Debug after step {} ---", i);
                for j in 0..num_sites as usize {
                    println!("Mako graph[{}]:\n{}", j, harness.mako_replicas[j].debug_graph());
                    println!(
                        "Mako merged ops[{}]: {}",
                        j,
                        harness.mako_replicas[j].debug_merged_ops()
                    );
                }
                println!("--- End debug ---\n");
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
        }

        Ok(())
    })
    .seed(0xc333fd5300000060)
    .run();
}

/// Deterministic replay of fuzz-found divergence (seed 0x79e8788900000060).
///
/// Run:
/// `cargo test -p mako-fuzz --test compare replay_seed_79e87889 -- --ignored --nocapture`
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_79e87889() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0x79e8788900000060 ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
            for j in 0..num_sites as usize {
                println!("Mako graph[{}]:\n{}", j, harness.mako_replicas[j].debug_graph());
                println!("Mako merged ops[{}]: {}", j, harness.mako_replicas[j].debug_merged_ops());
            }
        }

        Ok(())
    })
    .seed(0x79e8788900000060)
    .run();
}

/// Deterministic replay of fuzz-found divergence (seed 0xf13b71f200000078).
///
/// Run:
/// `cargo test -p mako-fuzz --test compare replay_seed_f13b71f2 -- --ignored --nocapture`
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_f13b71f2() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0xf13b71f200000078 ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
            for j in 0..num_sites as usize {
                println!("Mako graph[{}]:\n{}", j, harness.mako_replicas[j].debug_graph());
                println!("Mako merged ops[{}]: {}", j, harness.mako_replicas[j].debug_merged_ops());
            }
        }

        Ok(())
    })
    .seed(0xf13b71f200000078)
    .run();
}

/// Deterministic replay of fuzz-found divergence (seed 0x17322fae00000060).
///
/// Run:
/// `cargo test -p mako-fuzz --test compare replay_seed_17322fae -- --ignored --nocapture`
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_17322fae() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0x17322fae00000060 ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
            for j in 0..num_sites as usize {
                println!("Mako graph[{}]:\n{}", j, harness.mako_replicas[j].debug_graph());
                println!("Mako merged ops[{}]: {}", j, harness.mako_replicas[j].debug_merged_ops());
            }
        }

        Ok(())
    })
    .seed(0x17322fae00000060)
    .run();
}

/// Deterministic replay of fuzz-found divergence (seed 0x6d666d8e000000bb).
///
/// Run:
/// `cargo test -p mako-fuzz --test compare replay_seed_6d666d8e -- --ignored --nocapture`
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_6d666d8e() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0x6d666d8e000000bb ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
            for j in 0..num_sites as usize {
                println!("Mako graph[{}]:\n{}", j, harness.mako_replicas[j].debug_graph());
                println!("Mako merged ops[{}]: {}", j, harness.mako_replicas[j].debug_merged_ops());
            }
        }

        Ok(())
    })
    .seed(0x6d666d8e000000bb)
    .run();
}

/// Deterministic replay of current failing fuzz test (seed 0xa21227a600000373).
///
/// Run:
/// `cargo test -p mako-fuzz --test compare replay_seed_a21227a6 -- --ignored --nocapture`
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_a21227a6() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0xa21227a600000373 ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
            for j in 0..num_sites as usize {
                println!("Mako graph[{}]:\n{}", j, harness.mako_replicas[j].debug_graph());
                println!("Mako merged ops[{}]: {}", j, harness.mako_replicas[j].debug_merged_ops());
            }
        }

        Ok(())
    })
    .seed(0xa21227a600000373)
    .run();
}

/// Deterministic replay of current failing fuzz test (seed 0x0d628eb300010000).
///
/// Run:
/// `cargo test -p mako-fuzz --test compare replay_seed_0d628eb3 -- --ignored --nocapture`
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_0d628eb3() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0x0d628eb300010000 ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
            for j in 0..num_sites as usize {
                println!("Mako graph[{}]:\n{}", j, harness.mako_replicas[j].debug_graph());
                println!("Mako merged ops[{}]: {}", j, harness.mako_replicas[j].debug_merged_ops());
            }
        }

        Ok(())
    })
    .seed(0x0d628eb300010000)
    .run();
}

/// Deterministic replay of current failing fuzz test (seed 0xc2e4876900000078).
///
/// Run:
/// `cargo test -p mako-fuzz --test compare replay_seed_c2e48769 -- --ignored --nocapture`
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_c2e48769() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0xc2e4876900000078 ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
            for j in 0..num_sites as usize {
                println!("Mako graph[{}]:\n{}", j, harness.mako_replicas[j].debug_graph());
                println!("Mako merged ops[{}]: {}", j, harness.mako_replicas[j].debug_merged_ops());
            }
        }

        Ok(())
    })
    .seed(0xc2e4876900000078)
    .run();
}

/// Deterministic replay of current failing fuzz test (seed 0xd563101700000373).
///
/// Run:
/// `cargo test -p mako-fuzz --test compare replay_seed_d5631017 -- --ignored --nocapture`
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_d5631017() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0xd563101700000373 ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
            for j in 0..num_sites as usize {
                println!("Mako graph[{}]:\n{}", j, harness.mako_replicas[j].debug_graph());
                println!("Mako merged ops[{}]: {}", j, harness.mako_replicas[j].debug_merged_ops());
            }
        }

        Ok(())
    })
    .seed(0xd563101700000373)
    .run();
}

/// Deterministic replay of current failing fuzz test (seed 0x5dc28df200000028).
///
/// Run:
/// `cargo test -p mako-fuzz --test compare replay_seed_5dc28df2 -- --ignored --nocapture`
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_5dc28df2() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0x5dc28df200000028 ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
            for j in 0..num_sites as usize {
                println!("Mako graph[{}]:\n{}", j, harness.mako_replicas[j].debug_graph());
                println!("Mako merged ops[{}]: {}", j, harness.mako_replicas[j].debug_merged_ops());
            }
        }

        Ok(())
    })
    .seed(0x5dc28df200000028)
    .run();
}

/// Deterministic replay of current failing fuzz test (seed 0x30d21d4e00000060).
///
/// Run:
/// `cargo test -p mako-fuzz --test compare replay_seed_30d21d4e -- --ignored --nocapture`
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_30d21d4e() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0x30d21d4e00000060 ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
            for j in 0..num_sites as usize {
                println!("Mako graph[{}]:\n{}", j, harness.mako_replicas[j].debug_graph());
                println!("Mako merged ops[{}]: {}", j, harness.mako_replicas[j].debug_merged_ops());
            }
        }

        Ok(())
    })
    .seed(0x30d21d4e00000060)
    .run();
}

/// Deterministic replay of current failing fuzz test (seed 0x050f6c150000003e).
///
/// Run:
/// `cargo test -p mako-fuzz --test compare replay_seed_050f6c15 -- --ignored --nocapture`
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_050f6c15() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0x050f6c150000003e ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
            for j in 0..num_sites as usize {
                println!("Mako graph[{}]:\n{}", j, harness.mako_replicas[j].debug_graph());
                println!("Mako merged ops[{}]: {}", j, harness.mako_replicas[j].debug_merged_ops());
            }
        }

        Ok(())
    })
    .seed(0x050f6c150000003e)
    .run();
}

/// Deterministic replay of current failing fuzz test (seed 0xe94aabf9000002c3).
///
/// Run:
/// `cargo test -p mako-fuzz --test compare replay_seed_e94aabf9 -- --ignored --nocapture`
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_e94aabf9() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0xe94aabf9000002c3 ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
            for j in 0..num_sites as usize {
                println!("Mako graph[{}]:\n{}", j, harness.mako_replicas[j].debug_graph());
                println!("Mako merged ops[{}]: {}", j, harness.mako_replicas[j].debug_merged_ops());
            }
        }

        Ok(())
    })
    .seed(0xe94aabf9000002c3)
    .run();
}

/// Deterministic replay of current failing fuzz test (seed 0x5423ecfc00000060).
///
/// Run:
/// `cargo test -p mako-fuzz --test compare replay_seed_5423ecfc -- --ignored --nocapture`
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_5423ecfc() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0x5423ecfc00000060 ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
            for j in 0..num_sites as usize {
                println!("Mako graph[{}]:\n{}", j, harness.mako_replicas[j].debug_graph());
                println!("Mako merged ops[{}]: {}", j, harness.mako_replicas[j].debug_merged_ops());
            }
        }

        Ok(())
    })
    .seed(0x5423ecfc00000060)
    .run();
}

/// Deterministic replay of current failing fuzz test (seed 0xb53705a7000000e9).
///
/// Run:
/// `cargo test -p mako-fuzz --test compare replay_seed_b53705a7 -- --ignored --nocapture`
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_b53705a7() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0xb53705a7000000e9 ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
            for j in 0..num_sites as usize {
                println!("Mako graph[{}]:\n{}", j, harness.mako_replicas[j].debug_graph());
                println!("Mako merged ops[{}]: {}", j, harness.mako_replicas[j].debug_merged_ops());
            }
        }

        Ok(())
    })
    .seed(0xb53705a7000000e9)
    .run();
}

/// Deterministic replay of current failing fuzz test (seed 0x0038009800000078).
///
/// Run:
/// `cargo test -p mako-fuzz --test compare replay_seed_00380098 -- --ignored --nocapture`
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_00380098() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0x0038009800000078 ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
            for j in 0..num_sites as usize {
                println!("Mako graph[{}]:\n{}", j, harness.mako_replicas[j].debug_graph());
                println!("Mako merged ops[{}]: {}", j, harness.mako_replicas[j].debug_merged_ops());
            }
        }

        Ok(())
    })
    .seed(0x0038009800000078)
    .run();
}

/// Deterministic replay of current failing fuzz test (seed 0x43c95fbc00000562).
///
/// Run:
/// `cargo test -p mako-fuzz --test compare replay_seed_43c95fbc -- --ignored --nocapture`
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_43c95fbc() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0x43c95fbc00000562 ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
            for j in 0..num_sites as usize {
                println!("Mako graph[{}]:\n{}", j, harness.mako_replicas[j].debug_graph());
                println!("Mako merged ops[{}]: {}", j, harness.mako_replicas[j].debug_merged_ops());
            }
        }

        Ok(())
    })
    .seed(0x43c95fbc00000562)
    .run();
}

/// Deterministic replay of current failing fuzz test (seed 0x8df549a300010000).
///
/// Run:
/// `cargo test -p mako-fuzz --test compare replay_seed_8df549a3 -- --ignored --nocapture`
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_8df549a3() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0x8df549a300010000 ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
            for j in 0..num_sites as usize {
                println!("Mako graph[{}]:\n{}", j, harness.mako_replicas[j].debug_graph());
                println!("Mako merged ops[{}]: {}", j, harness.mako_replicas[j].debug_merged_ops());
            }
        }

        Ok(())
    })
    .seed(0x8df549a300010000)
    .run();
}

/// Deterministic replay of current failing fuzz test (seed 0x0e56061100000868).
///
/// Run:
/// `cargo test -p mako-fuzz --test compare replay_seed_0e560611 -- --ignored --nocapture`
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_0e560611() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0x0e56061100000868 ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
            for j in 0..num_sites as usize {
                println!("Mako graph[{}]:\n{}", j, harness.mako_replicas[j].debug_graph());
                println!("Mako merged ops[{}]: {}", j, harness.mako_replicas[j].debug_merged_ops());
            }
        }

        Ok(())
    })
    .seed(0x0e56061100000868)
    .run();
}

/// Deterministic replay of failing fuzz test (seed 0x45fbecdb00000078).
///
/// Run:
/// `cargo test -p mako-fuzz --test compare replay_seed_45fbecdb -- --ignored --nocapture`
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_45fbecdb() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0x45fbecdb00000078 ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
            for j in 0..num_sites as usize {
                println!("Mako graph[{}]:\n{}", j, harness.mako_replicas[j].debug_graph());
                println!("Mako merged ops[{}]: {}", j, harness.mako_replicas[j].debug_merged_ops());
            }
        }

        Ok(())
    })
    .seed(0x45fbecdb00000078)
    .run();
}

/// Deterministic replay of failing fuzz test (seed 0xb372f87f0000004d).
///
/// Run:
/// `cargo test -p mako-fuzz --test compare replay_seed_b372f87f -- --ignored --nocapture`
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_b372f87f() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0xb372f87f0000004d ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
            for j in 0..num_sites as usize {
                println!("Mako graph[{}]:\n{}", j, harness.mako_replicas[j].debug_graph());
                println!("Mako merged ops[{}]: {}", j, harness.mako_replicas[j].debug_merged_ops());
            }
        }

        Ok(())
    })
    .seed(0xb372f87f0000004d)
    .run();
}

/// Deterministic replay of failing fuzz test (seed 0xbc87aa1a000000bb).
///
/// Run:
/// `cargo test -p mako-fuzz --test compare replay_seed_bc87aa1a -- --ignored --nocapture`
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_bc87aa1a() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0xbc87aa1a000000bb ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
            for j in 0..num_sites as usize {
                println!("Mako graph[{}]:\n{}", j, harness.mako_replicas[j].debug_graph());
                println!("Mako merged ops[{}]: {}", j, harness.mako_replicas[j].debug_merged_ops());
            }
        }

        Ok(())
    })
    .seed(0xbc87aa1a000000bb)
    .run();
}

/// Deterministic replay of failing fuzz test (seed 0xd0ddc7f000000373).
///
/// Run:
/// `cargo test -p mako-fuzz --test compare replay_seed_d0ddc7f0 -- --ignored --nocapture`
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_d0ddc7f0() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0xd0ddc7f000000373 ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
            for j in 0..num_sites as usize {
                println!("Mako graph[{}]:\n{}", j, harness.mako_replicas[j].debug_graph());
                println!("Mako merged ops[{}]: {}", j, harness.mako_replicas[j].debug_merged_ops());
            }
        }

        Ok(())
    })
    .seed(0xd0ddc7f000000373)
    .run();
}

/// Deterministic replay of failing seed 0xe2106759000001c5
/// Mako produces 'VDLZ', Fugue produces 'VSDL'
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_e2106759() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0xe2106759000001c5 ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
        }

        Ok(())
    })
    .seed(0xe2106759000001c5)
    .run();
}

/// Deterministic replay of failing seed 0xf2c6c64a000061d0
/// Mako produces 'PWMDAR', Fugue produces 'RPWMSA'
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_f2c6c64a() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0xf2c6c64a000061d0 ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
        }

        Ok(())
    })
    .seed(0xf2c6c64a000061d0)
    .run();
}

/// Deterministic replay of failing seed 0xae51363200010000
/// Mako produces 'OJ', Fugue produces 'JO'
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_ae513632() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0xae51363200010000 ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
        }

        Ok(())
    })
    .seed(0xae51363200010000)
    .run();
}

/// Deterministic replay of failing seed 0x69a681c700000032
/// Mako produces 'AAAAAAMA', Fugue produces 'AAAAAAAM'
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_69a681c7() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0x69a681c700000032 ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
        }

        Ok(())
    })
    .seed(0x69a681c700000032)
    .run();
}

/// Deterministic replay of failing seed 0x5f00705d0000004d
/// Mako produces 'ARMKA', Fugue produces 'AARMK'
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_5f00705d() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0x5f00705d0000004d ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
        }

        Ok(())
    })
    .seed(0x5f00705d0000004d)
    .run();
}

/// Deterministic replay of failing seed 0x26261b0e00010000
/// Mako produces 'LZKN', Fugue produces 'KLZN'
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_26261b0e() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0x26261b0e00010000 ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
        }

        Ok(())
    })
    .seed(0x26261b0e00010000)
    .run();
}

/// Deterministic replay of failing seed 0x8b1a484900000028
/// Mako produces 'AAAAAAAAAAMA', Fugue produces 'AAAAAAAAAAAM'
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_8b1a4849() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0x8b1a484900000028 ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
        }

        Ok(())
    })
    .seed(0x8b1a484900000028)
    .run();
}

/// Deterministic replay of failing seed 0xef0ae3d5000002c3
/// Mako produces 'GNIR', Fugue produces 'NIRG'
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_ef0ae3d5() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0xef0ae3d5000002c3 ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
        }

        Ok(())
    })
    .seed(0xef0ae3d5000002c3)
    .run();
}

/// Deterministic replay of failing seed 0x8a0f798e00010000
/// Mako produces 'LJZN', Fugue produces 'TLZJ'
#[test]
#[ignore] // Enable to debug the specific seed
fn replay_seed_8a0f798e() {
    use arbtest::arbtest;

    arbtest(|u| {
        let num_sites: u8 = u.int_in_range(2..=4)?;
        let num_actions: usize = u.int_in_range(3..=20)?;

        println!("=== Seed 0x8a0f798e00010000 ===");
        println!("num_sites: {}", num_sites);
        println!("num_actions: {}", num_actions);

        let mut harness = TestHarness::new(num_sites);

        for i in 0..num_actions {
            let action: FuzzAction = u.arbitrary()?;
            println!("Step {}: {:?}", i, action);
            harness.apply(&action);

            for j in 0..num_sites as usize {
                println!(
                    "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                    j,
                    harness.mako_replicas[j].to_string(),
                    j,
                    harness.fugue_replicas[j].to_string()
                );
            }
        }

        println!("\n=== Final SyncAll ===");
        harness.apply(&FuzzAction::SyncAll);

        println!("=== Final State ===");
        for j in 0..num_sites as usize {
            println!(
                "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
                j,
                harness.mako_replicas[j].to_string(),
                j,
                harness.fugue_replicas[j].to_string()
            );
        }

        if let Err(e) = harness.check_equal() {
            println!("\nDIVERGENCE: {}", e);
        }

        Ok(())
    })
    .seed(0x8a0f798e00010000)
    .run();
}

/// BUG 1: Concurrent insert ordering differs from Fugue reference (seed 0xa79bb49300000060)
///
/// After a sequence of concurrent inserts and syncs, Mako produces 'FVE' while
/// Fugue produces 'VEF'. This causes subsequent deletes to target different characters.
///
/// Trace:
/// - Site 1 inserts 'V' at pos 0
/// - Sync 1->0, Site 0 inserts 'E' at pos 1
/// - Site 2 inserts 'F' at pos 0 (concurrent)
/// - Sync 0->2: Mako[2]='FVE', Fugue[2]='VEF' <-- DIVERGENCE
/// - Delete at pos 1: Mako deletes 'V', Fugue deletes 'E'
/// - Final: Mako='FV', Fugue='VE'
///
/// Run `cargo test -p mako-fuzz replay_seed_a79bb493 -- --ignored --nocapture` for details.

/// BUG 2: Delete incorrectly removes character after complex sync (seed 0x5ef5926200000078)
///
/// After 20 operations with 4 replicas, Mako produces 'II' while Fugue produces 'HII'.
/// The 'H' is incorrectly deleted in Mako.
///
/// Key operations:
/// - Multiple inserts and deletes across 4 replicas
/// - Several SyncAll operations
/// - At step 19, after final SyncAll, 'H' disappears in Mako
///
/// Run `cargo test -p mako-fuzz replay_seed_5ef59262 -- --ignored --nocapture` for details.

/// Additional failing seeds found during fuzz testing:
/// - 0xfac15edd00000060: Mako='UL', Fugue='UN'
/// - 0xd148f349000000bb: Mako='QNA', Fugue='QAY'  
/// - 0xf581efe00000003e: Mako='VI', Fugue='IV' (ordering bug)
/// - 0xd35a822300000096: Mako='ZO', Fugue='TO'
/// - 0x05156d8200000060: Mako='SS', Fugue='S' (failed delete)
/// - 0x57650b50000000bb: Mako='XMSX', Fugue='XISX'

/// BUG: Concurrent insert ordering differs from Fugue reference
///
/// Simplified from seed 0xa79bb49300000060:
/// - Site 1 inserts 'V' at pos 0
/// - Sync from site 1 to site 0
/// - Site 0 inserts 'E' at pos 1 (after V) -> 'VE'
/// - Site 2 inserts 'F' at pos 0
/// - Sync from site 0 to site 2 -> Mako gets 'FVE', Fugue gets 'VEF'
/// - Delete at position 1 -> Mako deletes 'V', Fugue deletes 'E'
///
/// Expected: Concurrent inserts at position 0 should have deterministic ordering
#[test]
fn test_concurrent_insert_ordering_bug() {
    let mut harness = TestHarness::new(3);

    // Site 1 inserts 'V' at position 0
    harness.apply(&FuzzAction::Insert {
        site: 1,
        pos: 0,
        char: 21,
    }); // V

    // Sync from site 1 to site 0
    harness.apply(&FuzzAction::Sync { from: 1, to: 0 });

    // Site 0 inserts 'E' at position 1 (after V)
    harness.apply(&FuzzAction::Insert {
        site: 0,
        pos: 1,
        char: 4,
    }); // E

    // Site 2 inserts 'F' at position 0 (concurrent with E)
    harness.apply(&FuzzAction::Insert {
        site: 2,
        pos: 0,
        char: 5,
    }); // F

    // Sync from site 0 to site 2 - this is where ordering differs
    harness.apply(&FuzzAction::Sync { from: 0, to: 2 });

    println!("After syncs:");
    println!("  Mako[2]: '{}'", harness.mako_replicas[2].to_string());
    println!("  Fugue[2]: '{}'", harness.fugue_replicas[2].to_string());

    // The order of 'F', 'V', 'E' should match between Mako and Fugue
    assert_eq!(
        harness.mako_replicas[2].to_string(),
        harness.fugue_replicas[2].to_string(),
        "Concurrent insert ordering should match Fugue reference"
    );
}

/// BUG: Delete incorrectly removes character after complex sync
///
/// Simplified from seed 0x5ef5926200000078:
/// After multiple concurrent deletes and syncs, Mako produces 'II'
/// while Fugue correctly produces 'HII' - the 'H' is incorrectly deleted.
///
/// This appears to be a case where the delete position is not properly
/// transformed against concurrent operations from other branches.
#[test]
fn test_delete_drops_concurrent_insert_bug() {
    let mut harness = TestHarness::new(4);

    // Site 3 inserts 'O'
    harness.apply(&FuzzAction::Insert {
        site: 3,
        pos: 0,
        char: 14,
    }); // O

    // Site 3 inserts 'X' at end
    harness.apply(&FuzzAction::Insert {
        site: 3,
        pos: 1,
        char: 23,
    }); // X

    // Sync all -> everyone has 'OX'
    harness.apply(&FuzzAction::SyncAll);

    // Site 2 inserts 'T' at end
    harness.apply(&FuzzAction::Insert {
        site: 2,
        pos: 2,
        char: 19,
    }); // T

    // Site 0 deletes at pos 1 (X) -> site 0 has 'O'
    harness.apply(&FuzzAction::Delete { site: 0, pos: 1 });

    // Site 3 inserts 'H' at end
    harness.apply(&FuzzAction::Insert {
        site: 3,
        pos: 2,
        char: 7,
    }); // H

    // Sync all -> should have 'OTH'
    harness.apply(&FuzzAction::SyncAll);

    println!("After first sync:");
    for i in 0..4 {
        println!(
            "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
            i,
            harness.mako_replicas[i].to_string(),
            i,
            harness.fugue_replicas[i].to_string()
        );
    }

    // Site 3 inserts 'I' at end
    harness.apply(&FuzzAction::Insert {
        site: 3,
        pos: 3,
        char: 8,
    }); // I

    // Site 1 deletes at pos 0 (O) -> site 1 has 'TH'
    harness.apply(&FuzzAction::Delete { site: 1, pos: 0 });

    // Site 2 inserts another 'I'
    harness.apply(&FuzzAction::Insert {
        site: 2,
        pos: 3,
        char: 8,
    }); // I

    // Site 1 deletes at pos 0 (T) -> site 1 has 'H'
    harness.apply(&FuzzAction::Delete { site: 1, pos: 0 });

    // Site 0 deletes at pos 0 (O) -> different view
    harness.apply(&FuzzAction::Delete { site: 0, pos: 0 });

    // Final sync - this is where the bug manifests
    harness.apply(&FuzzAction::SyncAll);

    println!("\nFinal state:");
    for i in 0..4 {
        println!(
            "  Mako[{}]: '{}' | Fugue[{}]: '{}'",
            i,
            harness.mako_replicas[i].to_string(),
            i,
            harness.fugue_replicas[i].to_string()
        );
    }

    // H should NOT be deleted - it was inserted by site 3 and never explicitly deleted
    if let Err(e) = harness.check_equal() {
        println!("\nDIVERGENCE: {}", e);
        println!("=== Mako[0] graph ===\n{}", harness.mako_replicas[0].debug_graph());
        println!(
            "=== Mako[0] merged ops ===\n{}",
            harness.mako_replicas[0].debug_merged_ops()
        );
        println!("=== Fugue[0] ===\n'{}'", harness.fugue_replicas[0].to_string());
        panic!("Mako should match Fugue reference - H should not be deleted");
    }
}
