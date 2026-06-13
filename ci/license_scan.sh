#!/usr/bin/env bash
# CLAUDE.md R5 / APPENDIX K.2: licensing gate.
# Linked (non-dev) dependencies must be MIT / Apache-2.0 / BSD (or other permissive:
# Unlicense, Unicode-3.0). GPL / LGPL / AGPL are forbidden in the release graph.
# Forbidden crates (barvinok, LattE, PPL, GMP, rug, M4RI) may appear only as
# out-of-process dev-deps in jeff-test-oracles.
set -euo pipefail
cd "$(dirname "$0")/.."

fail=0

# cargo-deny if available (preferred), else fall back to cargo tree parsing.
if command -v cargo-deny >/dev/null 2>&1; then
  cargo deny check licenses || fail=1
else
  echo "license_scan: cargo-deny not present; using cargo tree fallback"
fi

# 1) No copyleft licenses in the non-dev dependency graph.
if cargo tree -e no-dev --prefix none -f "{p} | {l}" 2>/dev/null | grep -Ei '\b(A?GPL|LGPL)'; then
  echo "error[license]: copyleft (GPL/LGPL/AGPL) license in non-dev graph (R5)"
  fail=1
fi

# 2) No forbidden crate names linked (dev-deps in jeff-test-oracles are allowed).
FORBIDDEN="barvinok latte ppl gmp-mpfr-sys rug m4ri"
for c in $FORBIDDEN; do
  if cargo tree -e no-dev -i "$c" >/dev/null 2>&1; then
    echo "error[license]: forbidden crate '$c' in non-dev dependency graph (R5)"
    fail=1
  fi
done

if [ "$fail" -ne 0 ]; then
  echo "license-scan FAILED"
  exit 1
fi
echo "license-scan OK"
