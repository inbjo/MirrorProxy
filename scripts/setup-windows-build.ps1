# Complete Windows Perl for vendored OpenSSL; x64 tools run under Windows ARM64 emulation.
$ErrorActionPreference = 'Stop'
$archive = Join-Path $env:RUNNER_TEMP 'strawberry-perl-5.42.3.1-portable.zip'
$destination = Join-Path $env:RUNNER_TEMP 'mirrorproxy-build-perl'
$url = 'https://github.com/StrawberryPerl/Perl-Dist-Strawberry/releases/download/SP_54231_64bit/strawberry-perl-5.42.3.1-64bit-portable.zip'
$expected = '6a081a811781c30aca51dbc036afd93092af91e3297901f02c17043795a10690'
Invoke-WebRequest $url -OutFile $archive
if ((Get-FileHash -Algorithm SHA256 $archive).Hash.ToLowerInvariant() -ne $expected) {
    throw 'Windows build Perl archive checksum mismatch'
}
Expand-Archive $archive -DestinationPath $destination
$perlBin = Join-Path $destination 'perl/bin'
$compilerBin = Join-Path $destination 'c/bin'
$env:PATH = "$perlBin;$compilerBin;$env:PATH"
& (Join-Path $perlBin 'perl.exe') -MIPC::Cmd -MLocale::Maketext::Simple -MWin32 -e 'print "$^X\n"'
if ($LASTEXITCODE -ne 0) { throw 'Windows Perl prerequisite modules are missing' }
$compilerBin | Out-File -FilePath $env:GITHUB_PATH -Encoding utf8 -Append
$perlBin | Out-File -FilePath $env:GITHUB_PATH -Encoding utf8 -Append
