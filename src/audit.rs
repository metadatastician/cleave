// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>

//! The audit log: the kernel's evidence stream. KERN-3/4/5 are *observable*
//! behaviours — the log is what makes them assertable in tests rather than
//! believed.

use crate::arena::{NodeId, Rank};
use crate::staircase::Stage;

/// One audited event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditEvent {
    /// A node was minted (an owning edge constructed, rank-decreasing).
    Minted {
        /// The new node.
        node: NodeId,
        /// Its rank (strictly below its parent's).
        rank: Rank,
        /// Its owner, if any (`None` only for the root).
        parent: Option<NodeId>,
    },
    /// A node was released as one step of a teardown descent.
    Released {
        /// The released node.
        node: NodeId,
        /// Its rank at release.
        rank: Rank,
        /// Its owner at release time.
        parent: Option<NodeId>,
        /// The stage it was minted at.
        stage: Stage,
        /// Global release sequence number (the order-dual evidence).
        seq: u64,
    },
    /// A lease fired and wiped a subtree (KERN-5 / KERN-6 degradation).
    Expired {
        /// The node whose lease fired.
        node: NodeId,
        /// Live nodes remaining in its subtree after the wipe — always 0.
        residue: usize,
    },
}

/// Append-only audit log.
pub struct AuditLog {
    events: Vec<AuditEvent>,
    seq: u64,
}

impl AuditLog {
    pub(crate) fn new() -> AuditLog {
        AuditLog { events: Vec::new(), seq: 0 }
    }

    pub(crate) fn minted(&mut self, node: NodeId, rank: Rank, parent: Option<NodeId>) {
        self.events.push(AuditEvent::Minted { node, rank, parent });
    }

    pub(crate) fn released(&mut self, node: NodeId, rank: Rank, parent: Option<NodeId>, stage: Stage) {
        let seq = self.seq;
        self.seq += 1;
        self.events.push(AuditEvent::Released { node, rank, parent, stage, seq });
    }

    pub(crate) fn expired(&mut self, node: NodeId, residue: usize) {
        self.events.push(AuditEvent::Expired { node, residue });
    }

    /// All events, in order.
    pub fn events(&self) -> &[AuditEvent] {
        &self.events
    }

    /// The release order (node ids in the sequence they were discharged).
    pub fn release_order(&self) -> Vec<NodeId> {
        self.events
            .iter()
            .filter_map(|e| match e {
                AuditEvent::Released { node, .. } => Some(*node),
                _ => None,
            })
            .collect()
    }
}

/// Report returned by a teardown: what was released, and what remains in
/// the affected subtree (zero on every graceful path — asserted, not
/// assumed).
#[derive(Debug)]
pub struct AuditReport {
    /// Nodes released by this teardown, in discharge order (children
    /// before parents).
    pub released: Vec<NodeId>,
    /// Live nodes remaining in the torn-down subtree. Zero on the graceful
    /// path (RC-13 degenerate).
    pub residue_in_subtree: usize,
}
