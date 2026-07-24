#!/bin/bash
set -e

source dev-container-features-test-lib

check "appz is on PATH" bash -c "command -v appz"
check "appz --version runs" appz --version

reportResults
