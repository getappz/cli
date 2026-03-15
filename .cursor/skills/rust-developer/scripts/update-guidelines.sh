#!/usr/bin/env bash
# Update Microsoft Rust guidelines from the official source.
# Run from anywhere; resolves paths relative to this script.

set -e
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SKILL_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

cd "$SKILL_DIR"
curl -sL "https://microsoft.github.io/rust-guidelines/agents/all.txt" -o rust-guidelines.txt
python3 split-guidelines.py
echo "Guidelines updated."
