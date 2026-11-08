$ErrorActionPreference = 'Stop'
Set-Location (Split-Path -Parent $PSScriptRoot)

cargo build --release --locked
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$versionLine = Select-String -Path Cargo.toml -Pattern '^version = "([^"]+)"' | Select-Object -First 1
$version = $versionLine.Matches[0].Groups[1].Value
New-Item -ItemType Directory -Force -Path dist | Out-Null
$output = "dist/vimpp-$version-windows-x86_64.exe"
Copy-Item target/release/vimpp.exe $output -Force
Write-Host "Created $output"
