#!/usr/bin/env bash
# CLAUDE.md R8 / R30 / APPENDIX K.2: no fabricated benchmarks. Speedups must be
# measured (kernel + N, non-uniform), never hardcoded in source/docs/benches.
set -euo pipefail
cd "$(dirname "$0")/.."

# Look for hardcoded "Nx faster" / "speedup" claims in source and benches.
if grep -REn '([0-9]{2,}) *[x×] *(faster|speedup)|exponential speedup|O\(1\) for arbitrary' \
     crates benches 2>/dev/null | grep -v '//.*example' ; then
  echo "error: hardcoded speedup / overclaim found (R8/R30)"
  exit 1
fi
echo "bench-honesty OK"
