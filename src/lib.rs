// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>

//! The cleave kernel — the minimal demonstrable core of the transmutable
//! interface engine, exactly as scoped by `docs/KERNEL.adoc`:
//!
//! * **one dial point** (the FFI-adjacent, in-process point),
//! * **a linear handle** (mint → use → teardown to `⊥`; nothing else),
//! * **a two-stage staircase** ([`Stage::S1`]/[`Stage::S2`]) so the
//!   order-dual of construction is observable,
//! * **a ranked `owns` arena** (every edge `rank(child) < rank(parent)`),
//! * **executable assertions** for RC-1 (rank descent), RC-6 (soft lease
//!   expires / hard lease refreshes) and RC-13 (graceful teardown drives
//!   residue to zero, degenerate single-process form), plus the promoted
//!   TS layer (posture is a function of dial position, KERN-7).
//!
//! Everything deeper — the full staircase, multi-process clean-cleave,
//! wire-level rupture — is deliberately out of scope here; see
//! `docs/PROOF-NEEDS.adoc` for the honest ledger.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod arena;
mod audit;
mod handle;
mod lease;
mod staircase;
mod teardown;

pub use arena::{NodeId, Rank, RankError};
pub use audit::{AuditEvent, AuditLog, AuditReport};
pub use handle::Handle;
pub use lease::{Expiry, Lease, LeaseError};
pub use staircase::{posture_of, Gating, PermSet, Posture, Preempt, Stage};

use arena::{Arena, ROOT_RANK};
use std::time::Instant;

/// One cleave surface: a ranked `owns` tree, a dial position, and an audit
/// log. The surface is itself linear in spirit: [`Surface::teardown_all`]
/// consumes it, and `⊥` (zero residue) is asserted, not assumed.
pub struct Surface {
    arena: Arena,
    stage: Stage,
    audit: AuditLog,
}

impl Surface {
    /// Create a surface and mint the root handle at rank
    /// [`ROOT_RANK`](arena::ROOT_RANK) (a finite ceiling: RC-11 — finite
    /// trees need only `ℕ`, never ordinals).
    pub fn new() -> (Surface, Handle) {
        let mut arena = Arena::new();
        let mut audit = AuditLog::new();
        let (id, state) = arena.mint_root();
        audit.minted(id, ROOT_RANK, None);
        let surface = Surface { arena, stage: Stage::S1, audit };
        (surface, Handle::new(id, state))
    }

    /// Mint a child of `parent` with the given lease, at the current stage.
    /// The child's rank is strictly below its parent's (RC-1/RC-2); rank
    /// exhaustion is an error rather than a wrap-around (`⊥` is a floor,
    /// not a modulus). KERN-1.
    pub fn mint(&mut self, parent: &Handle, lease: Lease, now: Instant) -> Result<Handle, RankError> {
        let (id, rank, state) = self.arena.mint_child(parent.id(), self.stage, lease, now)?;
        self.audit.minted(id, rank, Some(parent.id()));
        Ok(Handle::new(id, state))
    }

    /// Re-parent `child` under `new_parent`.
    ///
    /// The RC-1 assertion is live here: an adoption that would give an
    /// owning edge `rank(child) >= rank(parent)` **panics at construction**
    /// (`docs/KERNEL.adoc`, KERN-1 falsifier). Ownership cycles are thereby
    /// unrepresentable: a cycle needs `rank(a) < rank(b) < rank(a)`.
    /// Recoverable misuse (dead nodes, re-parenting the root) is an `Err`.
    pub fn adopt(&mut self, new_parent: &Handle, child: &Handle) -> Result<(), RankError> {
        self.arena.adopt(new_parent.id(), child.id())
    }

    /// Consume `handle` (exactly once — move semantics make a second
    /// teardown unrepresentable) and tear down its whole owned subtree,
    /// children before parent (RC-8, degenerate). KERN-2/KERN-4.
    ///
    /// ```compile_fail
    /// use cleave::{Lease, Surface};
    /// use std::time::{Duration, Instant};
    /// let (mut s, root) = Surface::new();
    /// let h = s.mint(&root, Lease::Soft { ttl: Duration::from_secs(1) }, Instant::now()).unwrap();
    /// let _ = s.teardown(h);
    /// let _ = s.teardown(h); // ERROR: use of moved value `h`
    /// # let _ = s.teardown_all();
    /// # let _ = root;
    /// ```
    pub fn teardown(&mut self, handle: Handle) -> AuditReport {
        let id = handle.id();
        let released = self.release_subtree(id);
        handle.disarm();
        AuditReport {
            released,
            residue_in_subtree: self.arena.live_in_subtree(id),
        }
    }

    /// Graceful teardown of everything, consuming the surface. Reaching `⊥`
    /// **is** the zero-residue state, and it is asserted, not assumed
    /// (RC-13 degenerate; KERN-3).
    pub fn teardown_all(mut self) -> AuditReport {
        let root = self.arena.root();
        let released = self.release_subtree(root);
        let residue = self.arena.live_count();
        assert!(
            residue == 0,
            "RC-13 violated: graceful teardown left {residue} live node(s) — residue must be zero at ⊥"
        );
        AuditReport { released, residue_in_subtree: residue }
    }

    /// Advance lease bookkeeping to `now` (RC-6). Soft leases past their
    /// TTL are wiped — a zero-residue expiry of the whole owned subtree
    /// (KERN-5). Hard leases are *renewable*: they persist while
    /// heartbeaten, and only after three whole missed TTL windows do they
    /// degrade through the same expiry path (KERN-6).
    pub fn tick(&mut self, now: Instant) -> Vec<Expiry> {
        let due = self.arena.leases_due(now);
        let mut expiries = Vec::with_capacity(due.len());
        for id in due {
            if !self.arena.is_live(id) {
                continue; // already wiped as part of an earlier subtree this tick
            }
            let released = self.release_subtree(id);
            let residue = self.arena.live_in_subtree(id);
            self.audit.expired(id, residue);
            expiries.push(Expiry { node: id, released: released.len(), residue });
        }
        expiries
    }

    /// Refresh a **hard** lease (RC-6: `HardGroove` MUST refresh on
    /// heartbeat; `SoftGroove` MUST be allowed to expire, so refreshing a
    /// soft lease is refused). KERN-6.
    pub fn heartbeat(&mut self, handle: &Handle, now: Instant) -> Result<(), LeaseError> {
        self.arena.refresh_hard(handle.id(), now)
    }

    /// Move the dial. Dialing **up** (S2 → S1, softening) releases every
    /// stage-2 resource before returning — the order-dual of construction,
    /// stage 2 before stage 1, children before parents (KERN-4). The
    /// posture triple is re-derived atomically from the new position
    /// (TS-1/TS-5): there is no observable interval in which the old
    /// posture answers for the new stage, because the dial and the posture
    /// change in the same exclusive borrow.
    pub fn dial(&mut self, to: Stage) -> Posture {
        if self.stage == Stage::S2 && to == Stage::S1 {
            let stage2 = self.arena.live_at_stage(Stage::S2);
            for id in stage2 {
                if self.arena.is_live(id) {
                    self.release_subtree(id);
                }
            }
        }
        self.stage = to;
        posture_of(self.stage)
    }

    /// The current dial position.
    pub fn stage(&self) -> Stage {
        self.stage
    }

    /// The posture triple for the current dial position. This — and
    /// [`dial`](Surface::dial) — are the only ways to observe a posture:
    /// posture is a function of the dial, never settable beside it (TS-1).
    pub fn posture(&self) -> Posture {
        posture_of(self.stage)
    }

    /// Number of live owned nodes (the running residue measure).
    pub fn residue(&self) -> usize {
        self.arena.live_count()
    }

    /// The append-only audit log (the evidence for KERN-3/4/5).
    pub fn audit(&self) -> &AuditLog {
        &self.audit
    }

    /// Release `id`'s subtree in postorder, auditing each release, and
    /// return the release order. Every released node's handle state moves
    /// to `Collected`, so a stale handle held by the caller no longer
    /// counts as a leak (it was consumed *by the system*).
    fn release_subtree(&mut self, id: NodeId) -> Vec<NodeId> {
        let plan = teardown::postorder_plan(&self.arena, id);
        for &node in &plan {
            let (rank, parent, stage) = self.arena.release(node);
            self.audit.released(node, rank, parent, stage);
        }
        debug_assert_eq!(
            self.arena.live_in_subtree(id),
            0,
            "RC-7 violated: subtree of {id:?} reached ⊥ with owned residue"
        );
        plan
    }
}

