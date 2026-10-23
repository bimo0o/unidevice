param([string]$Binary = "$PSScriptRoot\..\target\release\unidevice-gui.exe", [int]$Seconds = 10)
$ErrorActionPreference = 'Stop'
if ($Seconds -lt 5 -or $Seconds -gt 45) { throw 'Seconds must be 5..45' }
$binaryPath = (Resolve-Path -LiteralPath $Binary).Path
$process = Start-Process -FilePath $binaryPath -ArgumentList '--demo' -WindowStyle Hidden -PassThru
try {
    Start-Sleep -Seconds 3
    $process.Refresh()
    if ($process.HasExited) { throw "GUI exited: $($process.ExitCode)" }
    $cpuStart = $process.TotalProcessorTime.TotalMilliseconds
    $timer = [Diagnostics.Stopwatch]::StartNew()
    $working = @()
    $private = @()
    for ($sample=0; $sample -lt $Seconds; $sample++) {
        Start-Sleep -Seconds 1
        $process.Refresh()
        if ($process.HasExited) { throw 'GUI exited during measurement' }
        $working += $process.WorkingSet64
        $private += $process.PrivateMemorySize64
    }
    $cpu = $process.TotalProcessorTime.TotalMilliseconds - $cpuStart
    $elapsed = $timer.Elapsed.TotalMilliseconds
    [ordered]@{
        mode = 'native GUI demo; startup Hidden; no HID access or interaction'
        warmup_seconds = 3
        duration_seconds = [math]::Round($elapsed/1000,3)
        logical_cpus = [Environment]::ProcessorCount
        cpu_milliseconds = $cpu
        cpu_percent_one_core = [math]::Round(100*$cpu/$elapsed,4)
        cpu_percent_machine = [math]::Round(100*$cpu/$elapsed/[Environment]::ProcessorCount,4)
        working_set_max_bytes = ($working | Measure-Object -Maximum).Maximum
        private_max_bytes = ($private | Measure-Object -Maximum).Maximum
        binary_bytes = (Get-Item -LiteralPath $binaryPath).Length
    } | ConvertTo-Json
} finally {
    $process.Refresh()
    if (-not $process.HasExited) {
        $null=$process.CloseMainWindow()
        if (-not $process.WaitForExit(3000)) { $process.Kill() }
    }
    $process.Dispose()
}
