param([string]$Out, [int]$Every = 10)
# CPU per Chromium helper, as a percentage of one core: which type, and the busiest renderers.
"time`ttype`tpid`tcpu_pct" | Out-File -Encoding utf8 $Out
$was = @{}
$at = Get-Date
$idle = 0
while ($idle -lt 4) {
  Start-Sleep -Seconds $Every
  $list = Get-CimInstance Win32_Process -Filter "name='cefclient.exe'" -ErrorAction SilentlyContinue
  if (-not $list) { $idle++; continue }
  $idle = 0
  $now = Get-Date
  $seconds = ($now - $at).TotalSeconds
  $at = $now
  $stamp = $now.ToString("HH:mm:ss")
  foreach ($p in $list) {
    $used = ($p.KernelModeTime + $p.UserModeTime) / 1e7
    $type = if ($p.CommandLine -match "--type=([a-z-]+)") { $Matches[1] } else { "browser" }
    if ($was.ContainsKey($p.ProcessId)) {
      $pct = [math]::Round(($used - $was[$p.ProcessId]) * 100 / $seconds, 1)
      if ($pct -ge 1) { "$stamp`t$type`t$($p.ProcessId)`t$pct" | Out-File -Append -Encoding utf8 $Out }
    }
    $was[$p.ProcessId] = $used
  }
}
