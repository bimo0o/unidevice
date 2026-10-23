param([string]$Destination = "$PSScriptRoot/../dist/unidevice-gui")
$ErrorActionPreference='Stop'
$projectRoot=Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $projectRoot
$suffix=if ($IsWindows -or $env:OS -eq 'Windows_NT') {'.exe'} else {''}
New-Item -ItemType Directory -Force -Path $Destination|Out-Null
foreach ($name in @("unidevice$suffix","unidevice-gui$suffix")) {
    $source=Join-Path $projectRoot "target/release/$name"
    if (-not (Test-Path -LiteralPath $source)) {throw "Missing binary: $name"}
    Copy-Item -LiteralPath $source -Destination (Join-Path $Destination $name) -Force
}
foreach ($name in @('README.md','REPORT.md','ROADMAP.md','LICENSE','THIRD_PARTY.md','SECURITY_REVIEW.md')) {
    Copy-Item -LiteralPath (Join-Path $projectRoot $name) -Destination $Destination -Force
}
foreach ($name in @('profiles','effects','compatibility','reports','specs','licenses','assets','research')) {
    $sourceFolder=Join-Path $projectRoot $name
    if (-not (Test-Path -LiteralPath $sourceFolder)) {continue}
    foreach ($file in (Get-ChildItem -LiteralPath $sourceFolder -File -Recurse)) {
        $relative=$file.FullName.Substring($projectRoot.Length+1)
        $target=Join-Path $Destination $relative
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $target)|Out-Null
        Copy-Item -LiteralPath $file.FullName -Destination $target -Force
    }
}
if ($suffix) {Copy-Item -LiteralPath 'packaging/GUI-Demo.cmd' -Destination (Join-Path $Destination 'Demo.cmd') -Force}
else {Copy-Item -LiteralPath 'packaging/70-unidevice.rules.example' -Destination $Destination -Force}
