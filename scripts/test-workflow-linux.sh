#!/usr/bin/env bash
# Helper script to test Linux build workflow locally using Docker
set -euo pipefail

# This script simulates the GitHub Actions Linux build environment
# Usage: ./scripts/test-workflow-linux.sh [target]

TARGET=${1:-x86_64-unknown-linux-gnu}

echo "Testing Linux build workflow for target: $TARGET"

# Use cross Docker image (same as GitHub Actions)
docker run --rm -it \
  -v "$(pwd):/workspace" \
  -w /workspace \
  -e CARGO_TERM_COLOR=always \
  -e RUST_BACKTRACE=1 \
  ghcr.io/cross-rs/x86_64-unknown-linux-gnu:0.2.5 \
  bash -c "
    apt-get update && apt-get install -y upx-ucl
    ./scripts/build-tarball.sh $TARGET
  "

echo "Build completed! Check dist/ directory for artifacts."

