// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>

//! The teardown planner: the discharge direction of the self-dual surface.
//!
//! Release respects the order-dual of `owns` — every owned child before its
//! parent (RC-8; LIFO is only the degenerate linear-stack case). For a
//! finite tree the walk terminates because each step replaces a node by the
//! finite multiset of its children, which strictly decreases in the
//! Dershowitz–Manna multiset order — proven in
//! `proofs/Cleave/Kernel/{DMMultiset,ResourceCleanup}.idr`; here the same
//! plan is computed by an explicit postorder.

use crate::arena::{Arena, NodeId};

/// Postorder plan over the live subtree rooted at `root`: children before
/// parents, so executing the plan front-to-back discharges every owned
/// child before the node that owns it (RC-7/RC-8).
pub(crate) fn postorder_plan(arena: &Arena, root: NodeId) -> Vec<NodeId> {
    let mut plan = Vec::new();
    if !arena.is_live(root) {
        return plan;
    }
    // Iterative postorder: push node, then children; reverse at the end of
    // a preorder-with-children-last gives postorder (children first).
    let mut stack = vec![root];
    let mut order = Vec::new();
    while let Some(n) = stack.pop() {
        if !arena.is_live(n) {
            continue;
        }
        order.push(n);
        stack.extend(arena.children_of(n).iter().copied());
    }
    // `order` is a preorder (parents before children); its reverse is a
    // valid postorder for release purposes (every child precedes its
    // parent).
    plan.extend(order.into_iter().rev());
    plan
}
