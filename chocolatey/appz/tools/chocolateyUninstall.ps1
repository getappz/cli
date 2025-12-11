$ErrorActionPreference = 'Stop'

$packageName = 'appz'
$toolsDir = "$(Split-Path -Parent $MyInvocation.MyCommand.Definition)"
$installDir = Join-Path $toolsDir 'appz'
$binPath = Join-Path $installDir 'appz\bin'

# Remove from PATH
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($userPath -like "*$binPath*") {
    $newPath = ($userPath -split ';' | Where-Object { $_ -ne $binPath }) -join ';'
    [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
    Write-Host "Removed $binPath from PATH"
}

# Remove installation directory
if (Test-Path $installDir) {
    Remove-Item -Path $installDir -Recurse -Force
    Write-Host "Removed installation directory: $installDir"
}

Write-Host "$packageName uninstalled successfully"

