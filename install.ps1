# AIEN Sovereign Core Universal Windows Installer
# Free and open-source for humanity. Zero telemetry. Zero subscriptions.

Write-Host "==================================================================" -ForegroundColor Cyan
Write-Host "      ⚡ AIEN Sovereign Core: Windows Universal Installer         " -ForegroundColor Cyan
Write-Host "==================================================================" -ForegroundColor Cyan

$ConfigDir = "$env:USERPROFILE\.config\sovereign"
$ImprintDir = "$ConfigDir\imprints\en2"
$BinDir = "$env:USERPROFILE\.local\bin"

New-Item -ItemType Directory -Force -Path $ConfigDir | Out-Null
New-Item -ItemType Directory -Force -Path $ImprintDir | Out-Null
New-Item -ItemType Directory -Force -Path $BinDir | Out-Null

$ConfigFile = "$ConfigDir\operator.toml"
if (-Not (Test-Path $ConfigFile)) {
    $GitName = git config user.name 2>$null
    if (-not $GitName) { $GitName = "Sovereign Operator" }
    $GitEmail = git config user.email 2>$null
    if (-not $GitEmail) { $GitEmail = "operator@local" }

    $ConfigContent = @"
[operator]
name = "$GitName"
email = "$GitEmail"
handle = "operator"
sign_commits = false

[engine]
mode = "api"
api_base_url = "http://127.0.0.1:11434/v1"
api_key = ""
model_id = "default"
max_port = 18006
context_window = 32768
temperature = 0.7
"@
    Set-Content -Path $ConfigFile -Value $ConfigContent
    Write-Host "[+] Generated default configuration at $ConfigFile" -ForegroundColor Green
} else {
    Write-Host "[+] Existing configuration found at $ConfigFile" -ForegroundColor Green
}

if (Test-Path "imprints\en2-trinity") {
    Copy-Item -Recurse -Force "imprints\en2-trinity\*" $ImprintDir
    Write-Host "[+] EN2 Trinity Imprint installed locally to $ImprintDir" -ForegroundColor Green
}

Write-Host "==================================================================" -ForegroundColor Cyan
Write-Host "  ✓ Installation complete. Sovereign computing ready." -ForegroundColor Green
Write-Host "  - Configuration: $ConfigFile"
Write-Host "  - Binaries: $BinDir"
Write-Host "  - EN2 Imprint: $ImprintDir"
Write-Host "==================================================================" -ForegroundColor Cyan
