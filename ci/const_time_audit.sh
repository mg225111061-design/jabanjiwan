#!/usr/bin/env bash
# CLAUDE.md R6 / APPENDIX K.2: every secret[T] path must be data-oblivious.
# Builds each crypto/pqc/secret fixture with --const-time-audit; a data-dependent
# branch/index on a secret is a compile error (E0301/E0302) and fails the gate.
#
# The audit is SOUND, not complete (jeff-types): it never accepts a leaking
# branch/index (no false negatives); it may conservatively reject a safe one. See
# jeff-types' module docs for the leakage model (in scope: source/IR-level branch +
# memory-index; out of scope: Hertzbleed/DVFS, DMP prefetch, cache microarchitecture).
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build -q -p jeffc
BIN=target/debug/jeffc

files=$(git ls-files '*.jeff' | grep -E 'crypto|pqc|secret' || true)
if [ -z "$files" ]; then
  echo "const-time-audit OK (no secret[T] fixtures)"
  exit 0
fi

fail=0
for f in $files; do
  if ! "$BIN" build "$f" --const-time-audit; then
    echo "const-time FAIL in $f (R6)"
    fail=1
  fi
done

if [ "$fail" -ne 0 ]; then
  exit 1
fi
echo "const-time-audit OK"
