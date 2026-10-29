# Pure sample validation with an injected counter reader for deterministic checks.
function Read-VerifiedStockSample {
    param(
        [AllowEmptyCollection()][object[]]$Inventory,
        [string]$TargetPath,
        [scriptblock]$ReadCounters
    )
    $rows = [Collections.Generic.List[object]]::new()
    $targetFound = $false
    foreach ($entry in $Inventory) {
        $inRoots = $entry.ExecutablePath -like 'C:\Program Files\Razer\*' -or
            $entry.ExecutablePath -like 'C:\Program Files (x86)\Razer\*'
        $namedCandidate = $entry.Name -match '^(Razer|RzSynapse|RzEngineMon|RzChroma|GameManager|Synapse)'
        if ($namedCandidate -and [string]::IsNullOrWhiteSpace($entry.ExecutablePath)) {
            throw "Cannot establish executable scope for stock candidate PID $($entry.ProcessId); no baseline written"
        }
        if (-not $inRoots) {
            if ($namedCandidate) { throw "Stock candidate PID $($entry.ProcessId) is outside the documented roots; review scope before measuring" }
            continue
        }
        if ($null -eq $entry.CreationDate) { throw "Missing creation identity for PID $($entry.ProcessId)" }
        $counter = & $ReadCounters $entry.ProcessId
        if ($null -eq $counter) { throw "Counters unavailable for PID $($entry.ProcessId); retry the complete sample" }
        $started = ([DateTime]$counter.StartUtc).ToUniversalTime()
        $inventoried = ([DateTime]$entry.CreationDate).ToUniversalTime()
        if ($counter.Id -ne $entry.ProcessId -or [Math]::Abs(($started-$inventoried).TotalMilliseconds) -gt 1) {
            throw "Process identity changed for PID $($entry.ProcessId); no baseline written"
        }
        foreach ($field in @('WorkingSet','PrivateBytes','CpuSeconds')) {
            $value = $counter.$field
            if ($null -eq $value -or [double]::IsNaN([double]$value) -or [double]::IsInfinity([double]$value) -or $value -lt 0) {
                throw "Invalid $field counter for PID $($entry.ProcessId)"
            }
        }
        if ($entry.ExecutablePath -eq $TargetPath) { $targetFound = $true }
        $rows.Add([pscustomobject]@{
            pid = [int]$entry.ProcessId
            name = [string]$entry.Name
            start_utc = $started.ToString('o')
            working_set = [long]$counter.WorkingSet
            private_bytes = [long]$counter.PrivateBytes
            cpu_seconds = [double]$counter.CpuSeconds
        })
    }
    if ($rows.Count -eq 0) { throw 'No stock processes visible; an empty roster is not a zero-memory baseline' }
    if (-not $targetFound) { throw 'The selected AppEngine executable is not in this sample; no baseline written' }
    return $rows.ToArray()
}
