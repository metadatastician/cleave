-- SPDX-License-Identifier: MPL-2.0
-- Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
--
||| Inductive mirrors of the cleave kernel's actual types (src/arena.rs,
||| src/lease.rs, src/staircase.rs). Rebuilt fresh against the kernel per
||| PROOF-NEEDS G-6 — deliberately NOT lifted from gossamer's phantom
||| Gossamer.ABI.* surface.
|||
||| The load-bearing definition is `Ranked`: an owns-tree in which every
||| owning edge carries `rank(child) < rank(parent)` (RC-1/RC-2). The tree
||| is finite by construction, and a rank-decreasing finite tree cannot
||| contain an ownership cycle: a cycle would need an infinite strictly
||| descending chain of `Nat` ranks (RC-3). Per RC-11 (ordinal economy)
||| ranks are plain `Nat` — finite trees need no ordinals.

module Cleave.Kernel.Types

import Data.Nat

%default total

||| The two-stage dial of the kernel (src/staircase.rs). S2 is deeper.
public export
data Stage = S1 | S2

||| Lease modes (src/lease.rs): Soft expires, Hard refreshes (RC-6).
public export
data LeaseMode = Soft | Hard

||| Ranks come from a well-founded order; `Nat` suffices (RC-11).
public export
Rank : Type
Rank = Nat

||| The owns-tree: one node with a rank, a lease mode, and the finite
||| forest of resources it owns. Finite by construction.
public export
data RTree : Type where
  RNode : Rank -> LeaseMode -> List RTree -> RTree

||| The rank at the root of a tree.
public export
rankOf : RTree -> Rank
rankOf (RNode r _ _) = r

mutual
  ||| RC-1/RC-2 as an inductive witness: a tree is Ranked when every owned
  ||| child is strictly lower-ranked than its owner, recursively.
  public export
  data Ranked : RTree -> Type where
    MkRanked : {r : Rank} -> {m : LeaseMode} -> {cs : List RTree}
            -> RankedForest r cs
            -> Ranked (RNode r m cs)

  ||| A forest of children under a parent of rank `r`: each child's rank is
  ||| strictly below `r`, and each child is itself Ranked.
  public export
  data RankedForest : Rank -> List RTree -> Type where
    RFNil  : RankedForest r []
    RFCons : {r : Rank} -> {c : RTree} -> {cs : List RTree}
          -> LT (rankOf c) r
          -> Ranked c
          -> RankedForest r cs
          -> RankedForest r (c :: cs)

||| Number of nodes in a tree / forest (the residue measure: live nodes).
mutual
  public export
  size : RTree -> Nat
  size (RNode _ _ cs) = S (sizeForest cs)

  public export
  sizeForest : List RTree -> Nat
  sizeForest [] = 0
  sizeForest (t :: ts) = size t + sizeForest ts

||| The ranks of the roots of a forest — the multiset (as a list) that a
||| teardown step substitutes for its parent's rank.
public export
ranksOf : List RTree -> List Rank
ranksOf [] = []
ranksOf (t :: ts) = rankOf t :: ranksOf ts
