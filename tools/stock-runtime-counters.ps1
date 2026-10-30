# Literal directory prefixes, not wildcard patterns. Additional roots must be
# explicitly inventoried before capture; defaults retain the original scope.
function Resolve-StockRuntimeRoots {
    param([string[]]$AdditionalRoots=@())
    $roots = [Collections.Generic.List[string]]::new()
    foreach ($root in @('C:\Program Files\Razer','C:\Program Files (x86)\Razer') + $AdditionalRoots) {
        if ([string]::IsNullOrWhiteSpace($root) -or -not [IO.Path]::IsPathFullyQualified($root) -or $root.IndexOfAny([char[]]'*?') -ge 0) {
            throw 'Stock roots must be explicit absolute directories without wildcard characters'
        }
        $resolved = [IO.Path]::GetFullPath($root).TrimEnd([char[]]'\/') + '\'
        if ([IO.Path]::GetPathRoot($resolved) -eq $resolved) { throw 'A whole drive is not a stock process scope' }
        if (-not $roots.Contains($resolved)) { $roots.Add($resolved) }
    }
    return $roots.ToArray()
}
# Pure sample validation with an injected counter reader for deterministic checks.
function Read-VerifiedStockSample {
    param(
        [AllowEmptyCollection()][object[]]$Inventory,
        [string]$TargetPath,
        [scriptblock]$ReadCounters,
        [string[]]$AdditionalRoots=@()
    )
    $roots = @(Resolve-StockRuntimeRoots -AdditionalRoots $AdditionalRoots)
    $rows = [Collections.Generic.List[object]]::new()
    $targetFound = $false
    foreach ($entry in $Inventory) {
        $scopeRoot = -1
        if (-not [string]::IsNullOrWhiteSpace($entry.ExecutablePath)) {
            if (-not [IO.Path]::IsPathFullyQualified($entry.ExecutablePath)) { throw 'Inventory has a relative executable path; no baseline written' }
            $imagePath = [IO.Path]::GetFullPath($entry.ExecutablePath)
            for ($rootIndex=0; $rootIndex -lt $roots.Count; $rootIndex++) {
                if ($imagePath.StartsWith($roots[$rootIndex], [StringComparison]::OrdinalIgnoreCase)) { $scopeRoot=$rootIndex; break }
            }
        }
        $inRoots = $scopeRoot -ge 0
        $namedCandidate = $entry.Name -match '^(Razer|Rz|GameManager|Synapse)'
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
            scope_root = $scopeRoot
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
