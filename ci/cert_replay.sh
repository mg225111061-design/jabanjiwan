#!/usr/bin/env bash
# CLAUDE.md R25 / APPENDIX K.2: every emitted certificate is independently
# re-verified from disk. A certificate that does not re-verify is a P0/P1 failure.
set -euo pipefail
cd "$(dirname "$0")/.."

OUT="$(mktemp -d)"
trap 'rm -rf "$OUT"' EXIT

cargo build -q -p jeffc
BIN=target/debug/jeffc

shopt -s nullglob
for f in tests/e2e/*.jeff; do
  "$BIN" build "$f" --emit-certificates "$OUT/$(basename "$f" .jeff)" >/dev/null
done

# Re-verify every machine-readable certificate independently.
status=0
for d in "$OUT"/*/; do
  cargo run -q -p jeff-verify --bin replay -- "$d" || status=1
done
[ "$status" -eq 0 ] && echo "cert-replay OK" || { echo "cert-replay FAILED"; exit 1; }
