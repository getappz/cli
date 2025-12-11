#!/usr/bin/env bash
# Run appz-cli from anywhere
# Usage: ./run.sh [args...]
# Alias: alias appz='/home/avihs/projects/appz-cli/run.sh'

CWD="$(pwd)"
cd /home/avihs/projects/appz-cli && cargo run --package cli --quiet -- --cwd "$CWD" "$@"