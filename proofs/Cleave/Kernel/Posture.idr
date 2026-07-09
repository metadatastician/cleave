-- SPDX-License-Identifier: MPL-2.0
-- Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
--
||| Transmute-mode security (TS-1..TS-7, normative in
||| RANKED-OWNERSHIP-CLEAVE v0.3 §TS) — the kernel-sized proofs.
|||
||| The thesis: integration-degree IS security posture. The posture triple
||| (gating, permissions, preemptability) is a FUNCTION of dial position
||| (TS-1) — `postureOf` is total by construction and is the only
||| constructor of `Posture` values in the model, mirroring the Rust
||| kernel where `posture_of(stage)` is the only public constructor.
|||
||| Proven here for the two-stage dial:
|||   * `permsMonotone`  — TS-2: dialing deeper only widens permissions.
|||   * `gatingFloor`    — TS-3 (degenerate): each stage's declared floor.
|||   * `softPreemptable`/`hardNoticed` — TS-4: preemptability is dual to
|||     permanence.
||| TS-5 (atomic re-dial) is a property of the runtime borrow discipline
||| (KERN-7 exercises it); TS-7 (no wire leakage) is vacuous for the
||| in-process kernel and stated only in the spec.

module Cleave.Kernel.Posture

import Cleave.Kernel.Types

%default total

||| Transaction-gating floor (TS-3).
public export
data Gating = Transactional | Ungated

||| Permission set at a dial position (two-point lattice for the kernel).
public export
data PermSet = Narrow | Wide

||| Preemptability (TS-4).
public export
data Preempt = Immediate | WithNotice Nat

||| The posture triple (TS-1/TS-6): witnessed, not asserted.
public export
record Posture where
  constructor MkPosture
  gating  : Gating
  perms   : PermSet
  preempt : Preempt

||| TS-1: the posture triple is a total function of the dial position —
||| there is no other way to obtain one.
public export
postureOf : Stage -> Posture
postureOf S1 = MkPosture Transactional Narrow Immediate
postureOf S2 = MkPosture Ungated Wide (WithNotice 5000)

||| Depth order on the two-stage dial: S1 is at-most-as-deep-as S2.
public export
data Deeper : Stage -> Stage -> Type where
  DeeperRefl1 : Deeper S1 S1
  DeeperRefl2 : Deeper S2 S2
  Deeper12    : Deeper S1 S2

||| Subset order on the two-point permission lattice: Narrow ⊆ everything,
||| Wide ⊆ Wide.
public export
data SubsetP : PermSet -> PermSet -> Type where
  NarrowSub : SubsetP Narrow p
  WideWide  : SubsetP Wide Wide

||| TS-2: permissions are monotone in descent depth — dialing harder MAY
||| only widen, dialing softer MAY only narrow. Total case analysis over
||| the dial order.
public export
permsMonotone : Deeper s1 s2 -> SubsetP (perms (postureOf s1)) (perms (postureOf s2))
permsMonotone DeeperRefl1 = NarrowSub
permsMonotone DeeperRefl2 = WideWide
permsMonotone Deeper12    = NarrowSub

||| TS-3 (degenerate two-stage form): the gating floor each stage declares.
public export
gatingFloor : (gating (postureOf S1) = Transactional, gating (postureOf S2) = Ungated)
gatingFloor = (Refl, Refl)

||| TS-4, soft half: a Soft (lease-expiring) surface is preemptable at
||| once.
public export
softPreemptable : preempt (postureOf S1) = Immediate
softPreemptable = Refl

||| TS-4, hard half: a Hard (lease-refreshing) surface is owed notice —
||| there exists a declared notice period.
public export
hardNoticed : (n : Nat ** preempt (postureOf S2) = WithNotice n)
hardNoticed = (5000 ** Refl)
