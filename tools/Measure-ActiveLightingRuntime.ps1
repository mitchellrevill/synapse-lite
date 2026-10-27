param(
    [switch]$WriteUniformFrames,
    [Parameter(Mandatory=$true)][string]$Executable,
    [Parameter(Mandatory=$true)][string]$ExpectedSha256,
    [Parameter(Mandatory=$true)][string]$ConfigFile,
    [Parameter(Mandatory=$true)][string]$Assets,
    [ValidateSet('keyboard-lighting','keyboard-lighting-color-editor')][string]$Scenario = 'keyboard-lighting',
    [ValidateRange(8,600)][int]$Seconds = 15,
    [ValidateRange(1,3)][int]$Runs = 3,
    [Parameter(Mandatory=$true)][string]$Output
)
$ErrorActionPreference = 'Stop'
if (!$WriteUniformFrames) { throw 'Explicit -WriteUniformFrames is required; this workload sends RGB frames' }
if (Test-Path Env:SECBIRD_SIM) { throw 'Active physical measurement refuses simulator configuration' }
$binary = (Resolve-Path -LiteralPath $Executable).Path
if ((Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash -ne $ExpectedSha256) { throw 'Executable hash differs from the reviewed release build' }
$config = (Resolve-Path -LiteralPath $ConfigFile).Path
$seedSha256 = (Get-FileHash -LiteralPath $config -Algorithm SHA256).Hash
$assetsPath = (Resolve-Path -LiteralPath $Assets).Path
$outputPath = [IO.Path]::GetFullPath($Output)
$scratch = Join-Path (Split-Path -Parent $outputPath) ('active-runtime-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $scratch | Out-Null
$names = @('SYNAPSE_LITE_CONFIG_DIR','SYNAPSE_LITE_SMOKE_VIEW','SYNAPSE_LITE_ASSETS','SYNAPSE_LITE_SMOKE_REPORT','SYNAPSE_LITE_PROBE_REPORT')
$previous = @{}
foreach ($name in $names) { $previous[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
$results = @()
$failure = $null
try {
    $env:SYNAPSE_LITE_ASSETS = $assetsPath
    $env:SYNAPSE_LITE_SMOKE_VIEW = $Scenario
    for ($run = 1; $run -le $Runs; $run++) {
        $directory = Join-Path $scratch "run-$run"
        New-Item -ItemType Directory -Path $directory | Out-Null
        Copy-Item -LiteralPath $config -Destination (Join-Path $directory 'profiles.json')
        if ((Get-FileHash -LiteralPath (Join-Path $directory 'profiles.json') -Algorithm SHA256).Hash -ne $seedSha256) { throw 'The approved seed configuration changed between runs' }
        $env:SYNAPSE_LITE_CONFIG_DIR = $directory
        $env:SYNAPSE_LITE_SMOKE_REPORT = Join-Path $directory 'frame.json'
        $env:SYNAPSE_LITE_PROBE_REPORT = Join-Path $directory 'worker.json'
        $process = Start-Process -FilePath $binary -ArgumentList @('--runtime-lighting-probe', $Seconds.ToString()) -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $directory 'stdout.private.txt') -RedirectStandardError (Join-Path $directory 'stderr.private.txt')
        $clock = [Diagnostics.Stopwatch]::StartNew()
        $samples = @()
        $children = @()
        $lastDiscovery = -1.0
        $peakWorking = 0L
        $peakPaged = 0L
        $tcp = @()
        try {
            while (!$process.HasExited) {
                if ($clock.Elapsed.TotalSeconds -gt $Seconds + 30) { throw 'Active runtime exceeded its bounded probe interval' }
                if ($clock.Elapsed.TotalSeconds - $lastDiscovery -ge 1) {
                    $tree = @(Get-CimInstance Win32_Process | Select-Object ProcessId,ParentProcessId)
                    $ids = @($process.Id)
                    do {
                        $new = @($tree | Where-Object { $_.ParentProcessId -in $ids -and $_.ProcessId -notin $ids } | Select-Object -ExpandProperty ProcessId)
                        $ids += $new
                    } while ($new.Count -gt 0)
                    $children = @($ids | Where-Object { $_ -ne $process.Id })
                    $lastDiscovery = $clock.Elapsed.TotalSeconds
                }
                $process.Refresh()
                if ($process.HasExited) { break }
                $members = @($process)
                foreach ($childId in $children) {
                    $child = Get-Process -Id $childId -ErrorAction SilentlyContinue
                    if ($null -ne $child) { $members += $child }
                }
                $peakWorking = [Math]::Max($peakWorking, ($members | Measure-Object PeakWorkingSet64 -Sum).Sum)
                $peakPaged = [Math]::Max($peakPaged, ($members | Measure-Object PeakPagedMemorySize64 -Sum).Sum)
                $samples += [pscustomobject]@{
                    ElapsedSeconds = $clock.Elapsed.TotalSeconds
                    WorkingSetBytes = ($members | Measure-Object WorkingSet64 -Sum).Sum
                    PrivateBytes = ($members | Measure-Object PrivateMemorySize64 -Sum).Sum
                    Processes = $members.Count
                    CpuSeconds = ($members | ForEach-Object { $_.TotalProcessorTime.TotalSeconds } | Measure-Object -Sum).Sum
                }
                if ($clock.Elapsed.TotalSeconds -gt 4 -and $tcp.Count -eq 0) {
                    foreach ($member in $members) {
                        $tcp += @(Get-NetTCPConnection -OwningProcess $member.Id -ErrorAction SilentlyContinue | Select-Object State,LocalPort,RemotePort)
                    }
                }
                Start-Sleep -Milliseconds 200
            }
            $process.WaitForExit()
            if ($process.ExitCode -ne 0) { throw "Active runtime exited with code $($process.ExitCode); inspect private diagnostics" }
        } finally {
            if (!$process.HasExited) { $process.Kill(); $process.WaitForExit(3000) | Out-Null }
        }
        if ($samples.Count -lt 5) { throw 'Too few samples to cover the workload' }
        $frame = Get-Content -LiteralPath $env:SYNAPSE_LITE_SMOKE_REPORT -Raw | ConvertFrom-Json
        $worker = Get-Content -LiteralPath $env:SYNAPSE_LITE_PROBE_REPORT -Raw | ConvertFrom-Json
        if (!$frame.matched -or !$frame.presented -or !$frame.visible -or $frame.failed -or !$frame.active_native_runtime.ready) { throw 'The requested native view and active worker were not both verified' }
        if (!$worker.ready_before_stop -or !$worker.worker_was_running -or $worker.accepted_frames -lt 3 -or $null -ne $worker.last_error -or !$worker.worker_stopped -or !$worker.clean_stop -or $worker.restoration_failure_count -ne 0 -or $null -eq $worker.status_snapshot_age_ms -or $worker.status_snapshot_age_ms -gt 2500) { throw 'Active worker or bounded shutdown validation failed' }
        if ((Get-FileHash -LiteralPath (Join-Path $directory 'profiles.json') -Algorithm SHA256).Hash -ne $seedSha256) { throw 'Probe changed its approved native configuration' }
        $privatePeak = ($samples | Measure-Object PrivateBytes -Maximum).Maximum
        $results += [pscustomobject]@{
            Run = $run
            Scenario = $Scenario
            PresentedFrame = $frame
            ActiveWorker = $worker
            PeakWorkingSetBytes = $peakWorking
            SampledPeakPrivateBytes = $privatePeak
            PeakPagedMemoryBytes = $peakPaged
            MaxObservedProcesses = ($samples | Measure-Object Processes -Maximum).Maximum
            ObservedTcpConnections = $tcp
            Under100MB = ($peakWorking -lt 100000000 -and $privatePeak -lt 100000000 -and $peakPaged -lt 100000000)
            Samples = $samples
        }
    }
} catch {
    $failure = $_.Exception.Message
} finally {
    foreach ($name in $names) { [Environment]::SetEnvironmentVariable($name, $previous[$name], 'Process') }
}
[pscustomobject]@{
    BinarySha256 = $ExpectedSha256
    TimestampUtc = [DateTimeOffset]::UtcNow.ToString('o')
    LimitBytes = 100000000
    Scope = 'Explicit lighting-only native probe: actual tray, native editor, uniform worker and observed descendants. Existing stock services/kernel drivers excluded; unrelated apply/actions disabled. Not final feature, input, optical or RAM acceptance.'
    MetricNote = 'Sum of observed process working-set high-waters and sampled private committed bytes. Shared pages can be counted more than once. Descendants discovered once per second; short-lived processes and UDP/DNS traffic are not covered. Peak paged memory is an additional conservative metric.'
    ConfigurationUnchanged = $(if ($failure) { $null } else { $true })
    Failure = $failure
    Runs = $results
} | ConvertTo-Json -Depth 16 | Set-Content -LiteralPath $outputPath -Encoding utf8
$results | Select-Object Run,Scenario,PeakWorkingSetBytes,SampledPeakPrivateBytes,MaxObservedProcesses,Under100MB | Format-Table
if ($failure) { throw $failure }
if (@($results | Where-Object { !$_.Under100MB }).Count) { throw 'Active workload exceeded the memory threshold' }
