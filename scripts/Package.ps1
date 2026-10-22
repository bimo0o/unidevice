$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $projectRoot
if (Test-Path -LiteralPath 'packaging\DISTRIBUTION_HOLD') {
    throw 'Distribution suspended: unresolved antivirus detection. See SECURITY_REVIEW.md.'
}
$packages = @(
    @{ Name = 'windows-x64'; Source = 'target\release\unidevice.exe'; Binary = 'unidevice.exe' },
    @{ Name = 'linux-x64-musl'; Source = 'target\x86_64-unknown-linux-musl\release\unidevice'; Binary = 'unidevice' }
)
New-Item -ItemType Directory -Force -Path 'dist' | Out-Null
$hashes = @()
foreach ($package in $packages) {
    if (-not (Test-Path -LiteralPath $package.Source)) { throw "Missing build: $($package.Source)" }
    $folder = Join-Path $projectRoot "dist\unidevice-$($package.Name)"
    New-Item -ItemType Directory -Force -Path $folder | Out-Null
    Copy-Item -LiteralPath $package.Source -Destination (Join-Path $folder $package.Binary)
    foreach ($name in @('README.md', 'REPORT.md', 'LICENSE', 'THIRD_PARTY.md', 'profiles', 'specs', 'licenses')) {
        Copy-Item -LiteralPath (Join-Path $projectRoot $name) -Destination $folder -Recurse -Force
    }
    if ($package.Name -eq 'windows-x64') { Copy-Item -LiteralPath 'packaging\Demo.cmd' -Destination $folder -Force }
    else { Copy-Item -LiteralPath 'packaging\70-unidevice.rules.example' -Destination $folder -Force }
    $archive = "$folder.zip"
    Compress-Archive -Path "$folder\*" -DestinationPath $archive -Force
    $hashes += "$((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant())  $([IO.Path]::GetFileName($archive))"
}
$hashes | Set-Content -LiteralPath 'dist\SHA256SUMS' -Encoding ascii
Get-Item 'dist\*.zip' | Select-Object Name, Length
