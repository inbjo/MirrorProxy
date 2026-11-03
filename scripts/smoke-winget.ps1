[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$ManifestDirectory,
    [Parameter(Mandatory)][ValidateSet('x64', 'arm64')][string]$Architecture,
    [Parameter(Mandatory)][string]$Version
)
$ErrorActionPreference = 'Stop'
$winget = Get-Command winget.exe -ErrorAction SilentlyContinue
if (-not $winget) {
    $package = Get-AppxPackage -AllUsers Microsoft.DesktopAppInstaller |
        Sort-Object Version -Descending | Select-Object -First 1
    if ($package) {
        Add-AppxPackage -Register (Join-Path $package.InstallLocation 'AppxManifest.xml') -DisableDevelopmentMode
    }
    $winget = Get-Command winget.exe -ErrorAction SilentlyContinue
}
if (-not $winget) { throw 'WinGet is not available for the current runner user' }
& $winget.Source settings --enable LocalManifestFiles
if ($LASTEXITCODE -ne 0) { throw 'Could not enable local manifest validation' }
& $winget.Source validate $ManifestDirectory
if ($LASTEXITCODE -ne 0) { throw 'WinGet manifest validation failed' }
$installed = $false
try {
    & $winget.Source install --manifest $ManifestDirectory --architecture $Architecture --scope user --accept-package-agreements --accept-source-agreements --disable-interactivity
    if ($LASTEXITCODE -ne 0) { throw 'WinGet package installation failed' }
    $installed = $true
    $executable = Join-Path $env:LOCALAPPDATA 'Microsoft/WinGet/Links/mirrorproxy.exe'
    if (-not (Test-Path $executable)) { throw 'WinGet did not create the mirrorproxy command alias' }
    $reported = & $executable --version
    if ($LASTEXITCODE -ne 0 -or $reported -ne "mirrorproxy-cli $Version") { throw "Unexpected installed version: $reported" }
    & $executable list
    if ($LASTEXITCODE -ne 0) { throw 'Installed client source catalog command failed' }
    & $winget.Source list --id Inbjo.MirrorProxy --exact --accept-source-agreements --disable-interactivity
    if ($LASTEXITCODE -ne 0) { throw 'Installed package is missing from WinGet registration' }
    Write-Host "Verified WinGet $Architecture installation, command discovery, version and source catalog"
} finally {
    if ($installed) {
        & $winget.Source uninstall --id Inbjo.MirrorProxy --exact --scope user --disable-interactivity
        if ($LASTEXITCODE -ne 0) { throw 'WinGet package uninstallation failed' }
        if (Test-Path (Join-Path $env:LOCALAPPDATA 'Microsoft/WinGet/Links/mirrorproxy.exe')) { throw 'Uninstall left the command alias behind' }
    }
}
