# Helper script to test Windows build workflow locally
# Usage: .\scripts\test-workflow-windows.ps1 [target] [arch]

param(
    [string]$Target = "x86_64-pc-windows-msvc",
    [string]$Arch = "x64"
)

Write-Host "Testing Windows build workflow for target: $Target, arch: $Arch"

# Install UPX if not present
if (-not (Get-Command upx -ErrorAction SilentlyContinue)) {
    Write-Host "Installing UPX..."
    $upxVersion = "4.2.1"
    $upxUrl = "https://github.com/upx/upx/releases/download/v${upxVersion}/upx-${upxVersion}-win64.zip"
    $upxZip = "$env:TEMP\upx.zip"
    $upxDir = "$env:TEMP\upx"
    New-Item -ItemType Directory -Force -Path $upxDir | Out-Null
    Invoke-WebRequest -Uri $upxUrl -OutFile $upxZip
    Expand-Archive -Path $upxZip -DestinationPath $upxDir -Force
    $upxExe = Get-ChildItem -Path $upxDir -Recurse -Filter "upx.exe" | Select-Object -First 1
    $upxPath = Split-Path -Parent $upxExe.FullName
    $env:Path = "$upxPath;$env:Path"
}

# Set environment variables
$env:ARCH = $Arch
$env:OS = "windows"

# Run the build script
.\scripts\build-tarball.ps1 $Target

Write-Host "Build completed! Check dist/ directory for artifacts."

