param(
    [Parameter(Mandatory=$true)][string]$Output,
    [int]$Samples = 30
)
$ErrorActionPreference = 'Stop'
$processors = [Environment]::ProcessorCount
$watch = [Diagnostics.Stopwatch]::StartNew()
$previous = @{}
$rows = @()
for ($sample = 0; $sample -le $Samples; $sample++) {
    $elapsed = $watch.Elapsed.TotalSeconds
    foreach ($process in Get-Process) {
        $group = $null
        if ($process.ProcessName -eq 'secbird-tray') { $group = 'lite' }
        elseif ($process.ProcessName -match '^(Razer|Rz)' -or $process.ProcessName -eq 'GameManagerService') { $group = 'stock' }
        if (-not $group) { continue }
        try {
            $process.Refresh()
            # Protected service processes may expose memory but no start/CPU
            # counter. Retain their memory rows rather than losing the sample.
            $cpu = $null
            $started = $null
            try { if ($null -ne $process.TotalProcessorTime) { $cpu = $process.TotalProcessorTime.TotalSeconds } } catch {}
            try { if ($null -ne $process.StartTime) { $started = $process.StartTime.ToUniversalTime().ToString('o') } } catch {}
            $key = "$($process.Id):$started"
            $oneCore = $null
            if ($null -ne $cpu -and $null -ne $started -and $previous.ContainsKey($key)) {
                $dt = $elapsed - $previous[$key].elapsed
                if ($dt -gt 0) { $oneCore = 100 * ($cpu - $previous[$key].cpu) / $dt }
            }
            if ($null -ne $cpu -and $null -ne $started) { $previous[$key] = @{ elapsed=$elapsed; cpu=$cpu } }
            $rows += [ordered]@{
                sample=$sample; elapsed_seconds=$elapsed; group=$group
                pid=$process.Id; name=$process.ProcessName; path=$process.Path
                started_utc=$started; working_set_bytes=$process.WorkingSet64
                private_bytes=$process.PrivateMemorySize64
                cpu_percent_one_core=$oneCore
                cpu_percent_machine=if ($null -eq $oneCore) {$null} else {$oneCore/$processors}
                handles=$process.HandleCount; threads=$process.Threads.Count
            }
        } catch {
            $rows += [ordered]@{sample=$sample;elapsed_seconds=$elapsed;group=$group;pid=$process.Id;name=$process.ProcessName;error=$_.Exception.Message}
        }
    }
    if ($sample -lt $Samples) { Start-Sleep -Milliseconds 1000 }
}
[ordered]@{
    measured_utc=[DateTime]::UtcNow.ToString('o'); logical_processors=$processors
    scope='Existing processes only: secbird-tray; names beginning Razer/Rz and GameManagerService. No processes started/stopped or devices written.'
    workload='Observed desktop session, no automated interaction; not controlled startup or feature-load acceptance.'
    rows=$rows
} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath $Output -Encoding utf8
Write-Output "Saved $($rows.Count) process samples to $Output"
