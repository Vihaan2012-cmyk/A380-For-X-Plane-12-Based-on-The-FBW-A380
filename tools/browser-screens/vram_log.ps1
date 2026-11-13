param([string]$Out = "E:\A380\fbw-build\tmp\vram-flight.txt")
"time`tprocess`tdedicated_MB`tshared_MB" | Out-File -Encoding utf8 $Out
$idle = 0
for ($n = 0; $n -lt 720; $n++) {
  Start-Sleep -Seconds 10
  if (-not (Get-Process X-Plane -ErrorAction SilentlyContinue)) {
    $idle++
    if ($idle -gt 3) { break }
    continue
  }
  $idle = 0
  $t = Get-Date -Format HH:mm:ss
  try {
    $ded = (Get-Counter '\GPU Process Memory(*)\Dedicated Usage' -ErrorAction Stop).CounterSamples
    $sh = (Get-Counter '\GPU Process Memory(*)\Shared Usage' -ErrorAction Stop).CounterSamples
  } catch { continue }
  $rows = @{}
  foreach ($pair in @(@($ded, 0), @($sh, 1))) {
    foreach ($s in $pair[0]) {
      if ($s.CookedValue -lt 1MB) { continue }
      $id = [int][regex]::Match($s.InstanceName, 'pid_(\d+)_').Groups[1].Value
      $name = (Get-Process -Id $id -ErrorAction SilentlyContinue).ProcessName
      if (-not $name) { continue }
      if (-not $rows.ContainsKey($name)) { $rows[$name] = @(0.0, 0.0) }
      $rows[$name][$pair[1]] += $s.CookedValue / 1MB
    }
  }
  foreach ($k in $rows.Keys) {
    if ($rows[$k][0] -gt 20 -or $rows[$k][1] -gt 20) {
      "$t`t$k`t$([math]::Round($rows[$k][0]))`t$([math]::Round($rows[$k][1]))" | Out-File -Append -Encoding utf8 $Out
    }
  }
}
