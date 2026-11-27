<# : batch part, PowerShell ignores it
@echo off
title A380 for X-Plane 12 - native screens
set "A380_NATIVE_SCRIPT=%~f0"
powershell -NoProfile -ExecutionPolicy Bypass -Command "iex ([IO.File]::ReadAllText($env:A380_NATIVE_SCRIPT))"
echo.
pause
exit /b
#>

# Turns the A380's native screens (drawn by the plugin, no display app) on or off in X-Plane 12's own settings.
$ErrorActionPreference = 'Stop'
$aircraftDir = 'Aircraft\A380 for X-Plane 12'

function Get-XPlaneRoots {
    $roots = New-Object System.Collections.Generic.List[string]
    if ($env:A380_XPLANE_ROOT) { $roots.Add($env:A380_XPLANE_ROOT) }
    $txt = Join-Path $env:LOCALAPPDATA 'x-plane_install_12.txt'
    if (Test-Path -LiteralPath $txt) {
        foreach ($line in Get-Content -LiteralPath $txt) { if ($line.Trim()) { $roots.Add($line.Trim()) } }
    }
    $steam = @()
    foreach ($key in 'HKCU:\Software\Valve\Steam', 'HKLM:\SOFTWARE\WOW6432Node\Valve\Steam') {
        try {
            $p = Get-ItemProperty -Path $key -ErrorAction Stop
            if ($p.SteamPath) { $steam += $p.SteamPath }
            if ($p.InstallPath) { $steam += $p.InstallPath }
        } catch {}
    }
    foreach ($s in $steam) {
        $vdf = Join-Path $s 'steamapps\libraryfolders.vdf'
        if (Test-Path -LiteralPath $vdf) {
            foreach ($m in Select-String -LiteralPath $vdf -Pattern '"path"\s+"([^"]+)"') {
                $lib = $m.Matches[0].Groups[1].Value -replace '\\\\', '\'
                $roots.Add((Join-Path $lib 'steamapps\common\X-Plane 12'))
            }
        }
    }
    foreach ($d in Get-PSDrive -PSProvider FileSystem) {
        $roots.Add((Join-Path $d.Root 'X-Plane 12'))
        $roots.Add((Join-Path $d.Root 'Games\X-Plane 12'))
    }
    $seen = @{}
    foreach ($r in $roots) {
        try { $full = [IO.Path]::GetFullPath(($r -replace '/', '\')).TrimEnd('\') } catch { continue }
        if ($seen.ContainsKey($full.ToLower())) { continue }
        $seen[$full.ToLower()] = $true
        if (Test-Path -LiteralPath (Join-Path $full $aircraftDir)) { $full }
    }
}

Write-Host 'A380 for X-Plane 12: native screens'
Write-Host '-----------------------------------'
$found = @(Get-XPlaneRoots)
if ($found.Count -eq 0) {
    Write-Host "No X-Plane 12 with '$aircraftDir' was found."
    $manual = Read-Host 'Paste your X-Plane 12 folder (or press Enter to quit)'
    if (-not $manual) { return }
    $manual = $manual.Trim('"', ' ').TrimEnd('\')
    if (-not (Test-Path -LiteralPath (Join-Path $manual $aircraftDir))) { Write-Host "No '$aircraftDir' in $manual."; return }
    $found = @($manual)
}
$root = $found[0]
if ($found.Count -gt 1) {
    for ($i = 0; $i -lt $found.Count; $i++) { Write-Host "  [$($i + 1)] $($found[$i])" }
    $pick = Read-Host 'Which X-Plane? (number)'
    $n = 0
    if (-not [int]::TryParse($pick, [ref]$n) -or $n -lt 1 -or $n -gt $found.Count) { Write-Host 'Nothing changed.'; return }
    $root = $found[$n - 1]
}
Write-Host "X-Plane 12: $root"

if (-not (Test-Path -LiteralPath (Join-Path $root "$aircraftDir\plugins\fbw_a380_systems\64\a380_displays.dll"))) {
    Write-Host 'This A380 has no native screens yet. Update it to 1.0.4 or later with the installer first.'
    return
}
if (Get-Process -Name 'X-Plane' -ErrorAction SilentlyContinue) {
    Write-Host 'X-Plane is running. Close it first, then run this again.'
    return
}

$prefs = Join-Path $root 'Output\preferences\xphfbw.json'
$settings = [pscustomobject]@{}
if (Test-Path -LiteralPath $prefs) {
    try { $settings = [IO.File]::ReadAllText($prefs) | ConvertFrom-Json }
    catch { Write-Host "$prefs is not valid JSON; nothing changed."; return }
}
$now = if ($settings.PSObject.Properties['native'] -and "$($settings.native)".Trim().ToLower() -eq 'all') { 'ON' } else { 'OFF' }
Write-Host "Native screens are now: $now"
Write-Host ''
Write-Host '  [1] Native screens ON  (the plugin draws every screen, no display app)'
Write-Host '  [2] Native screens OFF (the display app draws them, as before)'
$choice = if ($env:A380_NATIVE_CHOICE) { $env:A380_NATIVE_CHOICE } else { Read-Host 'Choose 1 or 2' }

switch ($choice.Trim()) {
    '1' { $settings | Add-Member -NotePropertyName 'native' -NotePropertyValue 'all' -Force; $after = 'ON' }
    '2' { [void]$settings.PSObject.Properties.Remove('native'); $after = 'OFF' }
    default { Write-Host 'Nothing changed.'; return }
}
if (Test-Path -LiteralPath $prefs) { Copy-Item -LiteralPath $prefs -Destination "$prefs.bak" -Force }
else { New-Item -ItemType Directory -Force -Path (Split-Path $prefs) | Out-Null }
[IO.File]::WriteAllText($prefs, ($settings | ConvertTo-Json -Depth 32), (New-Object Text.UTF8Encoding $false))
Write-Host ''
Write-Host "Native screens: $after. Start X-Plane to use it."
if (Test-Path -LiteralPath "$prefs.bak") { Write-Host "Your previous settings are saved as $prefs.bak" }
