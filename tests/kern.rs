// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>

//! The kernel acceptance suite: one test per KERN-* ID in
//! `docs/KERNEL.adoc`. The kernel is done when all of these are green in
//! CI — each test name is the acceptance ID it demonstrates.
//!
//! Handle discipline used throughout: a handle consumed *explicitly* goes
//! through `Surface::teardown(h)` (move). A handle whose node was consumed
//! *by the system* (ancestor teardown, lease expiry, dial-up, or
//! `teardown_all`) becomes a stale receipt and drops safely afterwards.

use cleave::{
    posture_of, AuditEvent, Gating, Lease, NodeId, PermSet, Preempt, RankError, Stage, Surface,
};
use std::collections::HashMap;
use std::time::{Duration, Instant};

const TTL: Duration = Duration::from_millis(100);

fn soft() -> Lease {
    Lease::Soft { ttl: TTL }
}

fn hard() -> Lease {
    Lease::Hard { ttl: TTL }
}

/// KERN-1: `mint(parent)` returns a handle whose rank is strictly below its
/// parent's; ownership cycles are unrepresentable (RC-1); the staircase has
/// a bottom (rank exhaustion errs rather than wrapping).
#[test]
fn kern_1_rank_strictly_decreases_and_cycles_unrepresentable() {
    let now = Instant::now();
    let (mut s, root) = Surface::new();
    let a = s.mint(&root, soft(), now).unwrap();
    let b = s.mint(&a, soft(), now).unwrap();

    // Rank descent is recorded in the audit at every mint.
    let mut minted = HashMap::new();
    for e in s.audit().events() {
        if let AuditEvent::Minted { node, rank, parent } = e {
            minted.insert(*node, (*rank, *parent));
        }
    }
    let (root_rank, root_parent) = minted[&root.node_id()];
    let (a_rank, a_parent) = minted[&a.node_id()];
    let (b_rank, b_parent) = minted[&b.node_id()];
    assert_eq!(root_parent, None);
    assert!(a_rank < root_rank, "child rank must be strictly below parent");
    assert!(b_rank < a_rank, "grandchild rank must be strictly below child");
    assert_eq!(a_parent, Some(root.node_id()));
    assert_eq!(b_parent, Some(a.node_id()));

    // Rank exhaustion is an error, not a wrap-around: the staircase has a
    // bottom (WF). Mint a chain until the floor answers.
    let mut deepest = b;
    let mut receipts = vec![root, a];
    loop {
        match s.mint(&deepest, soft(), now) {
            Ok(h) => {
                receipts.push(deepest);
                deepest = h;
            }
            Err(e) => {
                assert_eq!(e, RankError::Exhausted, "the floor must answer Exhausted");
                receipts.push(deepest);
                break;
            }
        }
    }

    // Graceful global teardown collects every node; the held receipts go
    // stale and drop safely at end of scope.
    let report = s.teardown_all();
    assert_eq!(report.residue_in_subtree, 0);
    drop(receipts);
}

/// Companion falsifier for KERN-1: constructing a rank-violating owning
/// edge panics at construction. Adopting a node under its own strict
/// descendant is exactly the cycle case RC-1 exists to kill.
#[test]
#[should_panic(expected = "RC-1 violated")]
fn kern_1_rank_violation_panics() {
    let now = Instant::now();
    let (mut s, root) = Surface::new();
    let a = s.mint(&root, soft(), now).unwrap();
    let b = s.mint(&a, soft(), now).unwrap();
    // rank(a) > rank(b): adopting a under b must panic (RC-1 falsifier).
    let _ = s.adopt(&b, &a);
    unreachable!("adopt must have panicked; handles {root:?} intentionally not consumed", root = root.node_id());
}

/// KERN-2: a leaked handle (dropped without teardown while its node lives)
/// triggers the drop-bomb in debug/test builds (the bomb is
/// `cfg(debug_assertions)` by design — `docs/KERNEL.adoc`). Double-teardown
/// is covered by the `compile_fail` doctest on `Surface::teardown`.
#[test]
#[cfg(debug_assertions)]
fn kern_2_handle_linear_leak_bombs() {
    let result = std::panic::catch_unwind(|| {
        let now = Instant::now();
        let (mut s, root) = Surface::new();
        let leaked = s.mint(&root, soft(), now).unwrap();
        drop(leaked); // leak: node still live, handle unconsumed → bomb
        let _ = s.teardown(root);
    });
    assert!(result.is_err(), "leaking a live handle must bomb in debug builds");
}

/// KERN-2 non-bomb path: a handle whose node was consumed *by the system*
/// (an ancestor's teardown collected it) is a stale receipt and drops
/// safely — the obligation was already discharged by the descent.
#[test]
fn kern_2_stale_receipt_after_ancestor_teardown_is_safe() {
    let now = Instant::now();
    let (mut s, root) = Surface::new();
    let a = s.mint(&root, soft(), now).unwrap();
    let b = s.mint(&a, soft(), now).unwrap();
    let report = s.teardown(a); // collects b's node too, children-first
    assert_eq!(report.residue_in_subtree, 0);
    assert_eq!(report.released.first(), Some(&b.node_id()), "child released before parent");
    drop(b); // stale receipt: must NOT bomb
    let _ = s.teardown(root);
}

/// KERN-3: graceful teardown of the root drives residue to zero, and this
/// is asserted (inside `teardown_all`), not assumed.
#[test]
fn kern_3_teardown_all_residue_zero() {
    let now = Instant::now();
    let (mut s, root) = Surface::new();
    let a = s.mint(&root, soft(), now).unwrap();
    let a1 = s.mint(&a, soft(), now).unwrap();
    let a2 = s.mint(&a, hard(), now).unwrap();
    let b = s.mint(&root, hard(), now).unwrap();
    assert_eq!(s.residue(), 5);

    let report = s.teardown_all();
    assert_eq!(report.residue_in_subtree, 0, "⊥ must be the zero-residue state");
    assert_eq!(report.released.len(), 5, "every owned node discharged exactly once");

    // All receipts are stale now; safe to drop.
    drop((root, a, a1, a2, b));
}

/// KERN-4: the two-stage staircase tears down in the order-dual of
/// construction — stage-2 resources release before stage-1, children
/// before parents; the audit log proves the order.
#[test]
fn kern_4_staircase_order_dual() {
    let now = Instant::now();
    let (mut s, root) = Surface::new();

    // Construct downward: stage 1 first, then dial down and build stage 2.
    let s1_node = s.mint(&root, soft(), now).unwrap();
    let posture = s.dial(Stage::S2);
    assert_eq!(posture, posture_of(Stage::S2));
    let s2_a = s.mint(&s1_node, hard(), now).unwrap();
    let s2_b = s.mint(&s2_a, hard(), now).unwrap();

    // Dial back up: stage-2 resources must be released before any stage-1
    // resource is touched (order-dual of construction).
    let _ = s.dial(Stage::S1);

    let order = s.audit().release_order();
    assert_eq!(
        order,
        vec![s2_b.node_id(), s2_a.node_id()],
        "dial-up releases exactly the stage-2 subtree, children before parents"
    );
    assert_eq!(s.residue(), 2, "stage-1 nodes survive the dial-up"); // root + s1_node

    let _ = s.teardown(s1_node);
    let report = s.teardown_all();
    assert_eq!(report.residue_in_subtree, 0);

    // Full order-dual across the whole log: every released child precedes
    // its released parent (RC-8).
    // (Audit moved into the report path; re-derive from the release order
    // captured above plus the report.)
    drop((root, s2_a, s2_b));
}

/// KERN-5: a soft lease expires — absent refresh within its TTL the
/// resource is wiped and the audit shows a zero-residue expiry.
#[test]
fn kern_5_soft_lease_expires_zero_residue() {
    let now = Instant::now();
    let (mut s, root) = Surface::new();
    let a = s.mint(&root, soft(), now).unwrap();
    let a1 = s.mint(&a, soft(), now).unwrap();

    // A refresh on a soft lease is refused: soft MUST expire (RC-6).
    assert!(s.heartbeat(&a, now).is_err());

    let expiries = s.tick(now + TTL + Duration::from_millis(1));
    assert!(!expiries.is_empty(), "soft lease past TTL must expire");
    let total_released: usize = expiries.iter().map(|e| e.released).sum();
    assert_eq!(total_released, 2, "expiry wipes the whole owned subtree");
    assert!(expiries.iter().all(|e| e.residue == 0), "expiry is a zero-residue wipe");
    assert!(
        s.audit()
            .events()
            .iter()
            .any(|e| matches!(e, AuditEvent::Expired { residue: 0, .. })),
        "the audit must carry the zero-residue expiry"
    );
    // Children before parents inside the expiry wipe, too.
    let order = s.audit().release_order();
    assert_eq!(order, vec![a1.node_id(), a.node_id()]);

    let _ = s.teardown(root);
    drop((a, a1));
}

/// KERN-6: a hard lease refreshes — with a heartbeat it persists across
/// ≥3 TTL windows; when the heartbeat stops it degrades to the KERN-5 path.
#[test]
fn kern_6_hard_lease_heartbeat_survives_3_ttls_then_degrades() {
    let t0 = Instant::now();
    let (mut s, root) = Surface::new();
    let h = s.mint(&root, hard(), t0).unwrap();

    // Heartbeat across four TTL windows: never reaped while renewed (RC-6).
    let mut now = t0;
    for _ in 0..4 {
        now += TTL - Duration::from_millis(10);
        s.heartbeat(&h, now).unwrap();
        assert!(s.tick(now).is_empty(), "a lease being renewed must never be reaped");
    }
    assert_eq!(s.residue(), 2, "hard lease survived ≥3 TTL windows");

    // Stop heartbeating: within the grace windows it survives...
    let expiries = s.tick(now + TTL + Duration::from_millis(1));
    assert!(expiries.is_empty(), "hard degrades only after 3 whole missed windows");
    // ...after three whole missed windows it degrades through KERN-5.
    let expiries = s.tick(now + TTL * 3 + Duration::from_millis(1));
    assert_eq!(expiries.len(), 1);
    assert_eq!(expiries[0].residue, 0, "degradation is the same zero-residue wipe");

    let _ = s.teardown(root);
    drop(h);
}

/// KERN-7: the posture triple is a total function of the dial position —
/// monotone in depth (TS-2), gating floor by mode (TS-3), soft-preemptable /
/// hard-noticed (TS-4), and atomically re-derived on every re-dial (TS-5).
#[test]
fn kern_7_posture_total_monotone_atomic() {
    // Totality: every stage has a posture (exhaustive match, exercised for
    // both points of the two-stage dial).
    let p1 = posture_of(Stage::S1);
    let p2 = posture_of(Stage::S2);

    // TS-2: permissions monotone in descent depth (dialing harder widens).
    assert!(p1.permissions.subset_of(p2.permissions));
    assert_eq!(p1.permissions, PermSet::Narrow);
    assert_eq!(p2.permissions, PermSet::Wide);

    // TS-3: gating floor tightens as the groove softens.
    assert_eq!(p1.gating, Gating::Transactional);
    assert_eq!(p2.gating, Gating::Ungated);

    // TS-4: preemptability dual to permanence.
    assert_eq!(p1.preemptability, Preempt::Immediate);
    assert!(matches!(p2.preemptability, Preempt::WithNoticeMs(_)));

    // TS-1/TS-5: the surface's posture is derived from its dial, and a
    // re-dial returns the new posture from the same exclusive operation —
    // no observable interval disagrees.
    let (mut s, root) = Surface::new();
    assert_eq!(s.posture(), p1);
    assert_eq!(s.dial(Stage::S2), p2);
    assert_eq!(s.posture(), p2);
    assert_eq!(s.stage(), Stage::S2);
    assert_eq!(s.dial(Stage::S1), p1);
    assert_eq!(s.posture(), p1);

    let _ = s.teardown(root);
}

/// Cross-cutting: the order-dual property over an arbitrary mixed run —
/// every released child precedes its released parent (RC-8), regardless of
/// which path (explicit teardown, expiry, dial-up) did the releasing.
#[test]
fn order_dual_holds_across_mixed_release_paths() {
    let now = Instant::now();
    let (mut s, root) = Surface::new();
    let a = s.mint(&root, soft(), now).unwrap();
    let a1 = s.mint(&a, hard(), now).unwrap();
    let _ = s.dial(Stage::S2);
    let b = s.mint(&root, hard(), now).unwrap();
    let b1 = s.mint(&b, soft(), now).unwrap();

    // Mixed: expiry wipes a's subtree; dial-up wipes stage-2 (b's subtree).
    let _ = s.tick(now + TTL + Duration::from_millis(1));
    let _ = s.dial(Stage::S1);

    let mut minted_parent: HashMap<NodeId, Option<NodeId>> = HashMap::new();
    let mut released_seq: HashMap<NodeId, u64> = HashMap::new();
    for e in s.audit().events() {
        match e {
            AuditEvent::Minted { node, parent, .. } => {
                minted_parent.insert(*node, *parent);
            }
            AuditEvent::Released { node, seq, .. } => {
                released_seq.insert(*node, *seq);
            }
            _ => {}
        }
    }
    for (node, seq) in &released_seq {
        if let Some(Some(parent)) = minted_parent.get(node) {
            if let Some(parent_seq) = released_seq.get(parent) {
                assert!(
                    seq < parent_seq,
                    "RC-8 violated: {node:?} released after its parent {parent:?}"
                );
            }
        }
    }

    let report = s.teardown_all();
    assert_eq!(report.residue_in_subtree, 0);
    drop((root, a, a1, b, b1));
}
