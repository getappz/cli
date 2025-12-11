$ErrorActionPreference = 'Stop'

$packageName = 'appz'
$version = $env:ChocolateyPackageVersion
$url = "https://github.com/getappz/cli/releases/download/v${version}/appz-${version}-windows-x64.zip"
$checksum = 'PLACEHOLDER_SHA256'
$checksumType = 'sha256'

$toolsDir = "$(Split-Path -Parent $MyInvocation.MyCommand.Definition)"
$installDir = Join-Path $toolsDir 'appz'

# Download and extract
$zipFile = Join-Path $env:TEMP "$packageName.zip"
Write-Host "Downloading $packageName $version..."
Invoke-WebRequest -Uri $url -OutFile $zipFile -UseBasicParsing

# Verify checksum
$fileHash = Get-FileHash -Path $zipFile -Algorithm SHA256
if ($fileHash.Hash -ne $checksum) {
    throw "Checksum verification failed. Expected: $checksum, Got: $fileHash.Hash"
}

# Extract
Write-Host "Extracting to $installDir..."
Expand-Archive -Path $zipFile -DestinationPath $installDir -Force
Remove-Item $zipFile

# Add to PATH
$binPath = Join-Path $installDir 'appz\bin'
if ($env:PATH -notlike "*$binPath*") {
    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    [Environment]::SetEnvironmentVariable("Path", "$userPath;$binPath", "User")
    $env:Path += ";$binPath"
    Write-Host "Added $binPath to PATH"
}

Write-Host "$packageName $version installed successfully"

