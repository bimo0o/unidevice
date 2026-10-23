$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $projectRoot
if (Test-Path -LiteralPath "$projectRoot\.tools\cargo\bin\cargo.exe") {
    $env:CARGO_HOME="$projectRoot\.tools\cargo"
    $env:RUSTUP_HOME="$projectRoot\.tools\rustup"
    $env:PATH="$projectRoot\.tools\cargo\bin;"+$env:PATH
}
$json = & cargo metadata --offline --features gui --filter-platform x86_64-pc-windows-gnu --format-version 1
if ($LASTEXITCODE -ne 0) { throw 'Cargo metadata failed; fetch dependencies first' }
$metadata=$json|ConvertFrom-Json
$coreJson=& cargo metadata --offline --filter-platform x86_64-unknown-linux-musl --format-version 1
if ($LASTEXITCODE -ne 0) { throw 'Core metadata failed' }
$core=$coreJson|ConvertFrom-Json
$allPackages=@(@($metadata.packages)+@($core.packages)|Where-Object source|Sort-Object name,version -Unique)
$notice=@('# Third-party dependencies','','Application code: MIT. External dependencies retain their own licenses.','No manufacturer SDK or proprietary vendor binary is included.','','| Package | Version | Declared license | Source |','|---|---|---|---|')
foreach ($package in $allPackages) {
    $sourceFolder=Split-Path -Parent $package.manifest_path
    $targetFolder=Join-Path $projectRoot "licenses\$($package.name)-$($package.version)"
    $files=Get-ChildItem -LiteralPath $sourceFolder -File -Recurse | Where-Object { ($_.Name -match '(LICENSE|LICENCE|COPYING|COPYRIGHT|NOTICE|^OFL|^UFL)' -or $_.Name -eq 'Hack-Regular.txt') -and $_.Extension -ne '.rs' -and $_.Name -notmatch 'gpl3' }
    foreach ($file in $files) {
        $relative=$file.FullName.Substring($sourceFolder.Length+1)
        $destination=Join-Path $targetFolder $relative
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $destination)|Out-Null
        Copy-Item -LiteralPath $file.FullName -Destination $destination -Force
    }
    $notice+="| $($package.name) | $($package.version) | $($package.license) | https://crates.io/crates/$($package.name)/$($package.version) |"
}
$notice+=@('','License texts are in licenses/. Build-time and target-specific packages are listed too.','HIDAPI C uses its BSD option; its wrapper uses MIT. No GPL driver implementation is copied.','The Corsair research note has its separately marked CC BY-SA 4.0 attribution.')
$notice|Set-Content -LiteralPath THIRD_PARTY.md -Encoding utf8
Write-Output "Collected notices for $($allPackages.Count) packages."
