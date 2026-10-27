param(
    [switch]$WriteOneFrame,
    [string]$ReportPath,
    [string]$ReportDirectory = "$PSScriptRoot\..\target\frame-probe",
    [string]$FrameExecutable = "$PSScriptRoot\..\target\release\examples\ornata_frame_probe.exe",
    [string]$ReadbackExecutable = "$PSScriptRoot\..\target\release\examples\lighting_readback.exe",
    [string]$StockExecutable = 'C:\Program Files\Razer\RazerAppEngine\app-4.0.827\RazerAppEngine.exe'
)
$ErrorActionPreference = 'Stop'
if (!$WriteOneFrame) { throw 'Opt-in required: -WriteOneFrame. This script has not been authorized to execute merely by being prepared.' }
if (Test-Path Env:SECBIRD_SIM) { throw 'Physical frame wrapper refuses SECBIRD_SIM' }
$workRoot = [IO.Path]::GetFullPath($ReportDirectory)
New-Item -ItemType Directory -Path $workRoot -Force | Out-Null
if ([string]::IsNullOrWhiteSpace($ReportPath)) {
    $ReportPath = Join-Path $workRoot ('ornata-frame-probe-' + [guid]::NewGuid().ToString('N') + '.json')
}
$ReportPath = [IO.Path]::GetFullPath($ReportPath)
if (!$ReportPath.StartsWith($workRoot + '\',[StringComparison]::OrdinalIgnoreCase)) {
    throw 'Diagnostic report must stay inside the explicitly selected report directory'
}
$frameExe = [IO.Path]::GetFullPath($FrameExecutable)
$queryExe = [IO.Path]::GetFullPath($ReadbackExecutable)
$stockExe = [IO.Path]::GetFullPath($StockExecutable)
$baselinePath = [IO.Path]::ChangeExtension($ReportPath,'baseline.json')
$sessionPath = [IO.Path]::ChangeExtension($ReportPath,'session.json')
$restorePath = [IO.Path]::ChangeExtension($ReportPath,'restored-profile0.json')
$stdoutPath = [IO.Path]::ChangeExtension($ReportPath,'stdout.txt')
$stderrPath = [IO.Path]::ChangeExtension($ReportPath,'stderr.txt')
$restoreStdout = [IO.Path]::ChangeExtension($ReportPath,'restore-stdout.txt')
$restoreStderr = [IO.Path]::ChangeExtension($ReportPath,'restore-stderr.txt')
foreach ($exe in @($frameExe,$queryExe,$stockExe)) {
    if (!(Test-Path -LiteralPath $exe -PathType Leaf)) { throw 'An exact reviewed probe/query/stock executable is missing' }
}
foreach ($path in @($ReportPath,$baselinePath,$sessionPath,$restorePath,$stdoutPath,$stderrPath,$restoreStdout,$restoreStderr)) {
    if (Test-Path -LiteralPath $path) { throw 'A chosen diagnostic file already exists; choose a fresh report path' }
}
if (!(Test-Path -LiteralPath ([IO.Path]::GetDirectoryName($ReportPath)) -PathType Container)) {
    throw 'Diagnostic parent directory must already exist'
}
$before = @(Get-Process -Name RazerAppEngine,razerwdl -ErrorAction SilentlyContinue)
if (@($before | Where-Object ProcessName -eq 'RazerAppEngine').Count -eq 0) {
    throw 'Configured stock RazerAppEngine must initially be running for this restart strategy'
}
$session = [ordered]@{
    startedUtc = [DateTime]::UtcNow.ToString('o')
    intent = 'One uniform low-intensity frame; restore strategy is unchanged stock-owner restart, not exact RGB/phase/input restoration'
    before = @($before | Select-Object ProcessName,Id)
    beforeCount = $before.Count
    frameBinarySha256 = (Get-FileHash -LiteralPath $frameExe -Algorithm SHA256).Hash
    readOnlyQueryBinarySha256 = (Get-FileHash -LiteralPath $queryExe -Algorithm SHA256).Hash
    reportPath = $ReportPath
    probeExitCode = $null
    probeTimedOut = $false
    failure = $null
    ownerRestartAttempted = $false
    ownerProcessRunningAfterRestart = $false
    exactRgbPhaseInputRestorationProven = $false
    serviceChangesRequested = $false
}
$session | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath $sessionPath -Encoding utf8
$oldGuard = [Environment]::GetEnvironmentVariable('SECBIRD_ORNATA_FRAME_OWNER_RESTART','Process')
try {
    # This is the previously reviewed user-process boundary. SDK/services remain running.
    $before | Stop-Process -Force
    Start-Sleep -Milliseconds 700
    $session['isolatedCount'] = @(Get-Process -Name RazerAppEngine,razerwdl -ErrorAction SilentlyContinue).Count
    if ($session['isolatedCount'] -ne 0) { throw 'Stock writer process respawned; refusing the frame child' }
    [Environment]::SetEnvironmentVariable('SECBIRD_ORNATA_FRAME_OWNER_RESTART','reviewed-wrapper-finally-restart','Process')
    $quotedReport = '"' + $ReportPath + '"'
    $probe = Start-Process -FilePath $frameExe -ArgumentList @($quotedReport,'--write-one-frame') -WindowStyle Hidden -PassThru -RedirectStandardOutput $stdoutPath -RedirectStandardError $stderrPath
    if (!$probe.WaitForExit(20000)) {
        $session['probeTimedOut'] = $true
        Stop-Process -Id $probe.Id -Force
        $session['probeTerminationConfirmed'] = $probe.WaitForExit(3000)
        throw 'Frame child exceeded20seconds; outcome may include partial frame writes'
    }
    $probe.Refresh()
    $session['probeTerminationConfirmed'] = $true
    $session['probeExitCode'] = $probe.ExitCode
}
catch { $session['failure'] = $_.Exception.Message }
finally {
    [Environment]::SetEnvironmentVariable('SECBIRD_ORNATA_FRAME_OWNER_RESTART',$oldGuard,'Process')
    try {
        # Always restart the initially running configured owner, including after child failure.
        $session['ownerRestartAttempted'] = $true
        Start-Process -FilePath $stockExe -ArgumentList @('--url-params=apps=synapse,chroma-app','--launch-force-hidden=synapse,chroma-app','--autoStart=1') -WindowStyle Hidden
        Start-Sleep -Seconds 5
        $after = @(Get-Process -Name RazerAppEngine,razerwdl -ErrorAction SilentlyContinue)
        $session['after'] = @($after | Select-Object ProcessName,Id)
        $session['afterCount'] = $after.Count
        $session['ownerProcessRunningAfterRestart'] = @($after | Where-Object ProcessName -eq 'RazerAppEngine').Count -gt 0
    }
    catch { $session['ownerRestartFailure'] = $_.Exception.Message }
    try {
        # Read only, with the restarted owner left running. Failure/competition is evidence;
        # do not kill the owner again or retry a frame to obtain a cleaner diagnostic.
        $quotedRestore = '"' + $restorePath + '"'
        $readback = Start-Process -FilePath $queryExe -ArgumentList @('0',$quotedRestore,'--read-only') -WindowStyle Hidden -PassThru -RedirectStandardOutput $restoreStdout -RedirectStandardError $restoreStderr
        if (!$readback.WaitForExit(10000)) {
            $session['restoreQueryTimedOut'] = $true
            Stop-Process -Id $readback.Id -Force
            $readback.WaitForExit(3000) | Out-Null
        }
        else { $readback.Refresh(); $session['restoreQueryExitCode'] = $readback.ExitCode }
        $session['restoreControlReportExists'] = Test-Path -LiteralPath $restorePath -PathType Leaf
        if ($session['restoreControlReportExists']) {
            $session['restoreControlObservation'] = Get-Content -LiteralPath $restorePath -Raw | ConvertFrom-Json
        }
    }
    catch { $session['restoreQueryFailure'] = $_.Exception.Message }
    $session['finishedUtc'] = [DateTime]::UtcNow.ToString('o')
    $session | ConvertTo-Json -Depth 20 | Set-Content -LiteralPath $sessionPath -Encoding utf8
}
$session | ConvertTo-Json -Depth 20
if (!$session['ownerProcessRunningAfterRestart']) { throw 'Stock owner restart did not produce an observed running engine process' }
if ($session['probeTimedOut'] -or $null -ne $session['failure'] -or $session['probeExitCode'] -ne 0) {
    throw 'Frame probe did not complete cleanly; inspect immutable baseline, report and session evidence'
}
