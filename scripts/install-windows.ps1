$ErrorActionPreference = 'Stop'
$repo = 'neofetch-tech/vimpp'
$release = Invoke-RestMethod "https://api.github.com/repos/$repo/releases/latest"
$asset = $release.assets | Where-Object { $_.name -match 'windows-x86_64\.exe$' } | Select-Object -First 1
if (-not $asset) { throw "No Windows x86_64 release asset found for $repo." }

$installDir = Join-Path $env:LOCALAPPDATA 'Programs\VimPP'
New-Item -ItemType Directory -Force -Path $installDir | Out-Null
$exe = Join-Path $installDir 'vimpp.exe'
Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $exe

$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
$parts = @($userPath -split ';' | Where-Object { $_ })
if ($parts -notcontains $installDir) {
    [Environment]::SetEnvironmentVariable('Path', (($parts + $installDir) -join ';'), 'User')
    $env:Path = "$installDir;$env:Path"
}
Write-Host "Installed vimpp to $exe"
Write-Host 'Run: vimpp [file-or-directory]'
