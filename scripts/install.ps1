[CmdletBinding()]
param(
    [string]$Mirror = $env:MIRRORPROXY_DOWNLOAD_MIRROR,
    [string]$Version = $(if ($env:MIRRORPROXY_VERSION) { $env:MIRRORPROXY_VERSION } else { "latest" }),
    [string]$InstallDir = $(if ($env:MIRRORPROXY_INSTALL_DIR) { $env:MIRRORPROXY_INSTALL_DIR } else { Join-Path $env:LOCALAPPDATA "Programs\MirrorProxy\bin" }),
    [string]$Repository = $(if ($env:MIRRORPROXY_GITHUB_REPO) { $env:MIRRORPROXY_GITHUB_REPO } else { "inbjo/MirrorProxy" })
)

$ErrorActionPreference = "Stop"

$architecture = [Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()
if ($architecture -ne "X64") {
    throw "Unsupported Windows architecture: $architecture. Stable releases currently provide Windows x64 only."
}

$archive = "mirrorproxy-client-x86_64-pc-windows-msvc.zip"
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
    Write-Host "Downloading MirrorProxy $Version for Windows x64..."
    try {
        Invoke-WebRequest -UseBasicParsing -Uri (Get-DownloadUrl "$releaseUrl/$archive") -OutFile $archivePath
    } catch {
        throw "No stable release asset was found. Publish a v* release or set MIRRORPROXY_VERSION to a release tag. $($_.Exception.Message)"
    }
    Invoke-WebRequest -UseBasicParsing -Uri (Get-DownloadUrl "$releaseUrl/$archive.sha256") -OutFile $checksumPath

    $expected = ((Get-Content -Path $checksumPath -TotalCount 1) -split '\s+')[0].ToLowerInvariant()
    $actual = (Get-FileHash -Algorithm SHA256 -Path $archivePath).Hash.ToLowerInvariant()
    if ($expected -ne $actual) { throw "Checksum verification failed." }

    Expand-Archive -Path $archivePath -DestinationPath $tempDir -Force
    $binary = Join-Path $tempDir "mirrorproxy.exe"
    if (-not (Test-Path $binary)) { throw "Archive does not contain mirrorproxy.exe." }

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
} finally {
    if (Test-Path $tempDir) { Remove-Item -Recurse -Force $tempDir }
}
