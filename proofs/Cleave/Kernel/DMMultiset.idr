-- SPDX-License-Identifier: MPL-2.0
-- Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
--
||| The Dershowitz–Manna step for the teardown descent (G-1 upgrade,
||| the tree-shaped part).
|||
||| RANKED-OWNERSHIP-CLEAVE §"Fidelity is to the partial order" grounds
||| tree-teardown termination in the DM multiset order: replacing a node by
||| the finite multiset of its children strictly decreases, because every
||| child's rank is strictly below the parent's (RC-2).
|||
||| This module proves the STEP lemma constructively:
||| `teardownStepDecreasesDM` — releasing a Ranked node is a DM step on the
||| rank multiset (children in, parent out, all children strictly below).
|||
||| Honesty note (per PROOF-NEEDS discipline): general well-foundedness of
||| the DM order over Nat-multisets has order type ω^ω and is NOT proven
||| here. For the kernel it is also not needed: the teardown of a FIXED
||| finite tree terminates by structural totality (ResourceCleanup.idr's
||| `plan` is total), which is exactly the DM argument specialised to a
||| tree fixed at connect time — the substitution RC-11 anticipates
||| (dynamic ranking over a finite tree needs only ℕ / finite multisets).
||| The general accessibility proof remains open and is tracked in
||| PROOF-NEEDS (G-1 residual).

module Cleave.Kernel.DMMultiset

import Data.Nat
import Cleave.Kernel.Types

%default total

||| Every element of the list is strictly below the bound.
public export
data AllLT : List Nat -> Nat -> Type where
  ALNil  : AllLT [] r
  ALCons : LT n r -> AllLT ns r -> AllLT (n :: ns) r

||| One restricted Dershowitz–Manna step on rank multisets (multisets as
||| lists; order of elements is irrelevant to the measure's use): the
||| multiset `news ++ rest` is DM-below `r :: rest` when every rank in
||| `news` is strictly below `r`. This is precisely the shape of one
||| teardown step: the parent (rank r) leaves the live multiset, its
||| children enter.
public export
data DMStep : List Nat -> List Nat -> Type where
  MkDMStep : (0 r : Nat) -> (0 news : List Nat) -> (0 rest : List Nat)
          -> AllLT news r
          -> DMStep (news ++ rest) (r :: rest)

||| From a Ranked node: all its children's ranks are strictly below its
||| own (RC-2 read off the witness).
public export
childrenBelowParent : {r : Rank} -> {cs : List RTree}
                   -> RankedForest r cs
                   -> AllLT (ranksOf cs) r
childrenBelowParent RFNil = ALNil
childrenBelowParent (RFCons lt _ rest) = ALCons lt (childrenBelowParent rest)

||| THE STEP LEMMA: releasing a Ranked node `RNode r m cs` — with any
||| other live ranks `rest` unaffected — is one DM step on the rank
||| multiset. Every clean-cleave step strictly decreases the remaining-
||| obligation measure (RC-7's discharge-descent, made formal for one step).
public export
teardownStepDecreasesDM : {r : Rank} -> {m : LeaseMode} -> {cs : List RTree}
                       -> Ranked (RNode r m cs)
                       -> (rest : List Nat)
                       -> DMStep (ranksOf cs ++ rest) (r :: rest)
teardownStepDecreasesDM (MkRanked forest) rest =
  MkDMStep r (ranksOf cs) rest (childrenBelowParent forest)
