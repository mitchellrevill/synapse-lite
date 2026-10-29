$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/stock-runtime-counters.ps1"
$target = 'C:\Program Files\Razer\AppEngine\RazerAppEngine.exe'
$started = [DateTime]::SpecifyKind([DateTime]'2026-10-29T12:00:00', [DateTimeKind]::Utc)
$entry = [pscustomobject]@{Name='RazerAppEngine.exe';ProcessId=42;ExecutablePath=$target;CreationDate=$started}
$valid = { param($processId) [pscustomobject]@{Id=$processId;StartUtc=$started;WorkingSet=1024;PrivateBytes=2048;CpuSeconds=3.5} }
$checks = 0
function Expect-Refusal {
    param([scriptblock]$Action, [string]$Message)
    $errorSeen = $null
    try { & $Action | Out-Null } catch { $errorSeen = $_.Exception.Message }
    if ($null -eq $errorSeen -or $errorSeen -notlike $Message) { throw "Expected refusal '$Message', observed '$errorSeen'" }
    $script:checks++
}
$rows = @(Read-VerifiedStockSample @($entry) $target $valid)
if ($rows.Count -ne 1 -or $rows[0].working_set -ne 1024 -or $rows[0].private_bytes -ne 2048 -or $rows[0].cpu_seconds -ne 3.5) { throw 'Counter preservation failed' }
$checks++
Expect-Refusal { Read-VerifiedStockSample @() $target $valid } '*empty roster*'
Expect-Refusal { Read-VerifiedStockSample @($entry) ($target+'other') $valid } '*selected AppEngine*'
Expect-Refusal { Read-VerifiedStockSample @($entry) $target {param($processId) $null} } '*Counters unavailable*'
Expect-Refusal {
    Read-VerifiedStockSample @($entry) $target {param($processId) throw 'Access denied to counter'}
} '*Access denied*'
Expect-Refusal {
    Read-VerifiedStockSample @($entry) $target {
        param($processId)
        [pscustomobject]@{Id=$processId;StartUtc=$started.AddSeconds(2);WorkingSet=1;PrivateBytes=1;CpuSeconds=1}
    }
} '*identity changed*'
foreach ($badValue in @($null, -1, [double]::NaN, [double]::PositiveInfinity)) {
    Expect-Refusal {
        Read-VerifiedStockSample @($entry) $target {
            param($processId)
            [pscustomobject]@{Id=$processId;StartUtc=$started;WorkingSet=1;PrivateBytes=$badValue;CpuSeconds=1}
        }
    } '*Invalid PrivateBytes*'
}
$hidden = [pscustomobject]@{Name='RzEngineMon.exe';ProcessId=43;ExecutablePath=$null;CreationDate=$started}
Expect-Refusal { Read-VerifiedStockSample @($entry,$hidden) $target $valid } '*Cannot establish executable scope*'
$outside = [pscustomobject]@{Name='RazerHelper.exe';ProcessId=43;ExecutablePath='C:\Users\Example\RazerHelper.exe';CreationDate=$started}
Expect-Refusal { Read-VerifiedStockSample @($entry,$outside) $target $valid } '*outside the documented roots*'
$missingStart = [pscustomobject]@{Name=$entry.Name;ProcessId=42;ExecutablePath=$target;CreationDate=$null}
Expect-Refusal { Read-VerifiedStockSample @($missingStart) $target $valid } '*Missing creation identity*'
$unrelated = [pscustomobject]@{Name='example.exe';ProcessId=1;ExecutablePath=$null;CreationDate=$null}
$rows = @(Read-VerifiedStockSample @($entry,$unrelated) $target $valid)
if ($rows.Count -ne 1) { throw 'Unrelated process affected scope' }
$checks++

# Exercise the actual report writer's early failure without touching real counters.
$reportPath = Join-Path ([IO.Path]::GetTempPath()) ('stock-empty-'+[Guid]::NewGuid().ToString('N')+'.json')
function Get-CimInstance { param($ClassName,$Property) return @() }
$runningExe = (Get-Process -Id $PID).Path
Expect-Refusal {
    & "$PSScriptRoot/measure-stock-razer.ps1" -AppEngineExe $runningExe -OutputPath $reportPath -Workload 'Synthetic empty roster' -SampleCount 2 -IntervalSeconds 0.1
} '*empty roster*'
if (Test-Path -LiteralPath $reportPath) { throw 'Empty capture wrote a misleading baseline' }
$checks++
Write-Output "$checks stock runtime checks passed; synthetic fixtures, no stock app or device activity"
