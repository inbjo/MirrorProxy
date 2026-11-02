[CmdletBinding()]
param(
    [string]$Mirror = $env:MIRRORPROXY_DOWNLOAD_MIRROR,
    [string]$Version = $(if ($env:MIRRORPROXY_VERSION) { $env:MIRRORPROXY_VERSION } else { "latest" }),
    [string]$InstallDir = $(if ($env:MIRRORPROXY_INSTALL_DIR) { $env:MIRRORPROXY_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA "Programs\MirrorProxy\bin" }),
    [string]$Repository = $(if ($env:MIRRORPROXY_GITHUB_REPO) { $env:MIRRORPROXY_GITHUB_REPO } else { "inbjo/MirrorProxy" }),
    [switch]$Setup,
    [string]$BaseUrl
)

$ErrorActionPreference = "Stop"
if ($Setup -and [string]::IsNullOrWhiteSpace($BaseUrl)) { throw "-Setup requires -BaseUrl." }
if (-not $Setup -and -not [string]::IsNullOrWhiteSpace($BaseUrl)) { throw "-BaseUrl requires -Setup." }

$architecture = if ($env:PROCESSOR_ARCHITEW6432) { $env:PROCESSOR_ARCHITEW6432 } else { $env:PROCESSOR_ARCHITECTURE }
if ([string]::IsNullOrWhiteSpace($architecture)) {
    try { $architecture = [Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString() }
    catch { throw "Unable to determine Windows architecture. Run on Windows x64 or ARM64 with Windows PowerShell 5.1 or PowerShell 7." }
}
$target = switch ($architecture) {
    { $_ -in @('AMD64', 'X64') } { 'x86_64-pc-windows-msvc' }
    'ARM64' { 'aarch64-pc-windows-msvc' }
    default { throw "Unsupported Windows architecture: $architecture. Windows x64 and ARM64 are supported." }
}

$archive = "mirrorproxy-client-$target.zip"
if ($Version -eq "latest") {
    $releaseUrl = "https://github.com/$Repository/releases/latest/download"
} else {
    $releaseUrl = "https://github.com/$Repository/releases/download/$Version"
}

function Get-DownloadUrl([string]$Url) {
    if ([string]::IsNullOrWhiteSpace($Mirror)) { return $Url }
    return "$($Mirror.TrimEnd('/'))/$Url"
}

$tempDir = Join-Path ([IO.Path]::GetTempPath()) ("mirrorproxy-install-" + [guid]::NewGuid().ToString("N"))
$archivePath = Join-Path $tempDir $archive
$checksumPath = "$archivePath.sha256"

try {
    New-Item -ItemType Directory -Path $tempDir | Out-Null
    Write-Host "Downloading MirrorProxy $Version for $target..."
    try {
        Invoke-WebRequest -UseBasicParsing -Uri (Get-DownloadUrl "$releaseUrl/$archive") -OutFile $archivePath
    } catch {
        throw "Client archive download failed. Check the network, TLS or HTTP error below. If GitHub is unreachable, pass -Mirror https://YOUR-INSTANCE; proxying the script URL does not proxy subsequent downloads. For HTTP 404, check the release tag and asset name. $($_.Exception.Message)"
    }
    Invoke-WebRequest -UseBasicParsing -Uri (Get-DownloadUrl "$releaseUrl/$archive.sha256") -OutFile $checksumPath

    $checksumLine = Get-Content -Path $checksumPath -TotalCount 1
    if ([string]::IsNullOrWhiteSpace($checksumLine) -or $checksumLine -notmatch '^\s*([a-fA-F0-9]{64})(?:\s|$)') {
        throw "Downloaded checksum file is empty or invalid. Check the mirror URL and retry."
    }
    $expected = $Matches[1].ToLowerInvariant()
    $hashResult = Get-FileHash -Algorithm SHA256 -Path $archivePath
    if (-not $hashResult -or [string]::IsNullOrWhiteSpace($hashResult.Hash)) { throw "Could not calculate the client archive checksum." }
    $actual = $hashResult.Hash.ToLowerInvariant()
    if ($expected -ne $actual) { throw "Checksum verification failed." }

    Expand-Archive -Path $archivePath -DestinationPath $tempDir -Force
    $binary = Join-Path $tempDir "mirrorproxy.exe"
    if (-not (Test-Path $binary)) { throw "Archive does not contain mirrorproxy.exe." }

    if ($Setup) {
        & $binary setup --help *> $null
        if ($LASTEXITCODE -ne 0) { throw "This client release does not support setup. Publish a stable release containing setup, or select one with -Version. Existing client was not replaced." }
    }

    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    Copy-Item -Force -Path $binary -Destination (Join-Path $InstallDir "mirrorproxy.exe")

    $userPath = [Environment]::GetEnvironmentVariable("Path", "User")
    $pathEntries = @($userPath -split ';' | Where-Object { $_ })
    if ($pathEntries -notcontains $InstallDir) {
        $newPath = (@($InstallDir) + $pathEntries) -join ';'
        [Environment]::SetEnvironmentVariable("Path", $newPath, "User")
        Write-Host "Added $InstallDir to your user PATH. Open a new terminal to use it."
    }
    $env:Path = "$InstallDir;$env:Path"
    Write-Host "MirrorProxy installed to $(Join-Path $InstallDir 'mirrorproxy.exe')"
    if ($Setup) {
        $previousInputEncoding = [Console]::InputEncoding
        $previousOutputEncoding = [Console]::OutputEncoding
        try {
            # The client writes UTF-8 prompts directly to CONOUT$. Windows
            # PowerShell 5.1 commonly starts with an OEM console code page.
            [Console]::InputEncoding = [Text.UTF8Encoding]::new($false)
            [Console]::OutputEncoding = [Text.UTF8Encoding]::new($false)
            & (Join-Path $InstallDir 'mirrorproxy.exe') setup --base-url $BaseUrl
            if ($LASTEXITCODE -ne 0) { throw "MirrorProxy setup did not complete. Client remains installed; retry setup --base-url with this instance." }
        } finally {
            [Console]::InputEncoding = $previousInputEncoding
            [Console]::OutputEncoding = $previousOutputEncoding
        }
    }
} catch {
    throw "MirrorProxy installer failed at line $($_.InvocationInfo.ScriptLineNumber): $($_.Exception.Message)"
} finally {
    if (Test-Path $tempDir) { Remove-Item -Recurse -Force $tempDir }
}
