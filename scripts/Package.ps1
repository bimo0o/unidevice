$ErrorActionPreference='Stop'
$projectRoot=Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $projectRoot
$folder=Join-Path $projectRoot 'dist/unidevice-windows-x64'
& "$PSScriptRoot/Stage-Gui.ps1" -Destination $folder
$archive=Join-Path $projectRoot 'dist/unidevice-windows-x64.zip'
Compress-Archive -Path "$folder/*" -DestinationPath $archive -Force
$lines=@()
foreach ($file in @($archive,(Join-Path $folder 'unidevice-gui.exe'),(Join-Path $folder 'unidevice.exe'))) {
    $hash=(Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant()
    $relative=$file.Substring((Join-Path $projectRoot 'dist').Length+1).Replace('\','/')
    $lines+="$hash  $relative"
}
$lines|Set-Content -LiteralPath 'dist/SHA256SUMS' -Encoding ascii
Get-Item -LiteralPath $archive|Select-Object FullName,Length
