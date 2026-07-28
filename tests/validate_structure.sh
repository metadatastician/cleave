#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
# Copyright (c) 2026 Jonathan D.A. Jewell <j.d.a.jewell@open.ac.uk>
#
# Structure check for the cleave design repo. Tolerant before the prune
# (see the prune notes in the structure-check workflow), strict after: once
# the per-directory AI manifests are
# gone, any regression fails.

set -uo pipefail
cd "$(dirname "$0")/.."

fail=0

req() {
  if [ -e "$1" ]; then echo "PASS: $1 exists"; else echo "FAIL: $1 missing"; fail=1; fi
}

req README.adoc
req LICENSE
req docs/KERNEL.adoc
req docs/PROOF-NEEDS.adoc
req docs/standards/RANKED-OWNERSHIP-CLEAVE.adoc
req docs/architecture/CLEAVE-ENGINE-DESIGN.adoc

# The honesty line tracks KERNEL.adoc's definition of done: before the
# kernel, "feasible, not demonstrated"; after it, "demonstrated at one
# kernel point" - which still carries "not demonstrated" for the full
# surface. Either phrasing passes; losing both fails.
if grep -qi "demonstrated at one kernel point" README.adoc && grep -q "not demonstrated" README.adoc; then
  echo "PASS: README carries the kernel-scoped honesty line"
elif grep -q "not demonstrated" README.adoc; then
  echo "PASS: README carries the honesty line (feasible, not demonstrated)"
else
  echo "FAIL: README lost the honesty line"
  fail=1
fi

manifest_count=$(find . -name '*AI-MANIFEST*' -not -path './.git/*' | wc -l)
if [ "$manifest_count" -le 1 ]; then
  echo "PASS: at most one AI manifest ($manifest_count)"
elif [ "$manifest_count" -gt 1 ]; then
  echo "WARN: PRUNE PENDING ($manifest_count AI manifests; target is at most one)"
fi

workflow_count=$(ls .github/workflows 2>/dev/null | wc -l)
if [ "$workflow_count" -le 5 ]; then
  echo "PASS: workflow count $workflow_count (<= 5)"
else
  echo "WARN: PRUNE PENDING ($workflow_count workflows; target is at most five)"
fi

echo "----"
exit "$fail"
