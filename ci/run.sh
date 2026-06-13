#!/usr/bin/env bash
# CLAUDE.md PART 15 / APPENDIX K.2 — all gates must be green to merge (R13).
#
# Stage 0 active subset (ticket T0.5): build, clippy -D warnings, test, license_scan,
# determinism. Also runs cert_replay (R25) and bench_honesty (R8), which already pass.
# (Coverage R26 is added when cargo-llvm-cov is available; noted, not faked.)
set -euo pipefail
cd "$(dirname "$0")/.."

echo "== build =="
cargo build --workspace

echo "== clippy -D warnings =="
cargo clippy --workspace --all-targets -- -D warnings

echo "== test =="
cargo test --workspace

echo "== license-scan (R5) =="
./ci/license_scan.sh

echo "== determinism (R11) =="
./ci/determinism.sh

echo "== cert-replay (R25) =="
./ci/cert_replay.sh

echo "== bench-honesty (R8) =="
./ci/bench_honesty.sh

echo
echo "ALL STAGE-0 GATES GREEN"
