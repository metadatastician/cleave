<!-- SPDX-License-Identifier: CC-BY-SA-4.0 -->
# PRUNE-MANIFEST — cleave

**WARNING: execute only after owner sign-off.** Git history is the undo
button. Standard: `groove-protocol/docs/REPO-LAYOUT.adoc` (ADR 0008).
Rationale: this repo is a design corpus (see README); everything below is
un-customised RSR-template residue that tests/validates the *template*, not
cleave.

## KEEP

```
README.adoc  LICENSE  CHANGELOG.md  PRUNE-MANIFEST.md  0-AI-MANIFEST.a2ml
docs/KERNEL.adoc  docs/PROOF-NEEDS.adoc
docs/standards/  docs/architecture/  docs/decisions/
docs/status/PROOF-STATUS.adoc  docs/status/READINESS.adoc
.gitattributes  .editorconfig  .gitignore
.github/workflows/structure-check.yml  tests/validate_structure.sh
```

## DELETE — run from the repo root, one commit

```bash
# 1. Template scaffolding trees
git rm -r -q .machine_readable machine-readable-design docs-template session \
  benches build container features tools scripts verification examples \
  .devcontainer .well-known

# 2. Template source + template tests (generic FFI skeleton; tests test the
#    template's placeholder replacement, not cleave). KERNEL.adoc restarts src/.
git rm -r -q src
git rm -q tests/e2e/template_instantiation_test.sh tests/aspect_tests.sh 2>/dev/null || true
git rm -r -q tests/workflows tests/e2e 2>/dev/null || true

# 3. Byte-identical governance clones + template state files
git rm -q AUDIT.adoc EXPLAINME.adoc GOVERNANCE.adoc MAINTAINERS.adoc \
  coordination.k9 Justfile .gitlab-ci.yml .pre-commit-config.yaml \
  .tool-versions .envrc

# 4. Wrong-repo / unfilled status docs (TEST-NEEDS is for "rsr-template-repo")
git rm -q docs/status/TEST-NEEDS.adoc docs/status/ROADMAP.adoc \
  docs/QUICKSTART.adoc docs/RSR_OUTLINE.adoc docs/STATE-VISUALIZER.adoc 2>/dev/null || true

# 5. Empty/scaffold docs trees (verify template-only first — see check below)
git rm -r -q docs/theory docs/whitepapers docs/governance docs/practice \
  docs/reports docs/attribution docs/developer docs/onboarding \
  docs/proposals docs/wikis docs/legal 2>/dev/null || true

# 6. Per-directory AI manifests everywhere except the root one
find . -name '*AI-MANIFEST*' -not -path './.git/*' -not -path './0-AI-MANIFEST.a2ml' \
  -exec git rm -q {} +

# 7. 25 template workflows -> 1 structure check
cd .github/workflows
git rm -q boj-build.yml codeql.yml dependabot-automerge.yml dogfood-gate.yml \
  e2e.yml estate-rules.yml governance.yml guix-nix-policy.yml hypatia-scan.yml \
  instant-sync.yml mirror.yml npm-bun-blocker.yml openssf-compliance.yml \
  quality.yml release.yml rhodibot.yml rsr-antipattern.yml rust-ci.yml \
  scorecard.yml secret-scanner.yml security-policy.yml \
  static-analysis-gate.yml ts-blocker.yml wellknown-enforcement.yml \
  workflow-linter.yml
cd ../..
```

## Pre-flight check (before step 5)

```bash
# Confirm a docs subtree is template-only before removing it:
find docs/theory -type f | grep -v -E 'AI-MANIFEST|README|\.gitkeep' || echo "template-only: safe"
```

## Verify after

```bash
find . -name '*AI-MANIFEST*' -not -path './.git/*' | wc -l   # => 1
ls .github/workflows | wc -l                                 # => 1
ls -A | grep -v '^\.git$' | wc -l                            # fits one screen
bash tests/validate_structure.sh                             # strict mode now applies
```

Then: flip `tests/validate_structure.sh` to strict (it warns pre-prune), and
the owner archives nothing here — cleave stays live (ADR 0007); next
milestone is `docs/KERNEL.adoc`.
