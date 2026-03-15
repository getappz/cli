#!/usr/bin/env bash
set -euo pipefail

PASS=0
FAIL=0
ERRORS=""

run_test() {
    local name="$1"
    shift
    if "$@" >/dev/null 2>&1; then
        printf "  \033[32m✓\033[0m %s\n" "$name"
        ((PASS++))
    else
        printf "  \033[31m✗\033[0m %s\n" "$name"
        ((FAIL++))
        ERRORS="${ERRORS}\n  - ${name}"
    fi
}

run_test_fails() {
    local name="$1"
    shift
    if "$@" >/dev/null 2>&1; then
        printf "  \033[31m✗\033[0m %s (expected failure but succeeded)\n" "$name"
        ((FAIL++))
        ERRORS="${ERRORS}\n  - ${name}"
    else
        printf "  \033[32m✓\033[0m %s\n" "$name"
        ((PASS++))
    fi
}

run_test_output() {
    local name="$1"
    local expected="$2"
    shift 2
    local output
    output=$("$@" 2>&1) || true
    if echo "$output" | grep -qi "$expected"; then
        printf "  \033[32m✓\033[0m %s\n" "$name"
        ((PASS++))
    else
        printf "  \033[31m✗\033[0m %s (expected '%s' in output)\n" "$name" "$expected"
        ((FAIL++))
        ERRORS="${ERRORS}\n  - ${name}"
    fi
}

run_test_exists() {
    local name="$1"
    local path="$2"
    if [ -e "$path" ]; then
        printf "  \033[32m✓\033[0m %s\n" "$name"
        ((PASS++))
    else
        printf "  \033[31m✗\033[0m %s (%s not found)\n" "$name" "$path"
        ((FAIL++))
        ERRORS="${ERRORS}\n  - ${name}"
    fi
}

echo ""
echo "========================================="
echo "  appz CLI smoke tests"
echo "========================================="

# -----------------------------------------------
# 1. Basic CLI
# -----------------------------------------------
echo ""
echo "1. Basic CLI"
run_test "appz --version" appz --version
run_test "appz --help" appz --help
run_test_output "version string" "appz" appz --version

# -----------------------------------------------
# 2. All commands have help
# -----------------------------------------------
echo ""
echo "2. Command help pages"
for cmd in dev build init preview deploy deploy-list dev-server self-update telemetry; do
    run_test "appz $cmd --help" appz "$cmd" --help
done

# -----------------------------------------------
# 3. Cloud commands gated (not in default build)
# -----------------------------------------------
echo ""
echo "3. Cloud commands gated"
for cmd in login logout whoami link unlink; do
    run_test_fails "appz $cmd not available" appz "$cmd" --help
done

# -----------------------------------------------
# 4. deploy --init flag exists, deploy-init removed
# -----------------------------------------------
echo ""
echo "4. Deploy --init"
run_test_output "deploy help shows --init" "init" appz deploy --help
run_test_fails "deploy-init command removed" appz deploy-init --help

# -----------------------------------------------
# 5. Framework detection
# -----------------------------------------------
echo ""
echo "5. Framework detection"

mkdir -p /tmp/test-next && echo '{"dependencies":{"next":"14.0.0"}}' > /tmp/test-next/package.json
run_test_output "detects Next.js" "next" appz dev --cwd /tmp/test-next

mkdir -p /tmp/test-astro && echo '{"dependencies":{"astro":"4.0.0"}}' > /tmp/test-astro/package.json
run_test_output "detects Astro" "astro" appz dev --cwd /tmp/test-astro

mkdir -p /tmp/test-vite && echo '{"devDependencies":{"vite":"5.0.0"}}' > /tmp/test-vite/package.json
run_test_output "detects Vite" "vite" appz dev --cwd /tmp/test-vite

# -----------------------------------------------
# 6. Full flow: init → build (Vite)
# -----------------------------------------------
echo ""
echo "6. Full flow: init → build (Vite)"

PROJ_DIR="/tmp/test-flow-vite"
rm -rf "$PROJ_DIR"

echo "   Scaffolding project..."
if appz init vite --name test-flow-vite --output /tmp --skip-install 2>&1 | tail -5; then
    run_test "init vite project" true
else
    run_test "init vite project" false
fi

run_test_exists "package.json created" "$PROJ_DIR/package.json"
run_test_exists "index.html created" "$PROJ_DIR/index.html"
run_test_output "dev detects vite" "vite" appz dev --cwd "$PROJ_DIR"

echo "   Building project (appz handles deps + build)..."
if appz build --cwd "$PROJ_DIR" 2>&1 | tail -5; then
    run_test "appz build succeeds" true
else
    run_test "appz build succeeds" false
fi

run_test_exists "build output exists" "$PROJ_DIR/dist"
run_test_exists "index.html in dist" "$PROJ_DIR/dist/index.html"

# -----------------------------------------------
# 7. Full flow: init → build (Astro)
# -----------------------------------------------
echo ""
echo "7. Full flow: init → build (Astro)"

PROJ_DIR="/tmp/test-flow-astro"
rm -rf "$PROJ_DIR"

echo "   Scaffolding project..."
if appz init astro --name test-flow-astro --output /tmp --skip-install 2>&1 | tail -5; then
    run_test "init astro project" true
else
    run_test "init astro project" false
fi

run_test_exists "package.json created" "$PROJ_DIR/package.json"
run_test_output "dev detects astro" "astro" appz dev --cwd "$PROJ_DIR"

echo "   Building project (appz handles deps + build)..."
if appz build --cwd "$PROJ_DIR" 2>&1 | tail -5; then
    run_test "appz build succeeds" true
else
    run_test "appz build succeeds" false
fi

run_test_exists "build output exists" "$PROJ_DIR/dist"

# -----------------------------------------------
# 8. Error handling
# -----------------------------------------------
echo ""
echo "8. Error handling"
mkdir -p /tmp/test-empty
run_test_fails "build fails in empty dir" appz build --cwd /tmp/test-empty

# -----------------------------------------------
# Summary
# -----------------------------------------------
echo ""
echo "========================================="
TOTAL=$((PASS + FAIL))
printf "Results: \033[32m%d passed\033[0m, \033[31m%d failed\033[0m out of %d\n" "$PASS" "$FAIL" "$TOTAL"
if [ "$FAIL" -gt 0 ]; then
    printf "\nFailed tests:%b\n" "$ERRORS"
    exit 1
fi
echo ""
echo "All smoke tests passed!"
