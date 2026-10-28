# Read-only process counters; close/open/configure the target workload separately.
[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string]$OutputPath,
    [Parameter(Mandatory=$true)][string]$AppEngineExe,
    [Parameter(Mandatory=$true)][string]$Workload,
    [ValidateRange(2,300)][int]$SampleCount=12,
    [ValidateRange(0.1,30)][double]$IntervalSeconds=2
)
$taskExe=Get-Item -LiteralPath $AppEngineExe -ErrorAction Stop
if ($taskExe.Extension -ne '.exe') { throw 'Select the installed AppEngine executable' }
if (Test-Path -LiteralPath $OutputPath) { throw 'Choose a new report path' }

$samples = [System.Collections.Generic.List[object]]::new()
$prior = @{}
$priorTime = $null
for ($sampleIndex=0; $sampleIndex -lt $SampleCount; $sampleIndex++) {
    $now = [DateTime]::UtcNow
    $items = @(Get-CimInstance Win32_Process | Where-Object {
        $_.ExecutablePath -like 'C:\Program Files\Razer\*' -or $_.ExecutablePath -like 'C:\Program Files (x86)\Razer\*'
    } | ForEach-Object {
        $p = Get-Process -Id $_.ProcessId -ErrorAction SilentlyContinue
        if ($p) { [pscustomobject]@{pid=$p.Id;name=$p.ProcessName;start_utc=$p.StartTime.ToUniversalTime().ToString('o');working_set=[long]$p.WorkingSet64;private_bytes=[long]$p.PrivateMemorySize64;cpu_seconds=$p.CPU} }
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
$report=[pscustomobject]@{scope=$Workload;version_info=$taskExe.VersionInfo.ProductVersion;executable_sha256=(Get-FileHash -LiteralPath $taskExe.FullName -Algorithm SHA256).Hash;logical_processors=[Environment]::ProcessorCount;samples=$samples;peak_working_set=[long](($samples | Measure-Object working_set -Maximum).Maximum);peak_private_bytes=[long](($samples | Measure-Object private_bytes -Maximum).Maximum);mean_cpu_percent_machine=(($samples | Where-Object {$null -ne $_.cpu_percent_machine} | Measure-Object cpu_percent_machine -Average).Average);limitations='Sampled peaks, shared working-set pages may be counted across processes; exited-process tail CPU and kernel allocations are unmeasured. Other desktop applications remained open. No full Synapse UI/replacement comparison or three-run acceptance.'}
$bytes=[Text.Encoding]::UTF8.GetBytes(($report | ConvertTo-Json -Depth 6))
$stream=[IO.File]::Open([IO.Path]::GetFullPath($OutputPath),[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
try { $stream.Write($bytes,0,$bytes.Length); $stream.Flush($true) } finally { $stream.Dispose() }
$report | Select-Object version_info,peak_working_set,peak_private_bytes,mean_cpu_percent_machine | ConvertTo-Json
