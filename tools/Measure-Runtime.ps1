param(
    [string]$Executable = "$PSScriptRoot\..\target\release\secbird-tray.exe",
    [string]$Assets = "$PSScriptRoot\..\target\native-assets",
    [int]$Seconds = 12,
    [string[]]$Scenarios = @('dashboard', 'mouse-performance', 'keyboard-lighting'),
    [string]$Output = "$PSScriptRoot\..\target\runtime-memory.json"
)
$ErrorActionPreference = 'Stop'
if ($Seconds -lt 5 -or $Seconds -gt 600) { throw 'Seconds must be 5..600' }
$binary = (Resolve-Path -LiteralPath $Executable).Path
$assetsPath = (Resolve-Path -LiteralPath $Assets).Path
$outputPath = [IO.Path]::GetFullPath($Output)
$scratch = Join-Path (Split-Path -Parent $outputPath) 'runtime-scratch'
New-Item -ItemType Directory -Force $scratch | Out-Null
$original = @{}
foreach ($key in @('SYNAPSE_LITE_CONFIG_DIR', 'SYNAPSE_LITE_SMOKE_VIEW', 'SYNAPSE_LITE_ASSETS', 'SYNAPSE_LITE_SMOKE_REPORT')) {
    $original[$key] = [Environment]::GetEnvironmentVariable($key, 'Process')
}
$results = @()
try {
    $env:SYNAPSE_LITE_ASSETS = $assetsPath
    foreach ($scenario in $Scenarios) {
        $env:SYNAPSE_LITE_CONFIG_DIR = Join-Path $scratch $scenario
        $env:SYNAPSE_LITE_SMOKE_VIEW = $scenario
        $frameReport = Join-Path $scratch "$scenario-$([guid]::NewGuid().ToString('N')).json"
        $env:SYNAPSE_LITE_SMOKE_REPORT = $frameReport
        $process = Start-Process -FilePath $binary -ArgumentList @('--runtime-smoke', $Seconds.ToString()) -WindowStyle Hidden -PassThru
        $processStartedUtc = $process.StartTime.ToUniversalTime()
        $clock = [Diagnostics.Stopwatch]::StartNew()
        $samples = @()
        $knownChildren = @()
        $lastDiscovery = -1.0
        $peakWorking = 0L
        $peakPaged = 0L
        $tcp = @()
        try {
            while (-not $process.HasExited) {
                if ($clock.Elapsed.TotalSeconds -gt $Seconds + 60) { throw 'Runtime failed to exit within its bounded smoke interval' }
                if ($clock.Elapsed.TotalSeconds - $lastDiscovery -ge 1) {
                    $tree = @(Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId)
                    $ids = @($process.Id)
                    do {
                        $new = @($tree | Where-Object { $_.ParentProcessId -in $ids -and $_.ProcessId -notin $ids } | Select-Object -ExpandProperty ProcessId)
                        $ids += $new
                    } while ($new.Count -gt 0)
                    $knownChildren = @($ids | Where-Object { $_ -ne $process.Id })
                    $lastDiscovery = $clock.Elapsed.TotalSeconds
                }
                $process.Refresh()
                if ($process.HasExited) { break }
                $members = @($process)
                foreach ($childId in $knownChildren) {
                    $child = Get-Process -Id $childId -ErrorAction SilentlyContinue
                    if ($null -ne $child) { $members += $child }
                }
                $working = ($members | Measure-Object WorkingSet64 -Sum).Sum
                $private = ($members | Measure-Object PrivateMemorySize64 -Sum).Sum
                $peakWorking = [Math]::Max($peakWorking, ($members | Measure-Object PeakWorkingSet64 -Sum).Sum)
                $peakPaged = [Math]::Max($peakPaged, ($members | Measure-Object PeakPagedMemorySize64 -Sum).Sum)
                $samples += [pscustomobject]@{
                    ElapsedSeconds = $clock.Elapsed.TotalSeconds
                    WorkingSetBytes = $working
                    PrivateBytes = $private
                    Processes = $members.Count
                    CpuSeconds = ($members | ForEach-Object { $_.TotalProcessorTime.TotalSeconds } | Measure-Object -Sum).Sum
                }
                if ($clock.Elapsed.TotalSeconds -gt 4 -and $tcp.Count -eq 0) {
                    $tcp = @(Get-NetTCPConnection -OwningProcess $process.Id -ErrorAction SilentlyContinue | Select-Object State,LocalPort,RemotePort)
                }
                Start-Sleep -Milliseconds 200
            }
            $process.WaitForExit()
            if ($process.ExitCode -ne 0) { throw "Runtime exited with code $($process.ExitCode)" }
        } finally {
            if (-not $process.HasExited) { $process.Kill(); $process.WaitForExit() }
        }
        if ($samples.Count -lt 5) { throw 'Too few samples; the application may already be running or startup failed' }
        if (-not (Test-Path -LiteralPath $frameReport)) { throw "No presented-frame evidence was produced for $scenario" }
        $frame = Get-Content -LiteralPath $frameReport -Raw | ConvertFrom-Json
        if (-not $frame.matched -or -not $frame.presented -or -not $frame.visible -or $frame.failed) {
            throw "Requested view $scenario was not successfully presented; this run does not cover it"
        }
        $intrinsicField = $frame.PSObject.Properties['intrinsic_read_only_measurement']
        $sampledPrivate = ($samples | Measure-Object PrivateBytes -Maximum).Maximum
        $sampledWorking = ($samples | Measure-Object WorkingSetBytes -Maximum).Maximum
        $results += [pscustomobject]@{
            Scenario = $scenario
            ProcessStartedUtc = $processStartedUtc.ToString('o')
            StartupToPresentedFrameMilliseconds = ((Get-Item -LiteralPath $frameReport).LastWriteTimeUtc - $processStartedUtc).TotalMilliseconds
            PresentedFrame = $frame
            IntrinsicReadOnlyReported = ($null -ne $intrinsicField -and $intrinsicField.Value -eq $true)
            PeakWorkingSetBytes = $peakWorking
            SampledPeakWorkingSetBytes = $sampledWorking
            SampledPeakPrivateBytes = $sampledPrivate
            PeakPagedMemoryBytes = $peakPaged
            MaxObservedProcesses = ($samples | Measure-Object Processes -Maximum).Maximum
            ObservedTcpConnections = $tcp
            Under100MB = ($peakWorking -lt 100000000 -and $sampledPrivate -lt 100000000 -and $peakPaged -lt 100000000)
            Samples = $samples
        }
    }
} finally {
    foreach ($key in $original.Keys) {
        if ($null -eq $original[$key]) {
            Remove-Item -LiteralPath "Env:$key" -ErrorAction SilentlyContinue
        } else {
            [Environment]::SetEnvironmentVariable($key, $original[$key], 'Process')
        }
    }
}
$intrinsicAllRuns = @($results | Where-Object { -not $_.IntrinsicReadOnlyReported }).Count -eq 0
[pscustomobject]@{
    BinarySha256 = (Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash
    TimestampUtc = [DateTimeOffset]::UtcNow.ToString('o')
    LimitBytes = 100000000
    MetricNote = 'Working set high-water plus sampled private committed bytes. Peak paged memory is recorded as an additional conservative process metric. Shared resident pages may be counted per process.'
    IntrinsicReadOnlyReportedInEveryRun = $intrinsicAllRuns
    Scope = if ($intrinsicAllRuns) { 'Query-only runtime-smoke checkpoint: real tray/native presentation/GET discovery/listeners and observed descendants. Generic profile/policy apply, uniform worker and mutation actions disabled. Stock services/kernel drivers excluded. Not active-lighting, full workflow, visual or hardware parity.' } else { 'Historical/legacy scope: real tray/native presentation/listeners and observed descendants using fresh no-action configuration. Intrinsic read-only enforcement was not reported; --runtime-smoke alone on older binaries did not prevent generic profile/policy apply from existing configuration. Uniform worker and stock services/kernel drivers excluded. Not full workflow or parity.' }
    HistoricalEvidenceNote = 'Earlier binary-hash-scoped reports retain their original conditional no-action scope; this script does not retrofit intrinsic enforcement onto them.'
    Runs = $results
} | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $outputPath -Encoding utf8
$results | Select-Object Scenario,PeakWorkingSetBytes,SampledPeakPrivateBytes,PeakPagedMemoryBytes,MaxObservedProcesses,Under100MB | Format-Table
if (@($results | Where-Object { -not $_.Under100MB }).Count) { throw 'Runtime memory limit exceeded' }
