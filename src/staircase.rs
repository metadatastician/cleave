// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>

//! The two-stage staircase and the transmute-mode security layer
//! (TS-1..TS-7, promoted to the normative spec v0.3; KERN-7).
//!
//! The load-bearing rule is TS-1: the posture triple — (gating,
//! permissions, preemptability) — is a **function of the dial position**.
//! There is deliberately no way to construct or mutate a [`Posture`] except
//! through [`posture_of`]; softening the groove automatically tightens
//! gating and raises preemptability, hardening does the reverse, and the
//! two can never desynchronise (TS-5) because they are the same value.

/// A dial position on the kernel's two-stage staircase. `S2` is the deeper
/// (harder) stage; descending is grooving harder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    /// The shallow stage: soft reading of the dial.
    S1,
    /// The deep stage: hard reading of the dial.
    S2,
}

/// Transaction-gating floor at a dial position (TS-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gating {
    /// Every operation runs inside a checkpointed, abortable transaction.
    Transactional,
    /// Operations may run ungated (the hard/docked reading).
    Ungated,
}

/// Permission set at a dial position. Monotone in descent depth (TS-2):
/// dialing harder may only widen, softer may only narrow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PermSet {
    /// The session-scoped subset.
    Narrow,
    /// The full capability set.
    Wide,
}

impl PermSet {
    /// Subset order on permission sets: `Narrow ⊆ Wide` (TS-2's monotone
    /// map for a two-point lattice).
    pub fn subset_of(self, other: PermSet) -> bool {
        matches!((self, other), (PermSet::Narrow, _) | (PermSet::Wide, PermSet::Wide))
    }
}

/// Preemptability at a dial position (TS-4): preemptability is dual to
/// permanence. A soft surface is preemptable at once; a hard surface is
/// owed notice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preempt {
    /// The host may reclaim immediately (soft).
    Immediate,
    /// The host must honour a notice period of the given number of
    /// milliseconds before preemption (hard).
    WithNoticeMs(u64),
}

/// The posture triple (TS-1/TS-6): witnessed by value, derived only from
/// the dial. No field is settable independently of the stage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Posture {
    /// Transaction-gating floor (TS-3).
    pub gating: Gating,
    /// Granted permission set (TS-2).
    pub permissions: PermSet,
    /// Preemptability (TS-4).
    pub preemptability: Preempt,
}

/// The **only** constructor of [`Posture`] (TS-1): a total function of the
/// dial position. Exhaustive over [`Stage`], so totality is checked by the
/// compiler; the Idris2 mirror (`proofs/Cleave/Kernel/Posture.idr`) proves
/// monotonicity and the soft-preemptable/hard-noticed duality.
pub fn posture_of(stage: Stage) -> Posture {
    match stage {
        Stage::S1 => Posture {
            gating: Gating::Transactional,
            permissions: PermSet::Narrow,
            preemptability: Preempt::Immediate,
        },
        Stage::S2 => Posture {
            gating: Gating::Ungated,
            permissions: PermSet::Wide,
            preemptability: Preempt::WithNoticeMs(5_000),
        },
    }
}
