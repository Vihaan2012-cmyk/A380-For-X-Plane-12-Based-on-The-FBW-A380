$ids = @((Get-Process cefclient -ErrorAction SilentlyContinue).Id)
$ded = (Get-Counter '\GPU Process Memory(*)\Dedicated Usage').CounterSamples
$sh = (Get-Counter '\GPU Process Memory(*)\Shared Usage').CounterSamples
$by = @{}
foreach ($pair in @(@($ded, 0), @($sh, 1))) {
  foreach ($s in $pair[0]) {
    $m = [regex]::Match($s.InstanceName, 'pid_(\d+)_luid_0x[0-9a-f]+_0x([0-9a-f]+)_')
    if (-not $m.Success) { continue }
    if ($ids -notcontains [int]$m.Groups[1].Value) { continue }
    $l = $m.Groups[2].Value
    if (-not $by.ContainsKey($l)) { $by[$l] = @(0.0, 0.0) }
    $by[$l][$pair[1]] += $s.CookedValue / 1MB
  }
}
$rows = foreach ($k in $by.Keys) { "{0}: dedicated {1:N0} MB, shared {2:N0} MB" -f $k.Substring($k.Length - 4), $by[$k][0], $by[$k][1] }
$c1 = (Get-Process cefclient | Measure-Object CPU -Sum).Sum
Start-Sleep -Seconds 2
$c2 = (Get-Process cefclient | Measure-Object CPU -Sum).Sum
"helpers memory by adapter: " + ($rows -join "; ")
"helpers CPU {0:N0}% of a core" -f (($c2 - $c1) / 2 * 100)
