# Extract version from Cargo.toml or git tag
if ($env:GITHUB_REF -and $env:GITHUB_REF -match '^refs/tags/v') {
    $TAG = $env:GITHUB_REF -replace '^refs/tags/', ''
    Write-Output $TAG
} else {
    # Try workspace Cargo.toml first, then crates/cli/Cargo.toml
    $cargoFiles = @("Cargo.toml", "crates/cli/Cargo.toml")
    $version = $null
    
    foreach ($file in $cargoFiles) {
        if (Test-Path $file) {
            $match = Select-String -Path $file -Pattern '^version\s*=\s*"([^"]+)"' | Select-Object -First 1
            if ($match) {
                $version = $match.Matches.Groups[1].Value
                break
            }
        }
    }
    
    if ($version) {
        Write-Output "v$version"
    } else {
        Write-Error "Could not find version in Cargo.toml files"
        exit 1
    }
}

