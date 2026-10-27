param(
    [switch]$WriteUniformFrames,
    [Parameter(Mandatory=$true)][string]$ExpectedSha256,
    [Parameter(Mandatory=$true)][string]$TestExecutable,
    [string]$ReportDirectory = "$PSScriptRoot\..\target\uniform-worker-probe",
    [string]$StockExecutable = 'C:\Program Files\Razer\RazerAppEngine\app-4.0.827\RazerAppEngine.exe'
)
$ErrorActionPreference = 'Stop'
if (!$WriteUniformFrames) { throw 'Explicit -WriteUniformFrames is required' }
$binary = (Resolve-Path -LiteralPath $TestExecutable).Path
if ((Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash -ne $ExpectedSha256) { throw 'Test executable does not match the reviewed build hash' }
$stockExe = (Resolve-Path -LiteralPath $StockExecutable).Path
if (!(Test-Path -LiteralPath $binary -PathType Leaf) -or !(Test-Path -LiteralPath $stockExe -PathType Leaf)) { throw 'Exact test and stock executables required' }
$workRoot = [IO.Path]::GetFullPath($ReportDirectory)
New-Item -ItemType Directory -Path $workRoot -Force | Out-Null
if (Test-Path Env:SECBIRD_SIM) { throw 'Physical probe refuses simulator configuration' }
$root = Join-Path $workRoot ('uniform-worker-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $root | Out-Null
$report = Join-Path $root 'result.json'
$record = [ordered]@{ binary_sha256 = (Get-FileHash -LiteralPath $binary -Algorithm SHA256).Hash; report_directory = $root; stock_before = 0; stock_isolated = $null; stock_after_restart = $null; stock_engine_count_after_restart = $null; services_changed = $false; timed_out = $false; exit_code = $null; error = $null }
$names = @('SYNAPSE_LITE_CONFIG_DIR','SYNAPSE_LITE_PHYSICAL_UNIFORM_PROBE','SYNAPSE_LITE_PHYSICAL_UNIFORM_REPORT')
$previous = @{}
foreach ($name in $names) { $previous[$name] = [Environment]::GetEnvironmentVariable($name, 'Process') }
$before = @(Get-Process -Name RazerAppEngine,razerwdl -ErrorAction SilentlyContinue)
$record.stock_before = $before.Count
if (@($before | Where-Object ProcessName -eq RazerAppEngine).Count -eq 0) { throw 'Initial running stock owner required for restart recovery' }
$child = $null
try {
    $stable = 0
    for ($round = 0; $round -lt 8 -and $stable -lt 2; $round++) {
        $remaining = @(Get-Process -Name RazerAppEngine,razerwdl -ErrorAction SilentlyContinue)
        if ($remaining.Count) { $remaining | Stop-Process -Force -ErrorAction SilentlyContinue; $stable = 0 } else { $stable++ }
        Start-Sleep -Milliseconds 500
    }
    $record.stock_isolated = @(Get-Process -Name RazerAppEngine,razerwdl -ErrorAction SilentlyContinue).Count
    if ($stable -lt 2 -or $record.stock_isolated -ne 0) { throw 'Stock owner did not settle within bounded isolation' }
    $env:SYNAPSE_LITE_CONFIG_DIR = Join-Path $root 'native-config'
    $env:SYNAPSE_LITE_PHYSICAL_UNIFORM_PROBE = 'two-programs-v1'
    $env:SYNAPSE_LITE_PHYSICAL_UNIFORM_REPORT = $report
    $child = Start-Process -FilePath $binary -ArgumentList @('--exact','server::physical_uniform_probe::bounded_spectrum_and_breathing_worker_preserve_controls','--ignored','--nocapture','--test-threads=1') -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $root 'stdout.private.txt') -RedirectStandardError (Join-Path $root 'stderr.private.txt')
    if (!$child.WaitForExit(60000)) { $record.timed_out = $true; throw 'Physical uniform worker probe timed out' }
    $child.Refresh()
    $record.exit_code = $child.ExitCode
    if ($child.ExitCode -ne 0) { throw 'Physical uniform worker probe failed; inspect local report and diagnostic logs' }
    if (!(Test-Path -LiteralPath $report)) { throw 'Physical probe produced no result' }
} catch { $record.error = $_.Exception.Message }
finally {
    if ($null -ne $child -and !$child.HasExited) { $child.Kill(); $child.WaitForExit(3000) | Out-Null }
    foreach ($name in $names) { [Environment]::SetEnvironmentVariable($name, $previous[$name], 'Process') }
    for ($attempt = 0; $attempt -lt 2; $attempt++) {
        try {
            Start-Process -FilePath $stockExe -ArgumentList @('--url-params=apps=synapse,chroma-app','--launch-force-hidden=synapse,chroma-app','--autoStart=1') -WindowStyle Hidden
        } catch {
            $record['stock_restart_error'] = $_.Exception.Message
        }
        Start-Sleep -Seconds 5
        $record.stock_after_restart = @(Get-Process -Name RazerAppEngine,razerwdl -ErrorAction SilentlyContinue).Count
        $record.stock_engine_count_after_restart = @(Get-Process -Name RazerAppEngine -ErrorAction SilentlyContinue).Count
        if ($record.stock_engine_count_after_restart -gt 0) { break }
    }
    $record.finished_utc = [DateTimeOffset]::UtcNow.ToString('o')
    $record | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $root 'session.json') -Encoding utf8
}
$record | ConvertTo-Json -Depth 5
if ($record.error -or $record.stock_engine_count_after_restart -eq 0) { throw 'Physical probe or stock restoration did not complete cleanly' }
