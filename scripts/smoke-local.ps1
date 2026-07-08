param(
    [int]$Port = 39091
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$targetDir = Join-Path $root "target\smoke"
$binary = Join-Path $targetDir "debug\mirrorproxy.exe"
$config = Join-Path ([System.IO.Path]::GetTempPath()) "mirrorproxy-smoke-$Port.toml"
$publicBaseUrl = "http://127.0.0.1:$Port"
$process = $null

function Assert-Status {
    param(
        [string]$Path,
        [int]$Expected = 200
    )

    $response = Invoke-WebRequest -Uri "$publicBaseUrl$Path" -UseBasicParsing -TimeoutSec 10
    if ($response.StatusCode -ne $Expected) {
        throw "Expected $Path to return $Expected, got $($response.StatusCode)"
    }
    return $response
}

try {
    Push-Location $root
    cargo build --target-dir $targetDir
    if ($LASTEXITCODE -ne 0) {
        throw "cargo build failed with exit code $LASTEXITCODE"
    }

    @"
listen_addr = "127.0.0.1:$Port"
public_base_url = "$publicBaseUrl"
enabled_proxies = ["github", "composer", "oci", "npm", "go", "crates", "pypi"]

[upstreams]
github = "https://github.com"
github_raw = "https://raw.githubusercontent.com"
packagist = "https://repo.packagist.org"
docker_hub = "https://registry-1.docker.io"
ghcr = "https://ghcr.io"
quay = "https://quay.io"
kubernetes = "https://registry.k8s.io"
npm = "https://registry.npmjs.org"
go_proxy = "https://proxy.golang.org"
crates_index = "https://index.crates.io"
crates_api = "https://crates.io"
pypi_simple = "https://pypi.org/simple"
pypi_files = "https://files.pythonhosted.org"

[timeout]
request_secs = 15

[rate_limit]
enabled = false
requests_per_minute = 600
"@ | Set-Content -Path $config -Encoding UTF8

    $process = Start-Process -FilePath $binary -ArgumentList @("--config", $config) -WorkingDirectory $root -WindowStyle Hidden -PassThru
    Start-Sleep -Seconds 1

    $health = Assert-Status "/healthz"
    if ($health.Content -notlike '*"status":"ok"*') {
        throw "healthz did not return ok status"
    }

    $publicConfig = Assert-Status "/api/config"
    if ($publicConfig.Content -notlike '*"pypi"*') {
        throw "public config does not include enabled proxies"
    }

    $rootResponse = Assert-Status "/"
    if ($rootResponse.Content -notlike '*id="root"*') {
        throw "embedded web app root was not served"
    }

    $null = Assert-Status "/v2/"
    $null = Assert-Status "/goproxy/"

    $cratesConfig = Assert-Status "/crates-index/config.json"
    if ($cratesConfig.Content -notlike "*/crates/api/v1/crates*") {
        throw "crates sparse config did not include local download URL"
    }

    Write-Host "MirrorProxy smoke test passed on $publicBaseUrl"
}
finally {
    if ($process -and -not $process.HasExited) {
        Stop-Process -Id $process.Id -Force
    }
    Remove-Item -Path $config -ErrorAction SilentlyContinue
    Pop-Location
}
