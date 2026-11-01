param([string]$Label = "run", [string]$Set = "", [string]$XPlane = $env:FBW_XPLANE_ROOT, [switch]$Grabs)
if (-not $XPlane) { "set FBW_XPLANE_ROOT to the X-Plane folder"; exit 2 }
if (Get-Process -Name "X-Plane" -ErrorAction SilentlyContinue) { "X-Plane is already running"; exit 1 }
foreach ($pair in ($Set -split ";" | Where-Object { $_ })) {
  $name, $value = $pair -split "=", 2
  Set-Item -Path "env:$name" -Value $value
}
$dir = "E:\fbw-build\perf-runs\{0:yyyyMMdd-HHmm}-{1}" -f (Get-Date), $Label
New-Item -ItemType Directory -Force $dir | Out-Null
Copy-Item "$XPlane\Aircraft\A380 for X-Plane 12\plugins\fbw_a380_systems\64\win.xpl" "$dir\win.xpl"
if ($Grabs) {
  $env:FBW_DEVICE_GRABS = "$dir\devices"
  Start-Process -FilePath "powershell.exe" -ArgumentList "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "$PSScriptRoot\screen_grabs.ps1", "-Out", "$dir\screens" -WindowStyle Hidden | Out-Null
  Start-Process -FilePath "powershell.exe" -ArgumentList "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", "$PSScriptRoot\chromium_cpu.ps1", "-Out", "$dir\chromium-cpu.txt" -WindowStyle Hidden | Out-Null
  Start-Process -FilePath "node" -ArgumentList "$PSScriptRoot\vars_watch.mjs", "$dir\vars-watch" -WindowStyle Hidden | Out-Null
}
$xp = Start-Process -FilePath "$XPlane\X-Plane.exe" -WorkingDirectory $XPlane -PassThru
"X-Plane started, PID $($xp.Id), results in $dir"
& "$PSScriptRoot\vram_log.ps1" -Out "$dir\vram.txt"
Copy-Item "$XPlane\Log.txt" "$dir\Log.txt"
python "$PSScriptRoot\perf_report.py" "$dir\Log.txt" "$dir\vram.txt" | Tee-Object "$dir\report.txt"
