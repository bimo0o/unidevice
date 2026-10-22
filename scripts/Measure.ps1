param([string]$Binary = "$PSScriptRoot\..\target\release\unidevice.exe", [int]$Seconds = 10)
$ErrorActionPreference = 'Stop'
if ($Seconds -lt 5 -or $Seconds -gt 45) { throw 'Seconds must be 5..45' }
$resolvedBinary = (Resolve-Path -LiteralPath $Binary).Path
$startInfo = [System.Diagnostics.ProcessStartInfo]::new()
$startInfo.FileName = $resolvedBinary
$startInfo.UseShellExecute = $false
$startInfo.CreateNoWindow = $true
$startInfo.RedirectStandardInput = $true
$startInfo.RedirectStandardOutput = $true
$startInfo.RedirectStandardError = $true
$process = [System.Diagnostics.Process]::new()
$process.StartInfo = $startInfo
try {
    if (-not $process.Start()) { throw 'Process did not start' }
    Start-Sleep -Seconds 2
    $process.Refresh()
    if ($process.HasExited) { throw 'Process exited before measurement' }
    $cpuStart = $process.TotalProcessorTime.TotalMilliseconds
    $timer = [Diagnostics.Stopwatch]::StartNew()
    $workingSet = @()
    $privateBytes = @()
    for ($sample = 0; $sample -lt $Seconds; $sample++) {
        Start-Sleep -Seconds 1
        $process.Refresh()
        if ($process.HasExited) { throw 'Process exited during measurement' }
        $workingSet += $process.WorkingSet64
        $privateBytes += $process.PrivateMemorySize64
    }
    $elapsed = $timer.Elapsed.TotalMilliseconds
    $cpuMs = $process.TotalProcessorTime.TotalMilliseconds - $cpuStart
    $process.StandardInput.WriteLine('0')
    if (-not $process.WaitForExit(3000)) { throw 'Console did not exit' }
    [ordered]@{
        platform = [Environment]::OSVersion.VersionString
        logical_cpus = [Environment]::ProcessorCount
        mode = 'interactive menu blocked on stdin; no devices opened'
        duration_seconds = [math]::Round($elapsed / 1000, 3)
        cpu_milliseconds = $cpuMs
        cpu_percent_one_core = [math]::Round(100 * $cpuMs / $elapsed, 4)
        cpu_percent_machine = [math]::Round(100 * $cpuMs / $elapsed / [Environment]::ProcessorCount, 4)
        working_set_max_bytes = ($workingSet | Measure-Object -Maximum).Maximum
        private_max_bytes = ($privateBytes | Measure-Object -Maximum).Maximum
        binary_bytes = (Get-Item -LiteralPath $resolvedBinary).Length
    } | ConvertTo-Json
} finally {
    if ($process.Id -and -not $process.HasExited) { $process.Kill() }
    $process.Dispose()
}
