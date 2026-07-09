// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>

//! The occupancy lease (RC-6): a **renewable** well-founded measure that
//! bounds quiet infinite occupancy. `Soft` lets it expire — the expiry is a
//! zero-residue wipe (KERN-5). `Hard` refreshes it on heartbeat and only
//! degrades to the soft path after three whole missed TTL windows (KERN-6),
//! so a connection still being renewed is never reaped.

use crate::arena::NodeId;
use std::time::{Duration, Instant};

/// The two lease modes of the kernel's dial point. The soft/hard
/// distinction is the single most groove-visible behaviour of the whole
/// design (`docs/KERNEL.adoc`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lease {
    /// Session-scoped: expires at TTL absent refresh — and refresh is
    /// *refused*, because a soft lease MUST be allowed to expire (RC-6).
    Soft {
        /// Time to live.
        ttl: Duration,
    },
    /// Docked: refreshes on heartbeat; reaped only after three whole
    /// missed TTL windows, through the same zero-residue path.
    Hard {
        /// Time to live per heartbeat window.
        ttl: Duration,
    },
}

/// Lease-related recoverable errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaseError {
    /// Heartbeat on a soft lease: refused — soft MUST expire (RC-6).
    SoftMustExpire,
    /// The node carries no lease (the root).
    Unleased,
    /// The node is no longer live.
    Dead,
}

/// One expiry event returned by [`Surface::tick`](crate::Surface::tick).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Expiry {
    /// The node whose lease fired.
    pub node: NodeId,
    /// How many nodes the expiry wiped (the subtree, postorder).
    pub released: usize,
    /// Live nodes remaining in that subtree afterwards. Zero, always —
    /// asserted by the audit; carried so the evidence is explicit (KERN-5).
    pub residue: usize,
}

pub(crate) struct LeaseState {
    lease: Lease,
    expires_at: Instant,
}

impl LeaseState {
    pub(crate) fn new(lease: Lease, now: Instant) -> LeaseState {
        let ttl = match lease {
            Lease::Soft { ttl } | Lease::Hard { ttl } => ttl,
        };
        LeaseState { lease, expires_at: now + ttl }
    }

    /// Is this lease due for reaping at `now`?
    pub(crate) fn due(&self, now: Instant) -> bool {
        match self.lease {
            Lease::Soft { .. } => now >= self.expires_at,
            Lease::Hard { ttl } => {
                // Degrade only after three whole missed windows: the lease
                // expired, and two further full TTLs passed unrenewed.
                now >= self.expires_at + ttl + ttl
            }
        }
    }

    pub(crate) fn refresh_hard(&mut self, now: Instant) -> Result<(), LeaseError> {
        match self.lease {
            Lease::Soft { .. } => Err(LeaseError::SoftMustExpire),
            Lease::Hard { ttl } => {
                self.expires_at = now + ttl;
                Ok(())
            }
        }
    }
}
