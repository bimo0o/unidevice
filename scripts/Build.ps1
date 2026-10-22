param([switch]$LinuxMusl)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $projectRoot
if (Test-Path -LiteralPath "$projectRoot\.tools\cargo\bin\cargo.exe") {
    $env:CARGO_HOME = "$projectRoot\.tools\cargo"
    $env:RUSTUP_HOME = "$projectRoot\.tools\rustup"
    $env:PATH = "$projectRoot\.tools\cargo\bin;" + $env:PATH
}
$env:CARGO_INCREMENTAL = '0'
if ($LinuxMusl) {
    if ($IsWindows -or $env:OS -eq 'Windows_NT') {
        $sysroot = (& rustc --print sysroot).Trim()
        $rustTargetHost = ((& rustc -vV | Select-String '^host:') -replace '^host: ', '').Trim()
        $env:CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER = "$sysroot\lib\rustlib\$rustTargetHost\bin\rust-lld.exe"
        $env:CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_RUSTFLAGS = '-C linker-flavor=ld.lld -C link-self-contained=yes'
    }
    & cargo build --locked --release --target x86_64-unknown-linux-musl
} else {
    & cargo build --locked --release
}
if ($LASTEXITCODE -ne 0) { throw 'Cargo build failed' }
