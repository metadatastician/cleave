// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>

//! The ranked `owns` arena: a `Vec<Node>` tree in which **every owning edge
//! carries `rank(child) < rank(parent)`** (RC-1/RC-2), which makes ownership
//! cycles unrepresentable (RC-3): a cycle would need an infinite descending
//! chain in a well-founded order.

use crate::handle::HandleState;
use crate::lease::{Lease, LeaseError, LeaseState};
use crate::staircase::Stage;
use std::sync::Arc;
use std::time::Instant;

/// Root rank ceiling. Finite by design: RC-11 (ordinal economy) — a
/// dynamically-ranked finite tree needs only `ℕ`; `ω` and beyond MUST NOT
/// appear in the kernel.
pub(crate) const ROOT_RANK: Rank = Rank(64);

/// Index of a node in the arena. Slots are never reused within a surface's
/// lifetime, so a `NodeId` is stable for audit purposes even after release.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub(crate) usize);

/// A rank from a well-founded order (here: `ℕ`, RC-11). Owning edges are
/// only constructible rank-decreasing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Rank(pub u32);

/// Recoverable rank/ownership errors. The *unrecoverable* case — actually
/// constructing a rank-violating edge — panics instead (KERN-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RankError {
    /// The parent sits at rank 1: a child would need rank 0, and `⊥` is a
    /// floor, not a rank for live nodes. The staircase has a bottom (WF).
    Exhausted,
    /// The referenced node is no longer live.
    Dead,
    /// The root cannot be re-parented.
    Root,
}

pub(crate) struct Node {
    rank: Rank,
    parent: Option<NodeId>,
    children: Vec<NodeId>,
    stage: Stage,
    lease: Option<LeaseState>,
    live: bool,
    handle_state: Arc<HandleState>,
}

pub(crate) struct Arena {
    nodes: Vec<Node>,
}

impl Arena {
    pub(crate) fn new() -> Arena {
        Arena { nodes: Vec::new() }
    }

    pub(crate) fn root(&self) -> NodeId {
        NodeId(0)
    }

    pub(crate) fn mint_root(&mut self) -> (NodeId, Arc<HandleState>) {
        debug_assert!(self.nodes.is_empty());
        let state = Arc::new(HandleState::alive());
        self.nodes.push(Node {
            rank: ROOT_RANK,
            parent: None,
            children: Vec::new(),
            stage: Stage::S1,
            lease: None,
            live: true,
            handle_state: Arc::clone(&state),
        });
        (NodeId(0), state)
    }

    pub(crate) fn mint_child(
        &mut self,
        parent: NodeId,
        stage: Stage,
        lease: Lease,
        now: Instant,
    ) -> Result<(NodeId, Rank, Arc<HandleState>), RankError> {
        let parent_rank = {
            let p = self.node(parent);
            if !p.live {
                return Err(RankError::Dead);
            }
            p.rank
        };
        if parent_rank.0 <= 1 {
            return Err(RankError::Exhausted);
        }
        let rank = Rank(parent_rank.0 - 1);
        // RC-1/RC-2, asserted at every edge construction. With the checks
        // above this cannot fire; it is the executable invariant, not a
        // control path.
        assert!(
            rank < parent_rank,
            "RC-1 violated: owning edge must be rank-decreasing (child {rank:?} !< parent {parent_rank:?})"
        );
        let id = NodeId(self.nodes.len());
        let state = Arc::new(HandleState::alive());
        self.nodes.push(Node {
            rank,
            parent: Some(parent),
            children: Vec::new(),
            stage,
            lease: Some(LeaseState::new(lease, now)),
            live: true,
            handle_state: Arc::clone(&state),
        });
        self.node_mut(parent).children.push(id);
        Ok((id, rank, state))
    }

    /// Re-parent `child` under `new_parent`. Panics on a rank-violating
    /// edge — the RC-1 falsifier trigger (KERN-1): a violated assertion
    /// here is exactly "an owning edge has `rank(child) >= rank(parent)`".
    pub(crate) fn adopt(&mut self, new_parent: NodeId, child: NodeId) -> Result<(), RankError> {
        if !self.node(new_parent).live || !self.node(child).live {
            return Err(RankError::Dead);
        }
        let Some(old_parent) = self.node(child).parent else {
            return Err(RankError::Root);
        };
        let child_rank = self.node(child).rank;
        let parent_rank = self.node(new_parent).rank;
        assert!(
            child_rank < parent_rank,
            "RC-1 violated at adopt: rank(child) = {child_rank:?} must be strictly below rank(parent) = {parent_rank:?}; \
             ownership cycles are unrepresentable precisely because this assertion exists"
        );
        let old = self.node_mut(old_parent);
        old.children.retain(|&c| c != child);
        self.node_mut(new_parent).children.push(child);
        self.node_mut(child).parent = Some(new_parent);
        Ok(())
    }

    /// Mark `id` released, detach it from its parent's child list, move its
    /// handle receipt to `Collected`, and return `(rank, parent, stage)`
    /// for the audit. Children must already have been released — callers
    /// walk postorder (RC-8).
    pub(crate) fn release(&mut self, id: NodeId) -> (Rank, Option<NodeId>, Stage) {
        debug_assert!(
            self.node(id).children.iter().all(|&c| !self.node(c).live),
            "RC-8 violated: parent released while an owned child is live"
        );
        let (rank, parent, stage) = {
            let n = self.node_mut(id);
            n.live = false;
            n.lease = None;
            n.handle_state.collect();
            (n.rank, n.parent, n.stage)
        };
        if let Some(p) = parent {
            self.node_mut(p).children.retain(|&c| c != id);
        }
        (rank, parent, stage)
    }

    pub(crate) fn refresh_hard(&mut self, id: NodeId, now: Instant) -> Result<(), LeaseError> {
        let n = self.node_mut(id);
        if !n.live {
            return Err(LeaseError::Dead);
        }
        match &mut n.lease {
            Some(state) => state.refresh_hard(now),
            None => Err(LeaseError::Unleased),
        }
    }

    /// Nodes whose lease is due for action at `now`: soft past TTL, hard
    /// past three whole missed windows (RC-6).
    pub(crate) fn leases_due(&self, now: Instant) -> Vec<NodeId> {
        self.nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.live)
            .filter(|(_, n)| n.lease.as_ref().is_some_and(|l| l.due(now)))
            .map(|(i, _)| NodeId(i))
            .collect()
    }

    pub(crate) fn live_at_stage(&self, stage: Stage) -> Vec<NodeId> {
        self.nodes
            .iter()
            .enumerate()
            .filter(|(_, n)| n.live && n.stage == stage)
            .map(|(i, _)| NodeId(i))
            .collect()
    }

    pub(crate) fn is_live(&self, id: NodeId) -> bool {
        self.node(id).live
    }

    pub(crate) fn live_count(&self) -> usize {
        self.nodes.iter().filter(|n| n.live).count()
    }

    pub(crate) fn live_in_subtree(&self, id: NodeId) -> usize {
        let mut count = 0;
        let mut stack = vec![id];
        while let Some(n) = stack.pop() {
            if self.node(n).live {
                count += 1;
            }
            stack.extend(self.node(n).children.iter().copied());
        }
        count
    }

    pub(crate) fn children_of(&self, id: NodeId) -> &[NodeId] {
        &self.node(id).children
    }

    fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id.0]
    }

    fn node_mut(&mut self, id: NodeId) -> &mut Node {
        &mut self.nodes[id.0]
    }
}
