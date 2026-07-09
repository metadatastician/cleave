// SPDX-License-Identifier: MPL-2.0
// Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>

//! The linear handle (KERN-2, closing PROOF-NEEDS G-3 for the kernel path).
//!
//! A handle is a **use-exactly-once receipt** for an owned node:
//!
//! * consuming it goes through [`Surface::teardown`](crate::Surface::teardown),
//!   which takes it **by value** — a second teardown of the same handle is a
//!   move-checker error, not a runtime error (see the `compile_fail` doctest
//!   there);
//! * dropping it while its node is still live is a **leak**, and the
//!   drop-bomb panics in debug/test builds;
//! * dropping it after the *system* consumed the node (an ancestor's
//!   teardown, a lease expiry, a dial-up) is fine: the receipt is stale,
//!   the obligation was already discharged by the descent that collected it.
//!
//! This is the runtime mirror of `proofs/Cleave/Kernel/HandleLinearity.idr`;
//! true exactly-once linearity is the proof's job, the bomb is the trap for
//! the common case Rust can catch.

use crate::arena::NodeId;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

const ALIVE: u8 = 0;
const COLLECTED: u8 = 1;

/// Shared liveness cell between a node and its handle. The arena flips it
/// to `Collected` whenever the node is released (explicitly or by descent).
pub(crate) struct HandleState(AtomicU8);

impl HandleState {
    pub(crate) fn alive() -> HandleState {
        HandleState(AtomicU8::new(ALIVE))
    }

    pub(crate) fn collect(&self) {
        self.0.store(COLLECTED, Ordering::Release);
    }

    fn is_alive(&self) -> bool {
        self.0.load(Ordering::Acquire) == ALIVE
    }
}

/// A linear handle to an owned node. `#[must_use]`, neither `Clone` nor
/// `Copy`: mint → use → teardown to `⊥`, and nothing else can happen to it.
#[must_use = "a cleave Handle is linear: consume it with Surface::teardown (or let the owning descent collect it)"]
pub struct Handle {
    id: NodeId,
    state: Arc<HandleState>,
    armed: bool,
}

impl Handle {
    pub(crate) fn new(id: NodeId, state: Arc<HandleState>) -> Handle {
        Handle { id, state, armed: true }
    }

    pub(crate) fn id(&self) -> NodeId {
        self.id
    }

    /// The node this handle is a receipt for. An identifier only — it
    /// carries no authority; consuming still requires moving the handle
    /// itself into [`Surface::teardown`](crate::Surface::teardown).
    pub fn node_id(&self) -> NodeId {
        self.id
    }

    /// Defuse after an explicit consume (`Surface::teardown`).
    pub(crate) fn disarm(mut self) {
        self.armed = false;
        // Drops without bombing: consumed exactly once.
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        {
            if self.armed && self.state.is_alive() && !std::thread::panicking() {
                panic!(
                    "linear Handle for {:?} leaked: dropped while its node is live and unconsumed (G-3 / KERN-2)",
                    self.id
                );
            }
        }
        #[cfg(not(debug_assertions))]
        {
            let _ = self.armed;
        }
    }
}
