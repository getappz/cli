# Plugin Build System

Build and publish appz WASM plugins to the CDN.

## Prerequisites

1. **Rust toolchain** with wasm32 target:
   ```bash
   rustup target add wasm32-wasip1
   ```

2. **Signing key** (optional, for generating .sig files): see [Generating .sig files](#generating-sig-files) below.

## Commands

```bash
# Build plugins (compile to WASM)
cargo run -p plugin-build -- build

# Build a specific plugin
cargo run -p plugin-build -- build --plugin check

# Full package: build + inject header + sign + checksum
cargo run -p plugin-build -- package

# Publish to CDN (requires S3/R2 config)
cargo run -p plugin-build -- publish

# Dry run (update manifest only, no upload)
cargo run -p plugin-build -- publish --dry-run
```

## Configuration

Edit `scripts/plugins.toml` to add plugins or change CDN URLs.

## CDN Upload (S3/R2)

Set these environment variables or add to `plugins.toml`:

| Variable | Description |
|----------|-------------|
| `APPZ_PLUGIN_S3_BUCKET` | S3/R2 bucket name |
| `APPZ_PLUGIN_S3_REGION` | Region (use `auto` for R2) |
| `APPZ_PLUGIN_S3_ENDPOINT` | Custom endpoint (for R2: `https://<account_id>.r2.cloudflarestorage.com`) |

**Cloudflare R2 example:**
```bash
export APPZ_PLUGIN_S3_BUCKET=appz-plugins
export APPZ_PLUGIN_S3_REGION=auto
export APPZ_PLUGIN_S3_ENDPOINT=https://YOUR_ACCOUNT_ID.r2.cloudflarestorage.com
export AWS_ACCESS_KEY_ID=your-r2-access-key
export AWS_SECRET_ACCESS_KEY=your-r2-secret-key
cargo run -p plugin-build -- publish
```

**Credentials:** Use `AWS_ACCESS_KEY_ID` and `AWS_SECRET_ACCESS_KEY` (R2 uses the same env vars).

**AWS S3 example:**
```bash
export APPZ_PLUGIN_S3_BUCKET=appz-plugins-cdn
export APPZ_PLUGIN_S3_REGION=us-east-1
cargo run -p plugin-build -- publish
```

## Output Structure

After `package`:
```
dist/plugins/
├── check/
│   └── 0.1.0/
│       ├── plugin.wasm      # Signed WASM
│       ├── plugin.wasm.sig  # Ed25519 signature
│       ├── checksum.txt     # SHA-256
│       └── manifest_entry.json
├── ssg-migrator/
│   └── 0.1.0/
│       └── ...
└── plugins.json            # Combined manifest
```

## Generating .sig files

Signatures are used by the plugin-manager to verify plugins. To generate `.sig` files:

**1. Generate the Ed25519 keypair (once):**
```bash
./scripts/generate-plugin-signing-key.sh
```
This creates:
- `scripts/signing_key.key` (private key — keep secret, already in .gitignore)
- Updates `crates/plugin-manager/src/signing_key.pub` (public key for verification)

**2. Package with signing:**
```bash
# Set key path (or place signing_key.key in scripts/)
export APPZ_SIGNING_KEY=scripts/signing_key.key

# Package will inject header and sign each plugin
cargo run -p plugin-build -- package
```

Or sign a single WASM file:
```bash
cargo run -p plugin-build -- sign --input dist/plugins/check/0.1.0/plugin.wasm --key scripts/signing_key.key
```
This creates `plugin.wasm.sig` next to the WASM file.

## WASM Target

Default: `wasm32-wasip1`. Override with:
```bash
APPZ_WASM_TARGET=wasm32-wasi cargo run -p plugin-build -- build
```
