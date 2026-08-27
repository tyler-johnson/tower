# tower installer for Windows (PowerShell 5+):
#   irm https://raw.githubusercontent.com/tyler-johnson/tower/main/install.ps1 | iex
#
# Downloads the latest release binary, verifies its sha256 against the
# release's checksums.txt, installs to %LOCALAPPDATA%\Programs\ff-tower,
# and adds that directory to your user PATH. Pin a version by setting
# $env:TOWER_VERSION (e.g. v0.1.0) first.
$ErrorActionPreference = 'Stop'

$repo = 'tyler-johnson/tower'
$installDir = Join-Path $env:LOCALAPPDATA 'Programs\ff-tower'

$arch = switch ($env:PROCESSOR_ARCHITECTURE) {
    'AMD64' { 'amd64' }
    'ARM64' { 'arm64' }
    default { throw "tower installer: unsupported architecture $env:PROCESSOR_ARCHITECTURE" }
}

$version = $env:TOWER_VERSION
if (-not $version) {
    $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$repo/releases/latest" -Headers @{ 'User-Agent' = 'tower-installer' }
    $version = $release.tag_name
}
if (-not $version) { throw 'tower installer: could not determine the latest release' }

$archive = "ff-tower_$($version.TrimStart('v'))_windows_$arch.zip"
$base = "https://github.com/$repo/releases/download/$version"

$tmp = Join-Path $env:TEMP "tower-install-$PID"
New-Item -ItemType Directory -Force -Path $tmp | Out-Null
try {
    Write-Host "downloading tower $version (windows/$arch)..."
    Invoke-WebRequest -Uri "$base/$archive" -OutFile (Join-Path $tmp $archive)
    Invoke-WebRequest -Uri "$base/checksums.txt" -OutFile (Join-Path $tmp 'checksums.txt')

    $line = Get-Content (Join-Path $tmp 'checksums.txt') | Where-Object { $_ -match [regex]::Escape($archive) }
    if (-not $line) { throw "tower installer: checksums.txt has no entry for $archive" }
    $want = ($line -split '\s+')[0]
    $got = (Get-FileHash -Algorithm SHA256 (Join-Path $tmp $archive)).Hash
    if ($got -ne $want) { throw "tower installer: checksum mismatch for $archive - refusing to install" }

    Expand-Archive -Path (Join-Path $tmp $archive) -DestinationPath $tmp -Force
    New-Item -ItemType Directory -Force -Path $installDir | Out-Null
    Copy-Item (Join-Path $tmp 'ff-tower.exe') (Join-Path $installDir 'ff-tower.exe') -Force
} finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}

Write-Host "installed tower $version to $installDir\ff-tower.exe"

# Put the install directory on the user PATH so new terminals find ff-tower.
$userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
if (($userPath -split ';') -notcontains $installDir) {
    [Environment]::SetEnvironmentVariable('Path', "$userPath;$installDir", 'User')
    Write-Host "added $installDir to your user PATH - open a new terminal to pick it up"
}
if (($env:Path -split ';') -notcontains $installDir) {
    $env:Path = "$env:Path;$installDir"
}

if (-not (Get-Command ff -ErrorAction SilentlyContinue)) {
    Write-Host ''
    Write-Host 'tower is reached through fufu (`ff tower`), and no `ff` is on your PATH.'
    Write-Host 'install fufu first:'
    Write-Host '  irm https://raw.githubusercontent.com/tyler-johnson/fufu/main/install.ps1 | iex'
}

Write-Host ''
Write-Host 'next steps:'
Write-Host "  ff tower                       # the board"
