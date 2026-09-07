// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Jonathan D.A. Jewell

//! Adversarial public API checks: authority belongs to an issuing surface,
//! and expiry is a deadline even if the application has not ticked yet.

use cleave::{Lease, LeaseError, RankError, Surface};
use std::time::{Duration, Instant};

#[test]
fn foreign_parent_cannot_mint_in_another_surface() {
    let (mut left, left_root) = Surface::new();
    let (right, right_root) = Surface::new();
    // A positive control for the alias: both arenas really use the same index.
    assert_eq!(left_root.node_id(), right_root.node_id());
    let result = left.mint(
        &right_root,
        Lease::Soft {
            ttl: Duration::from_secs(1),
        },
        Instant::now(),
    );
    let rejected = result.is_err();
    let residue = left.residue();
    left.teardown_all();
    right.teardown_all();
    drop((result, left_root, right_root));
    assert!(rejected, "a foreign receipt must not grant mint authority");
    assert_eq!(
        residue, 1,
        "rejection must not mutate the receiving surface"
    );
}

#[test]
fn foreign_teardown_cannot_release_another_surface() {
    let (mut left, left_root) = Surface::new();
    let (right, right_root) = Surface::new();
    let result =
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| left.teardown(right_root)));
    let residue = left.residue();
    left.teardown_all();
    right.teardown_all();
    drop(left_root);
    assert!(result.is_err(), "foreign teardown must reject the receipt");
    assert_eq!(residue, 1, "foreign teardown must not release local nodes");
}

#[test]
fn hard_lease_cannot_be_resurrected_before_a_delayed_tick() {
    let (mut surface, root) = Surface::new();
    let start = Instant::now();
    let ttl = Duration::from_millis(100);
    let child = surface.mint(&root, Lease::Hard { ttl }, start).unwrap();
    let deadline = start + ttl * 3;
    let heartbeat = surface.heartbeat(&child, deadline);
    let expiries = surface.tick(deadline);
    surface.teardown_all();
    drop((root, child));
    assert!(
        heartbeat.is_err(),
        "a heartbeat at the expiry deadline is too late"
    );
    assert_eq!(
        expiries.len(),
        1,
        "the delayed tick must still collect the lease"
    );
}

#[test]
fn foreign_adoption_and_heartbeat_are_rejected_without_mutation() {
    let now = Instant::now();
    let ttl = Duration::from_millis(100);
    let (mut left, left_root) = Surface::new();
    let (mut right, right_root) = Surface::new();
    let left_child = left.mint(&left_root, Lease::Hard { ttl }, now).unwrap();
    let right_child = right.mint(&right_root, Lease::Hard { ttl }, now).unwrap();
    assert_eq!(left_child.node_id(), right_child.node_id());
    assert_eq!(
        left.adopt(&right_root, &left_child),
        Err(RankError::ForeignSurface)
    );
    assert_eq!(
        left.adopt(&left_root, &right_child),
        Err(RankError::ForeignSurface)
    );
    assert_eq!(
        left.heartbeat(&right_child, now + ttl),
        Err(LeaseError::ForeignSurface)
    );
    assert_eq!(
        left.tick(now + ttl * 3).len(),
        1,
        "foreign heartbeat did not refresh local lease"
    );
    left.teardown_all();
    right.teardown_all();
    drop((left_root, right_root, left_child, right_child));
}

#[test]
fn recoverable_foreign_teardown_preserves_the_original_receipt() {
    let (mut left, left_root) = Surface::new();
    let (mut right, right_root) = Surface::new();
    let receipt = match left.try_teardown(right_root) {
        Err(receipt) => receipt,
        Ok(_) => panic!("foreign receipt was accepted"),
    };
    assert_eq!(left.residue(), 1);
    assert_eq!(right.teardown(receipt).released.len(), 1);
    left.teardown(left_root);
}

#[test]
fn foreign_out_of_bounds_receipt_is_an_error_and_surface_moves_keep_authority() {
    let now = Instant::now();
    let lease = Lease::Soft {
        ttl: Duration::from_secs(1),
    };
    let (mut left, left_root) = Surface::new();
    let (mut right, right_root) = Surface::new();
    let right_child = right.mint(&right_root, lease, now).unwrap();
    assert!(matches!(
        left.mint(&right_child, lease, now),
        Err(RankError::ForeignSurface)
    ));
    let mut moved = right;
    let descendant = moved.mint(&right_child, lease, now).unwrap();
    moved.teardown_all();
    left.teardown_all();
    drop((left_root, right_root, right_child, descendant));
}

#[test]
fn lease_arithmetic_handles_extremes_without_overflow_or_time_reversal() {
    let now = Instant::now();
    let (mut surface, root) = Surface::new();
    let huge = surface
        .mint(&root, Lease::Hard { ttl: Duration::MAX }, now)
        .unwrap();
    assert!(surface.tick(now + Duration::from_secs(1)).is_empty());
    surface
        .heartbeat(&huge, now + Duration::from_secs(1))
        .unwrap();
    assert_eq!(
        surface.heartbeat(&huge, now),
        Err(LeaseError::TimeWentBackwards)
    );
    let immediate = surface
        .mint(
            &root,
            Lease::Soft {
                ttl: Duration::ZERO,
            },
            now,
        )
        .unwrap();
    assert_eq!(surface.tick(now).len(), 1);
    surface.teardown_all();
    drop((root, huge, immediate));
}

#[test]
fn every_increasing_tree_on_seven_nodes_tears_down_once_in_postorder() {
    // 6! = 720 distinct labelled trees, with parent(i) in 0..i. This checks
    // node identity and order, not just the cardinality of a list of ranks.
    for encoding in 0..720 {
        let (mut surface, root) = Surface::new();
        let mut receipts = vec![root];
        let mut parents = vec![0];
        let mut remaining = encoding;
        for i in 1..7 {
            let parent = remaining % i;
            remaining /= i;
            let child = surface
                .mint(
                    &receipts[parent],
                    Lease::Soft {
                        ttl: Duration::from_secs(1),
                    },
                    Instant::now(),
                )
                .unwrap();
            receipts.push(child);
            parents.push(parent);
        }
        let report = surface.teardown_all();
        assert_eq!(report.residue_in_subtree, 0);
        let unique: std::collections::HashSet<_> = report.released.iter().copied().collect();
        assert_eq!(
            unique.len(),
            7,
            "every node appears once in tree {encoding}"
        );
        assert_eq!(report.released.len(), 7);
        for i in 1..7 {
            let child = report
                .released
                .iter()
                .position(|id| *id == receipts[i].node_id())
                .unwrap();
            let parent = report
                .released
                .iter()
                .position(|id| *id == receipts[parents[i]].node_id())
                .unwrap();
            assert!(child < parent, "child before parent in tree {encoding}");
        }
    }
}
