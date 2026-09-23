#!/usr/bin/env bash
# SPDX-License-Identifier: MPL-2.0
# Copyright (c) 2026 Jonathan D.A. Jewell (hyperpolymath) <j.d.a.jewell@open.ac.uk>
#
# Regression tests for the foundation CI security configuration.

set -uo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
dependabot_file="$repo_root/.github/dependabot.yml"
codeql_file="$repo_root/.github/workflows/codeql.yml"
scorecard_file="$repo_root/.github/workflows/scorecard.yml"
failures=0

pass() {
    printf 'PASS: %s\n' "$1"
}

fail() {
    printf 'FAIL: %s\n' "$1" >&2
    failures=$((failures + 1))
}

assert_file() {
    local file="$1"
    local label="$2"

    if [[ -f "$file" ]]; then
        pass "$label"
    else
        fail "$label"
    fi
}

assert_line() {
    local file="$1"
    local pattern="$2"
    local label="$3"

    if grep -Eq "$pattern" "$file"; then
        pass "$label"
    else
        fail "$label"
    fi
}

assert_no_line() {
    local file="$1"
    local pattern="$2"
    local label="$3"

    if grep -Eq "$pattern" "$file"; then
        fail "$label"
    else
        pass "$label"
    fi
}

assert_text_line() {
    local text="$1"
    local pattern="$2"
    local label="$3"

    if grep -Eq "$pattern" <<<"$text"; then
        pass "$label"
    else
        fail "$label"
    fi
}

assert_text_line_once() {
    local text="$1"
    local pattern="$2"
    local label="$3"
    local count

    count="$(grep -Ec "$pattern" <<<"$text" || true)"
    if [[ "$count" == "1" ]]; then
        pass "$label"
    else
        fail "$label"
    fi
}

named_step() {
    local file="$1"
    local step_name="$2"

    awk -v step_name="$step_name" '
        /^      - name:/ {
            active = ($0 == "      - name: " step_name)
            if (active) {
                matches++
            }
        }
        active {
            print
        }
        END {
            if (matches != 1) {
                exit 1
            }
        }
    ' "$file"
}

dependabot_limit() {
    local ecosystem="$1"

    awk -v ecosystem="$ecosystem" '
        /^  - package-ecosystem:/ {
            active = ($0 == "  - package-ecosystem: \"" ecosystem "\"")
            if (active) {
                blocks++
            }
            next
        }
        active && /^    open-pull-requests-limit:/ {
            value = $0
            sub(/^    open-pull-requests-limit:[[:space:]]*/, "", value)
            limits++
        }
        END {
            if (blocks != 1 || limits != 1) {
                exit 1
            }
            print value
        }
    ' "$dependabot_file"
}

assert_dependabot_limit() {
    local ecosystem="$1"
    local expected="$2"
    local actual

    if actual="$(dependabot_limit "$ecosystem")" && [[ "$actual" == "$expected" ]]; then
        pass "Dependabot $ecosystem updates are capped at $expected open pull requests"
    else
        fail "Dependabot $ecosystem cap is exactly $expected and occurs once in its update block"
    fi
}

assert_file "$dependabot_file" "Dependabot configuration exists"
assert_file "$codeql_file" "CodeQL workflow exists"
assert_file "$scorecard_file" "Scorecard workflow exists"

if [[ -f "$dependabot_file" ]]; then
    assert_dependabot_limit "github-actions" "2"
    assert_dependabot_limit "mix" "3"
    assert_dependabot_limit "npm" "3"
    assert_dependabot_limit "pip" "3"
fi

if [[ -f "$codeql_file" ]]; then
    checkout_sha='3d3c42e5aac5ba805825da76410c181273ba90b1'
    if checkout_step="$(named_step "$codeql_file" 'Checkout')"; then
        assert_text_line_once "$checkout_step" \
            "^        uses: actions/checkout@${checkout_sha}([[:space:]]+#.*)?$" \
            "CodeQL checkout step uses the reviewed immutable commit exactly once"
        assert_text_line_once "$checkout_step" \
            '^          persist-credentials:[[:space:]]+false([[:space:]]+#.*)?$' \
            "CodeQL checkout step does not persist its token"
    else
        fail "CodeQL has exactly one named Checkout step"
    fi
    assert_no_line "$codeql_file" \
        '^[[:space:]]+uses: actions/checkout@(v[0-9]|main|master)([^0-9a-f]|$)' \
        "CodeQL checkout does not use a mutable ref"
fi

if [[ -f "$scorecard_file" ]]; then
    scorecard_sha='8750b94ac1bbe8c51ad13fe106669b13478f0b62'
    assert_line "$scorecard_file" '^  schedule:$' \
        "Scorecard runs on a schedule"
    assert_line "$scorecard_file" '^    - cron: "0 0 \* \* 0"$' \
        "Scorecard retains its weekly cadence"
    assert_line "$scorecard_file" '^  push:$' \
        "Scorecard runs when the default branch changes"
    assert_line "$scorecard_file" '^    branches: \[main, master\]$' \
        "Scorecard push runs are limited to default-branch names"
    assert_line "$scorecard_file" '^  workflow_dispatch:$' \
        "Scorecard supports manual runs"
    assert_line "$scorecard_file" '^  group: \$\{\{ github\.workflow \}\}-\$\{\{ github\.ref \}\}$' \
        "Scorecard concurrency is isolated by workflow and ref"
    assert_line "$scorecard_file" '^  cancel-in-progress:[[:space:]]+true$' \
        "Scorecard cancels superseded runs"

    assert_line "$scorecard_file" '^  actions:[[:space:]]+read$' \
        "Scorecard has read-only Actions access"
    assert_line "$scorecard_file" '^  contents:[[:space:]]+read$' \
        "Scorecard has read-only repository access"
    assert_line "$scorecard_file" '^  security-events:[[:space:]]+write$' \
        "Scorecard can publish security results"
    assert_line "$scorecard_file" '^  id-token:[[:space:]]+write$' \
        "Scorecard can request an OIDC token"
    assert_no_line "$scorecard_file" \
        '^[[:space:]]*(permissions:[[:space:]]+write-all|contents:[[:space:]]+write)([[:space:]]+#.*)?$' \
        "Scorecard does not grant broad repository write access"

    assert_line "$scorecard_file" \
        "^    uses: hyperpolymath/standards/\\.github/workflows/scorecard-reusable\\.yml@${scorecard_sha}$" \
        "Scorecard calls the reviewed immutable reusable workflow"
    assert_no_line "$scorecard_file" \
        '^[[:space:]]+uses: hyperpolymath/standards/.+@(main|master|v[0-9])([^0-9a-f]|$)' \
        "Scorecard does not use a mutable reusable-workflow ref"
    assert_no_line "$scorecard_file" '^[[:space:]]+secrets:[[:space:]]+inherit([[:space:]]+#.*)?$' \
        "Scorecard does not forward repository secrets"
fi

if ((failures > 0)); then
    printf '\nFoundation CI security tests failed: %d failure(s)\n' "$failures" >&2
    exit 1
fi

printf '\nFoundation CI security tests passed\n'
