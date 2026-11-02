# Run under Windows PowerShell 5.1 and PowerShell 7 on Windows.
$ErrorActionPreference = 'Stop'
$root = Join-Path ([IO.Path]::GetTempPath()) ('mirrorproxy-windows-test-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $root | Out-Null
$installer = [scriptblock]::Create([IO.File]::ReadAllText((Join-Path $PSScriptRoot 'install.ps1')))
$previousArchitecture = $env:PROCESSOR_ARCHITECTURE
$previousWowArchitecture = $env:PROCESSOR_ARCHITEW6432
function global:Invoke-WebRequest {
    param([switch]$UseBasicParsing, [string]$Uri, [string]$OutFile)
    $global:RequestedDownload = $Uri
    if ($Uri.EndsWith('.sha256')) { [IO.File]::WriteAllText($OutFile, '') }
    else { [IO.File]::WriteAllText($OutFile, 'fixture archive') }
}
try {
    $installed = Join-Path $root 'installed'
    New-Item -ItemType Directory -Path $installed | Out-Null
    $binary = Join-Path $installed 'mirrorproxy.exe'
    [IO.File]::WriteAllText($binary, 'previous client')
    foreach ($case in @('x64', 'wow64', 'arm64', 'wowarm64', 'x86')) {
        $env:PROCESSOR_ARCHITECTURE = switch ($case) { 'x64' { 'AMD64' }; 'wow64' { 'x86' }; 'arm64' { 'ARM64' }; 'wowarm64' { 'AMD64' }; 'x86' { 'x86' } }
        $env:PROCESSOR_ARCHITEW6432 = switch ($case) { 'wow64' { 'AMD64' }; 'wowarm64' { 'ARM64' }; default { $null } }
        $global:RequestedDownload = $null
        $failure = $null
        try { & $installer -Mirror 'https://mirror.example' -Version fixture -InstallDir $installed -Setup -BaseUrl 'https://mirror.example' }
        catch { $failure = $_.Exception.Message }
        $expected = if ($case -ne 'x86') { '*checksum file is empty or invalid*' } else { '*Unsupported Windows architecture*' }
        if (-not $failure -or $failure -notlike $expected) { throw "Unexpected $case installer failure: $failure" }
        if ($case -ne 'x86') {
            if ($failure -notlike '*installer failed at line*') { throw 'Installer error lacks a line number' }
            $target = if ($case -in @('arm64', 'wowarm64')) { 'aarch64' } else { 'x86_64' }
            $expectedUrl = "https://mirror.example/https://github.com/inbjo/MirrorProxy/releases/download/fixture/mirrorproxy-client-$target-pc-windows-msvc.zip.sha256"
            if ($global:RequestedDownload -ne $expectedUrl) { throw "Incorrect $case asset: $global:RequestedDownload" }
        } elseif ($global:RequestedDownload) { throw 'Unsupported architecture attempted a download' }
        if ([IO.File]::ReadAllText($binary) -ne 'previous client') { throw 'Existing client was overwritten' }
        Write-Host "PASS PowerShell $($PSVersionTable.PSVersion): Windows $case architecture and existing client preservation"
    }
} finally {
    $env:PROCESSOR_ARCHITECTURE = $previousArchitecture
    $env:PROCESSOR_ARCHITEW6432 = $previousWowArchitecture
    Remove-Item Function:\Invoke-WebRequest -ErrorAction SilentlyContinue
    Remove-Item -Recurse -Force $root
}
