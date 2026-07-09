-- SPDX-License-Identifier: MPL-2.0
-- Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
--
||| Tree-shaped resource cleanup (the G-1 upgrade): the teardown plan is a
||| POSTORDER over the owns-tree — children before parents (RC-8) — rather
||| than gossamer's flat-LIFO list recursion. Rebuilt fresh against the
||| kernel's shapes per G-6; runtime mirror is `src/teardown.rs` +
||| KERN-3/KERN-4.
|||
||| Theorems:
|||   * `gracefulTeardownTerminates` — the plan of a finite tree is a
|||     finite list of steps. Totality of `plan` (checked by Idris2's
|||     totality checker on the structural recursion) is the termination
|||     argument; per DMMultiset.idr this is the Dershowitz–Manna argument
|||     specialised to a tree fixed at connect time. Named per the spec's
|||     slogan — NOT `residueAlwaysZero` (rupture is out of scope, RC-10).
|||   * `planPostorder` — the plan discharges children-forest first, parent
|||     last (definitional equation, exported as evidence).
|||   * `planParentLast` — the parent is the LAST entry of its subtree's
|||     plan segment (order-dual of construction, RC-8/O-4).
|||   * `planComplete` — every node is discharged exactly once: the plan's
|||     length equals the subtree's size.
|||   * `residueZeroAtPerp` — after executing the whole plan the residue
|||     measure (size minus steps executed) is zero: ⊥ IS the zero-residue
|||     state (O-2/RC-7).

module Cleave.Kernel.ResourceCleanup

import Data.Nat
import Cleave.Kernel.Types

%default total

mutual
  ||| The discharge plan: postorder ranks (rank stands in for the node at
  ||| its tree position). Children-forest first, then the parent.
  public export
  plan : RTree -> List Rank
  plan (RNode r _ cs) = planForest cs ++ [r]

  public export
  planForest : List RTree -> List Rank
  planForest [] = []
  planForest (t :: ts) = plan t ++ planForest ts

-- --- small self-contained lemmas (base-only, no contrib) ----------------

||| length distributes over append.
lenAppend : (xs, ys : List Rank) -> length (xs ++ ys) = length xs + length ys
lenAppend [] ys = Refl
lenAppend (x :: xs) ys = cong S (lenAppend xs ys)

||| n + 1 = S n.
plusOneSucc : (n : Nat) -> n + 1 = S n
plusOneSucc Z = Refl
plusOneSucc (S k) = cong S (plusOneSucc k)

||| minus n n = 0.
minusSelf : (n : Nat) -> minus n n = Z
minusSelf Z = Refl
minusSelf (S k) = minusSelf k

||| The last element of a list, safely.
public export
lastOf : List Rank -> Maybe Rank
lastOf [] = Nothing
lastOf [x] = Just x
lastOf (_ :: xs@(_ :: _)) = lastOf xs

||| A snoc list is never empty, and its last element is the snocced one.
lastOfSnoc : (xs : List Rank) -> (x : Rank) -> lastOf (xs ++ [x]) = Just x
lastOfSnoc [] x = Refl
lastOfSnoc (y :: []) x = Refl
lastOfSnoc (y :: z :: zs) x = lastOfSnoc (z :: zs) x

-- --- theorems -----------------------------------------------------------

||| The postorder equation, exported as evidence: a node's plan is its
||| children-forest's plan followed by the node itself. Children before
||| parent (RC-8), definitionally.
public export
planPostorder : (r : Rank) -> (m : LeaseMode) -> (cs : List RTree)
             -> plan (RNode r m cs) = planForest cs ++ [r]
planPostorder r m cs = Refl

||| The parent is the last entry of its own subtree's plan segment: no
||| parent is discharged while any of its (transitively) owned children
||| remains undischarged within that segment (order-dual, O-4).
public export
planParentLast : (r : Rank) -> (m : LeaseMode) -> (cs : List RTree)
              -> lastOf (plan (RNode r m cs)) = Just r
planParentLast r m cs = lastOfSnoc (planForest cs) r

mutual
  ||| Every owned node is discharged exactly once: the plan visits as many
  ||| steps as the subtree has nodes. (Explicit `trans`/`cong` chains
  ||| rather than `rewrite`, so the equational direction is unambiguous.)
  public export
  planComplete : (t : RTree) -> length (plan t) = size t
  planComplete (RNode r m cs) =
    trans
      (trans (lenAppend (planForest cs) [r])
             (cong (\k => k + 1) (planForestComplete cs)))
      (plusOneSucc (sizeForest cs))

  public export
  planForestComplete : (ts : List RTree) -> length (planForest ts) = sizeForest ts
  planForestComplete [] = Refl
  planForestComplete (t :: ts) =
    trans (lenAppend (plan t) (planForest ts))
          (cong2 (+) (planComplete t) (planForestComplete ts))

||| Graceful teardown terminates: the discharge plan of a finite owns-tree
||| is a finite list — there is a step count, and it is exactly the
||| subtree's size. (The totality checker validates the structural
||| recursion of `plan`; this exports the bound as a value.)
public export
gracefulTeardownTerminates : (t : RTree) -> (steps : Nat ** length (plan t) = steps)
gracefulTeardownTerminates t = (size t ** planComplete t)

||| ⊥ is the zero-residue state: after executing every step of the plan,
||| the residue measure (nodes not yet discharged) is zero (O-2, RC-7).
public export
residueZeroAtPerp : (t : RTree) -> minus (size t) (length (plan t)) = Z
residueZeroAtPerp t =
  trans (cong (minus (size t)) (planComplete t)) (minusSelf (size t))
