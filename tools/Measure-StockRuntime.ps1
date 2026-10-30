param(
    [int]$Seconds = 60,
    [int]$IntervalMilliseconds = 1000,
    [string]$Scenario = 'Existing desktop state; no workload automation',
    [string]$Output = "$PSScriptRoot\..\target\stock-runtime.json"
)
$ErrorActionPreference = 'Stop'
if ($Seconds -lt 5 -or $Seconds -gt 600) { throw 'Seconds must be 5..600' }
if ($IntervalMilliseconds -lt 200 -or $IntervalMilliseconds -gt 10000) { throw 'IntervalMilliseconds must be 200..10000' }
$outputPath = [IO.Path]::GetFullPath($Output)
$outputDirectory = Split-Path -Parent $outputPath
New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null

# Process enumeration can be restricted by the host. An empty set is a failed
# measurement, never evidence that stock Synapse uses zero memory.
function Read-StockProcesses {
    @(Get-Process | Where-Object {
        ($_.ProcessName -match '^(Razer|RzSynapse|GameManagerService|Synapse)' -or
        $_.Id -in $serviceProcessIds) -and $_.ProcessName -notmatch '^synapse-lite$'
    })
}
$services = @(Get-CimInstance Win32_Service | Where-Object {
    $_.Name -match '(Razer|RzSynapse|GameManager)' -or $_.DisplayName -match '(Razer|Synapse)'
} | Select-Object Name,State,StartMode,ProcessId)
$serviceProcessIds = @($services | Where-Object { $_.ProcessId -gt 0 } | Select-Object -ExpandProperty ProcessId)
if (@(Read-StockProcesses).Count -eq 0) { throw 'No stock processes visible; check the execution context' }
$logicalProcessors = [Environment]::ProcessorCount
$previous = @{}
$samples = [Collections.Generic.List[object]]::new()
$clock = [Diagnostics.Stopwatch]::StartNew()
while ($clock.Elapsed.TotalSeconds -lt $Seconds) {
    $rows = [Collections.Generic.List[object]]::new()
    foreach ($process in Read-StockProcesses) {
        try {
            $process.Refresh()
            $working = $process.WorkingSet64
            $private = $process.PrivateMemorySize64
        } catch { continue } # A process that exits mid-sample is not a zero row.
        $cpuSeconds = $null
        try { $cpuSeconds = $process.TotalProcessorTime.TotalSeconds } catch {}
        $sampleTime = $clock.Elapsed.TotalSeconds
        $percent = $null
        if ($null -ne $cpuSeconds) {
            $old = $previous[$process.Id]
            if ($null -ne $old -and $cpuSeconds -ge $old.Cpu -and $sampleTime -gt $old.Time) {
                $percent = 100 * ($cpuSeconds - $old.Cpu) / ($sampleTime - $old.Time)
            }
            $previous[$process.Id] = @{ Cpu = $cpuSeconds; Time = $sampleTime }
        }
        $rows.Add([pscustomobject]@{
            Name = $process.ProcessName
            Pid = $process.Id
            WorkingSetBytes = $working
            PrivateBytes = $private
            CpuPercentOfOneCore = $percent
        })
    }
    if ($rows.Count -eq 0) { throw 'Stock processes disappeared during measurement' }
    $cpuRows = @($rows | Where-Object { $null -ne $_.CpuPercentOfOneCore })
    $samples.Add([pscustomobject]@{
        TimestampUtc = [DateTimeOffset]::UtcNow.ToString('o')
        ElapsedSeconds = $clock.Elapsed.TotalSeconds
        ProcessCount = $rows.Count
        WorkingSetBytes = ($rows | Measure-Object WorkingSetBytes -Sum).Sum
        PrivateBytes = ($rows | Measure-Object PrivateBytes -Sum).Sum
        CpuPercentOfOneCore = if ($cpuRows.Count) { ($cpuRows | Measure-Object CpuPercentOfOneCore -Sum).Sum } else { $null }
        CpuMeasuredProcesses = $cpuRows.Count
        Processes = @($rows)
    })
    Start-Sleep -Milliseconds $IntervalMilliseconds
}
$cpuSamples = @($samples | Where-Object { $null -ne $_.CpuPercentOfOneCore })
$summary = [pscustomobject]@{
    Samples = $samples.Count
    AverageWorkingSetBytes = ($samples | Measure-Object WorkingSetBytes -Average).Average
    PeakSampledWorkingSetBytes = ($samples | Measure-Object WorkingSetBytes -Maximum).Maximum
    AveragePrivateBytes = ($samples | Measure-Object PrivateBytes -Average).Average
    PeakSampledPrivateBytes = ($samples | Measure-Object PrivateBytes -Maximum).Maximum
    AverageCpuPercentOfOneCore = if ($cpuSamples.Count) { ($cpuSamples | Measure-Object CpuPercentOfOneCore -Average).Average } else { $null }
}
[pscustomobject]@{
    Scenario = $Scenario
    DurationSeconds = $clock.Elapsed.TotalSeconds
    SamplingIntervalMilliseconds = $IntervalMilliseconds
    LogicalProcessors = $logicalProcessors
    Scope = 'Visible Razer/RzSynapse/GameManagerService/Synapse process names plus PIDs of matching services inventoried at start; no processes or services changed; kernel-driver memory excluded'
    MemoryNote = 'Summed working sets may count shared pages repeatedly; private bytes measure private committed memory, not private resident memory. Peaks are sampled.'
    CpuNote = 'Percent of one core; missing/protected process CPU stays null and is excluded from the partial aggregate. First observations have no delta.'
    Services = $services
    Summary = $summary
    Samples = @($samples)
} | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath $outputPath -Encoding utf8
$summary | Format-List
