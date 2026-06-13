#!/usr/bin/env bash
# CLAUDE.md R11 / APPENDIX K.2: same input + flags => identical compile artifacts
# (certificates included).
set -euo pipefail
cd "$(dirname "$0")/.."

F=tests/e2e/triangular.jeff
O1="$(mktemp -d)"
O2="$(mktemp -d)"
trap 'rm -rf "$O1" "$O2"' EXIT

cargo build -q -p jeffc
BIN=target/debug/jeffc

"$BIN" build "$F" --emit-certificates "$O1" >/dev/null
"$BIN" build "$F" --emit-certificates "$O2" >/dev/null

if diff -r "$O1" "$O2" >/dev/null; then
  echo "determinism OK"
else
  echo "error: non-deterministic output (R11)"
  diff -r "$O1" "$O2" || true
  exit 1
fi
