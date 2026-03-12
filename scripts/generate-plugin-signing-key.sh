#!/bin/bash
# Generate Ed25519 keypair for plugin signing.
# The private key is used by plugin-build to sign WASM artifacts.
# The public key (32 raw bytes) is embedded in plugin-manager for verification.

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
KEY_FILE="$SCRIPT_DIR/signing_key.key"
PUB_FILE="$SCRIPT_DIR/signing_key.pub"
PLUGIN_MANAGER_PUB="$SCRIPT_DIR/../crates/plugin-manager/src/signing_key.pub"

if [ -f "$KEY_FILE" ]; then
    echo "Signing key already exists at $KEY_FILE"
    echo "To regenerate, remove it first."
    exit 0
fi

echo "Generating Ed25519 keypair for plugin signing..."

# Generate private key (PEM format) - used by plugin-build sign
openssl genpkey -algorithm ed25519 -out "$KEY_FILE"

# Extract raw 32-byte public key for plugin-manager (embedded in binary)
openssl pkey -in "$KEY_FILE" -pubout -outform DER | tail -c 32 > "$PUB_FILE"

# Copy to plugin-manager if path exists
if [ -d "$(dirname "$PLUGIN_MANAGER_PUB")" ]; then
    cp "$PUB_FILE" "$PLUGIN_MANAGER_PUB"
    echo "Copied public key to $PLUGIN_MANAGER_PUB"
fi

echo ""
echo "Done. Keys created:"
echo "  Private (for signing): $KEY_FILE"
echo "  Public (for verification): $PUB_FILE"
echo ""
echo "Keep the private key secret! Add signing_key.key to .gitignore."
echo "The public key is committed so the CLI can verify plugin signatures."
