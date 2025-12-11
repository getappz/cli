# Generate test SSH keys for Docker testing (PowerShell)

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ProjectRoot = Split-Path -Parent $ScriptDir
$KeysDir = Join-Path $ProjectRoot "test-keys"

if (-not (Test-Path $KeysDir)) {
    New-Item -ItemType Directory -Path $KeysDir | Out-Null
}

$KeyPath = Join-Path $KeysDir "test_key"

if (-not (Test-Path $KeyPath)) {
    Write-Host "Generating test SSH keys..."
    
    # Check if ssh-keygen is available
    if (Get-Command ssh-keygen -ErrorAction SilentlyContinue) {
        ssh-keygen -t rsa -b 2048 -f $KeyPath -N '""' -C "saasctl-test-key"
        Write-Host "Keys generated in $KeysDir"
        Write-Host ""
        Write-Host "To use these keys in your recipe.yaml:"
        Write-Host "  identity_file: `"$KeyPath`""
        Write-Host ""
        Write-Host "To connect to the test server:"
        Write-Host "  ssh -i $KeyPath -p 2222 testuser@localhost"
    } else {
        Write-Host "Error: ssh-keygen not found. Please install OpenSSH or use WSL."
        Write-Host "Alternatively, you can generate keys manually using:"
        Write-Host "  ssh-keygen -t rsa -b 2048 -f $KeyPath -N ''"
        exit 1
    }
} else {
    Write-Host "Test keys already exist in $KeysDir"
}

