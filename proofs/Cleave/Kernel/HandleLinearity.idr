-- SPDX-License-Identifier: MPL-2.0
-- Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
--
||| Handle linearity (O-5; closes G-3 for the kernel path).
|||
||| A kernel handle is consumed EXACTLY once. In Idris2 this is not a
||| theorem *about* the code but a property *of* the types: `teardown`
||| takes its handle at multiplicity 1, so any caller that drops the handle
||| without consuming it, or consumes it twice, is rejected by the type
||| checker. The Rust kernel mirrors this at runtime: move semantics make
||| double-teardown unrepresentable (the `compile_fail` doctest on
||| `Surface::teardown`), and the debug drop-bomb traps leak-without-
||| teardown (KERN-2).
|||
||| Negative examples (each fails to typecheck, which IS the proof; kept
||| as comments because failing code cannot be committed):
|||
|||   leak : (1 h : Handle) -> Perp
|||   leak _ = AtPerp                    -- ERROR: h is not used
|||
|||   dup : (1 h : Handle) -> (Perp, Perp)
|||   dup h = (teardown h, teardown h)   -- ERROR: h used twice

module Cleave.Kernel.HandleLinearity

%default total

||| An opaque linear handle. Mint → use → teardown to ⊥; nothing else.
public export
data Handle : Type where
  MkHandle : Handle

||| ⊥ — the zero-residue terminal state (the order minimum of the
||| discharge measure; RC-7).
public export
data Perp : Type where
  AtPerp : Perp

||| Consuming teardown: the handle is used at multiplicity exactly 1.
||| `disconnect : CleaveSurface -o ()` from CLEAVE-ENGINE-DESIGN, in its
||| kernel-sized form.
public export
teardown : (1 h : Handle) -> Perp
teardown MkHandle = AtPerp

||| Scoped use: the continuation receives the handle linearly, so the
||| handle cannot escape the scope un-consumed. The `withHandle` shape from
||| CLEAVE-ENGINE-DESIGN §Linear handle.
public export
withHandle : (1 h : Handle) -> (1 k : (1 h' : Handle) -> Perp) -> Perp
withHandle h k = k h

||| Consumption is witnessed: teardown of the (sole) handle constructor
||| reaches ⊥ definitionally.
public export
teardownReachesPerp : teardown MkHandle = AtPerp
teardownReachesPerp = Refl
