# Read-only process counters; close/open/configure the target workload separately.
[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string]$OutputPath,
    [Parameter(Mandatory=$true)][string]$AppEngineExe,
    [Parameter(Mandatory=$true)][string]$Workload,
    [ValidateRange(2,300)][int]$SampleCount=12,
    [ValidateRange(0.1,30)][double]$IntervalSeconds=2
)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/stock-runtime-counters.ps1"
$taskExe=Get-Item -LiteralPath $AppEngineExe -ErrorAction Stop
if ($taskExe.Extension -ne '.exe') { throw 'Select the installed AppEngine executable' }
if (Test-Path -LiteralPath $OutputPath) { throw 'Choose a new report path' }
$targetHash = (Get-FileHash -LiteralPath $taskExe.FullName -Algorithm SHA256).Hash

$samples = [System.Collections.Generic.List[object]]::new()
$prior = @{}
$priorTime = $null
for ($sampleIndex=0; $sampleIndex -lt $SampleCount; $sampleIndex++) {
    $now = [DateTime]::UtcNow
    $inventory = @(Get-CimInstance Win32_Process -Property Name,ProcessId,ExecutablePath,CreationDate)
    $items = @(Read-VerifiedStockSample -Inventory $inventory -TargetPath $taskExe.FullName -ReadCounters {
        param($processId)
        $p = Get-Process -Id $processId -ErrorAction Stop
        try {
            $p.Refresh()
            [pscustomobject]@{Id=$p.Id;StartUtc=$p.StartTime.ToUniversalTime();WorkingSet=$p.WorkingSet64;PrivateBytes=$p.PrivateMemorySize64;CpuSeconds=$p.TotalProcessorTime.TotalSeconds}
        } finally { $p.Dispose() }
    })
    $cpuDelta = 0.0
    $next = @{}
    foreach ($item in $items) {
        $key = "$($item.pid):$($item.start_utc)"
        if ($prior.ContainsKey($key)) { $cpuDelta += [Math]::Max(0.0,$item.cpu_seconds-$prior[$key]) }
        elseif ($priorTime -and [DateTime]::Parse($item.start_utc) -ge $priorTime) { $cpuDelta += $item.cpu_seconds }
        $next[$key] = $item.cpu_seconds
    }
    $percent = if ($priorTime) { 100.0*$cpuDelta/($now-$priorTime).TotalSeconds } else { $null }
    $samples.Add([pscustomobject]@{utc=$now.ToString('o');processes=$items;working_set=[long](($items | Measure-Object working_set -Sum).Sum);private_bytes=[long](($items | Measure-Object private_bytes -Sum).Sum);cpu_percent_single_core=$percent;cpu_percent_machine=if($null -ne $percent){$percent/[Environment]::ProcessorCount}else{$null}})
    $prior=$next; $priorTime=$now
    if ($sampleIndex -lt ($SampleCount-1)) { Start-Sleep -Milliseconds ([int]($IntervalSeconds*1000)) }
}
if ((Get-FileHash -LiteralPath $taskExe.FullName -Algorithm SHA256).Hash -ne $targetHash) { throw 'Target executable changed during measurement; no baseline written' }
$report = [pscustomobject]@{
    schema_version = 2
    scope = $Workload
    process_scope = 'All readable executables under both Program Files Razer roots; selected executable must be present in every sample. Named stock candidates with unreadable paths or outside those roots abort the capture.'
    coverage = 'Complete counters for the enumerated scope at each sample, not proof of all installed Razer components'
    version_info = $taskExe.VersionInfo.ProductVersion
    executable_sha256 = $targetHash
    logical_processors = [Environment]::ProcessorCount
    samples = $samples
    peak_working_set = [long](($samples | Measure-Object working_set -Maximum).Maximum)
    peak_private_bytes = [long](($samples | Measure-Object private_bytes -Maximum).Maximum)
    mean_cpu_percent_machine = (($samples | Where-Object {$null -ne $_.cpu_percent_machine} | Measure-Object cpu_percent_machine -Average).Average)
    limitations = 'Sampled peaks; processes may start/exit between inventories. Shared working-set pages may be counted across processes; private bytes are committed memory, not private resident memory. Exited-process tail CPU and kernel allocations are unmeasured. Other desktop applications remained open. No full Synapse UI/replacement comparison or three-run acceptance.'
}
$bytes=[Text.Encoding]::UTF8.GetBytes(($report | ConvertTo-Json -Depth 6))
$stream=[IO.File]::Open([IO.Path]::GetFullPath($OutputPath),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
try { $stream.Write($bytes,0,$bytes.Length); $stream.Flush($true) } finally { $stream.Dispose() }
$report | Select-Object version_info,peak_working_set,peak_private_bytes,mean_cpu_percent_machine | ConvertTo-Json
