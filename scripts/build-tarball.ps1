Set-StrictMode -Version Latest

$Target = $args[0]
$Version = (./scripts/get-version.ps1) -replace '^v', ''

# Detect architecture from target triple if ARCH env var is not set
if (-not $Env:ARCH) {
    if ($Target -match 'aarch64-') {
        $Env:ARCH = "arm64"
    } elseif ($Target -match 'arm') {
        $Env:ARCH = "armv7"
    } elseif ($Target -match 'x86_64-') {
        $Env:ARCH = "x64"
    } else {
        Write-Error "Could not determine architecture from target: $Target"
        exit 1
    }
    Write-Host "Detected architecture: $Env:ARCH"
}

$BaseName = "appz-$Version-windows-$Env:ARCH"

# Build the binary
Write-Host "Building appz for target: $Target"
cargo build --release --target "$Target" --bin appz

if ($LASTEXITCODE -ne 0) {
    Write-Error "Build failed with exit code $LASTEXITCODE"
    exit $LASTEXITCODE
}

# Check if binary exists
$binaryPath = "target/$Target/release/appz.exe"
if (-not (Test-Path $binaryPath)) {
    Write-Error "Binary not found at: $binaryPath"
    exit 1
}

# Create distribution directory
New-Item -ItemType Directory -Force -Path dist/appz/bin | Out-Null

# Copy binary
Copy-Item $binaryPath dist/appz/bin/appz.exe
Write-Host "Binary copied successfully"

# Compress binary with UPX (skip for ARM architectures if it fails)
$binaryToCompress = "dist/appz/bin/appz.exe"
if (Test-Path $binaryToCompress) {
    if (Get-Command upx -ErrorAction SilentlyContinue) {
        if ($Env:ARCH -eq "arm64") {
            # Try UPX compression for ARM, but don't fail if it doesn't work
            Write-Host "Attempting UPX compression for ARM64..."
            try {
                upx --best $binaryToCompress 2>$null
                if ($LASTEXITCODE -eq 0) {
                    Write-Host "UPX compression successful"
                } else {
                    Write-Host "UPX compression skipped for ARM architecture"
                }
            } catch {
                Write-Host "UPX compression skipped for ARM architecture"
            }
        } else {
            # For non-ARM architectures, compress with UPX (non-blocking)
            Write-Host "Compressing binary with UPX..."
            try {
                upx --best $binaryToCompress
                if ($LASTEXITCODE -eq 0) {
                    Write-Host "UPX compression successful"
                } else {
                    Write-Host "UPX compression failed, continuing without compression"
                }
            } catch {
                Write-Host "UPX compression failed, continuing without compression"
            }
        }
    } else {
        Write-Host "UPX not found, skipping compression"
    }
} else {
    Write-Warning "Binary not found for UPX compression: $binaryToCompress"
}

if (Test-Path README.md) {
    Copy-Item README.md dist/appz/README.md
}
if (Test-Path LICENSE) {
    Copy-Item LICENSE dist/appz/LICENSE
}

Set-Location dist
Compress-Archive -Path appz -DestinationPath "$BaseName.zip" -Force
Set-Location ..
Get-Item "dist/$BaseName.zip" | Select-Object Name, Length

