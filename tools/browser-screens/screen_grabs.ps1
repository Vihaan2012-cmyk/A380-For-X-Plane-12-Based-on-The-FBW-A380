param([string]$Out, [int]$Every = 60)
# X-Plane's window as the monitor shows it, one PNG a minute, named by Unix time so they pair with FBW_DEVICE_GRABS.
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class ScreenGrab {
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
  [DllImport("user32.dll")] public static extern bool SetProcessDpiAwarenessContext(IntPtr value);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr hwnd, out RECT rect);
}
"@
[ScreenGrab]::SetProcessDpiAwarenessContext([IntPtr]::new(-4)) | Out-Null
New-Item -ItemType Directory -Force $Out | Out-Null
$idle = 0
while ($idle -lt 4) {
  Start-Sleep -Seconds $Every
  $xp = Get-Process -Name "X-Plane" -ErrorAction SilentlyContinue | Select-Object -First 1
  if (-not $xp -or $xp.MainWindowHandle -eq 0) { $idle++; continue }
  $idle = 0
  $r = New-Object ScreenGrab+RECT
  if (-not [ScreenGrab]::GetWindowRect($xp.MainWindowHandle, [ref]$r)) { continue }
  $w = $r.R - $r.L
  $h = $r.B - $r.T
  if ($w -le 0 -or $h -le 0) { continue }
  $bmp = New-Object System.Drawing.Bitmap $w, $h
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  try {
    $g.CopyFromScreen($r.L, $r.T, 0, 0, $bmp.Size)
    $name = [DateTimeOffset]::UtcNow.ToUnixTimeSeconds()
    $bmp.Save("$Out\$name.png", [System.Drawing.Imaging.ImageFormat]::Png)
  } catch {
  } finally {
    $g.Dispose()
    $bmp.Dispose()
  }
}
