#!/bin/bash
# Generate test SSH keys for Docker testing

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
KEYS_DIR="$PROJECT_ROOT/test-keys"

mkdir -p "$KEYS_DIR"

if [ ! -f "$KEYS_DIR/test_key" ]; then
    echo "Generating test SSH keys..."
    ssh-keygen -t rsa -b 2048 -f "$KEYS_DIR/test_key" -N "" -C "saasctl-test-key"
    echo "Keys generated in $KEYS_DIR"
    echo ""
    echo "To use these keys in your recipe.yaml:"
    echo "  identity_file: \"$KEYS_DIR/test_key\""
    echo ""
    echo "To connect to the test server:"
    echo "  ssh -i $KEYS_DIR/test_key -p 2222 testuser@localhost"
else
    echo "Test keys already exist in $KEYS_DIR"
fi

