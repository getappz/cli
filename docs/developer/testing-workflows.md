# Testing GitHub Workflows Locally

This guide explains how to test the GitHub Actions workflows locally before pushing changes.

## Quick Start

### Option 1: Test Build Scripts Directly (Fastest)

**Linux/macOS:**
```bash
# Install UPX first
sudo apt-get install -y upx-ucl  # Linux
brew install upx                 # macOS

# Test the build script
./scripts/build-tarball.sh x86_64-unknown-linux-gnu
```

**Windows:**
```powershell
# Install UPX (download from https://github.com/upx/upx/releases)
# Then test:
$env:ARCH = "x64"
.\scripts\build-tarball.ps1 x86_64-pc-windows-msvc
```

### Option 2: Use Helper Scripts

**Linux (using Docker):**
```bash
chmod +x scripts/test-workflow-linux.sh
./scripts/test-workflow-linux.sh x86_64-unknown-linux-gnu
```

**Windows:**
```powershell
.\scripts\test-workflow-windows.ps1 -Target x86_64-pc-windows-msvc -Arch x64
```

### Option 3: Use `act` (Full Workflow Testing)

Install `act`:
```bash
# Windows
choco install act-cli
# or
scoop install act

# macOS
brew install act

# Linux
curl https://raw.githubusercontent.com/nektos/act/master/install.sh | sudo bash
```

Test specific jobs:
```bash
# Test Linux build
act push -j build-tarball-linux --matrix name:linux-x64

# Test macOS build  
act push -j build-tarball-macos --matrix name:macos-x64

# Test Windows build
act push -j build-tarball-windows --matrix arch:x64
```

## Testing UPX Compression

To verify UPX compression is working:

1. **Build without UPX** (comment out UPX step temporarily)
2. **Build with UPX** (normal build)
3. **Compare binary sizes**:
   ```bash
   ls -lh dist/appz/bin/appz
   ```

The UPX-compressed binary should be significantly smaller.

## Testing Different Architectures

**Linux:**
```bash
./scripts/build-tarball.sh x86_64-unknown-linux-gnu      # x64
./scripts/build-tarball.sh aarch64-unknown-linux-gnu    # ARM64
./scripts/build-tarball.sh armv7-unknown-linux-gnueabi   # ARMv7
```

**macOS:**
```bash
./scripts/build-tarball.sh x86_64-apple-darwin           # Intel
./scripts/build-tarball.sh aarch64-apple-darwin          # Apple Silicon
```

**Windows:**
```powershell
$env:ARCH = "x64"
.\scripts\build-tarball.ps1 x86_64-pc-windows-msvc

$env:ARCH = "arm64"
.\scripts\build-tarball.ps1 aarch64-pc-windows-msvc
```

## Verifying UPX Installation

**Linux/macOS:**
```bash
which upx
upx --version
```

**Windows:**
```powershell
Get-Command upx
upx --version
```

## Troubleshooting

### UPX not found
- **Linux**: Install with `sudo apt-get install -y upx-ucl`
- **macOS**: Install with `brew install upx`
- **Windows**: Download from https://github.com/upx/upx/releases and add to PATH

### UPX fails on ARM architectures
This is expected behavior. The scripts are configured to skip UPX compression gracefully for ARM architectures if it fails.

### Cross-compilation issues
For Linux cross-compilation, ensure Docker is running and `cross` is installed:
```bash
cargo install cross --git https://github.com/cross-rs/cross
```

## Testing Release Workflow

The release workflow is triggered by tags. To test locally:

1. **Create a test tag:**
   ```bash
   git tag -a v2025.1.0-test -m "Test release"
   ```

2. **Test with act:**
   ```bash
   act push --eventpath <(echo '{"ref":"refs/tags/v2025.1.0-test"}') -j release
   ```

3. **Clean up:**
   ```bash
   git tag -d v2025.1.0-test
   ```

## Best Practices

1. **Always test build scripts locally** before pushing workflow changes
2. **Test on the target platform** when possible (or use Docker for Linux)
3. **Verify UPX compression** by comparing binary sizes
4. **Test ARM architectures** separately as they may have different behavior
5. **Check artifacts** in `dist/` directory after build

