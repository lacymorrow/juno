# LAC-6321 trials, part 4: two routes around UI Automation, each pressed from
# behind Notepad. CDP (what Juno's browser stack uses) on an Edge page, and
# MSAA's accDoDefaultAction on WPF, Edge and VS Code. Same rules as the other
# trials: Notepad must be in front when a trial starts, and a press counts only
# if its effect shows up. Opens its own apps. -Part 2 reruns the Edge and
# VS Code MSAA cases after reading their trees first, since Chromium builds
# its accessibility tree only when asked.
param([int]$Part = 1)
Add-Type -TypeDefinition 'using System; using System.Text; using System.Runtime.InteropServices; public static class P { [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow(); [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n); [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr h,int x,int y,int w,int ht,bool r); [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h); [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint p); [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId(); [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint a, uint b, bool f); [DllImport("user32.dll")] public static extern void keybd_event(byte k, byte s, uint f, UIntPtr e); [DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT p); [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X; public int Y; } }'
Add-Type -ReferencedAssemblies Accessibility -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using Accessibility;
// Finds an MSAA object by name under a window and runs its default action.
public static class Msaa {
  [DllImport("oleacc.dll")] static extern int AccessibleObjectFromWindow(IntPtr h, int id, ref Guid iid, [MarshalAs(UnmanagedType.Interface)] out IAccessible acc);
  [DllImport("oleacc.dll")] static extern int AccessibleChildren(IAccessible p, int start, int count, [Out] object[] children, out int obtained);
  public static string Press(IntPtr hwnd, string name) {
    Guid iid = new Guid("618736E0-3C3D-11CF-810C-00AA00389B71");
    IAccessible root;
    int hr = AccessibleObjectFromWindow(hwnd, -4, ref iid, out root);
    if (hr != 0 || root == null) return "no MSAA root (" + hr + ")";
    return Find(root, name, 0) ?? "not found";
  }
  static string Find(IAccessible a, string name, int depth) {
    if (depth > 40) return null;
    int n = 0;
    try { n = a.accChildCount; } catch { return null; }
    if (n <= 0) return null;
    object[] kids = new object[n]; int got;
    if (AccessibleChildren(a, 0, n, kids, out got) != 0) return null;
    for (int i = 0; i < got; i++) {
      IAccessible k = kids[i] as IAccessible;
      if (k != null) {
        string nm = null; try { nm = k.get_accName(0); } catch { }
        if (nm == name) { k.accDoDefaultAction(0); return "MSAA accDoDefaultAction"; }
        string r = Find(k, name, depth + 1); if (r != null) return r;
      } else if (kids[i] is int) {
        int id = (int)kids[i]; string nm = null; try { nm = a.get_accName(id); } catch { }
        if (nm == name) { a.accDoDefaultAction(id); return "MSAA accDoDefaultAction (child id)"; }
      }
    }
    return null;
  }
}
'@
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$A = [System.Windows.Automation.AutomationElement]
$dir = "C:\JunoSpike\trials-cdp-msaa-$Part"; New-Item -ItemType Directory -Force $dir | Out-Null
Start-Transcript "$dir\transcript.txt" -Force | Out-Null
function Title($h) { $s=New-Object Text.StringBuilder 256; [void][P]::GetWindowText($h,$s,256); $s.ToString() }
function Fg { Title ([P]::GetForegroundWindow()) }
function Cursor { $p = New-Object P+POINT; [void][P]::GetCursorPos([ref]$p); "$($p.X),$($p.Y)" }
function Window($proc, $like) { for ($i=0; $i -lt 60; $i++) { $h = Get-Process $proc -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowTitle -like $like } | Select-Object -First 1 -ExpandProperty MainWindowHandle; if ($h) { return $h }; Start-Sleep -Milliseconds 500 } }

# --- Apps ---------------------------------------------------------------------
Get-Process msedge, Code, notepad -ErrorAction SilentlyContinue | Stop-Process -Force
Get-CimInstance Win32_Process -Filter "Name='powershell.exe'" | Where-Object { $_.CommandLine -like '*apps\wpf.ps1*' } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force }
Start-Sleep 2
Start-Process powershell -ArgumentList '-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File C:\JunoSpike\apps\wpf.ps1'
Start-Process msedge -ArgumentList '--remote-debugging-port=9222 --user-data-dir=C:\JunoSpike\edge-cdp --no-first-run --no-default-browser-check --new-window file:///C:/JunoSpike/apps/counter.html'
Start-Process (Join-Path $env:LOCALAPPDATA 'Programs\Microsoft VS Code\Code.exe') -ArgumentList '--new-window C:\JunoSpike\apps'
$wpf = Window powershell 'WPF*'; $edge = Window msedge 'count*'; $code = Window Code '*Visual Studio Code'
Start-Sleep 8
Start-Process notepad.exe; Start-Sleep 3
$n = (Get-Process notepad | Where-Object MainWindowHandle -ne 0 | Select-Object -First 1).MainWindowHandle
[void][P]::MoveWindow($n, 0, 0, 1280, 750, $true); [void][P]::SetForegroundWindow($n); Start-Sleep 2
$np = Fg

# --- CDP ----------------------------------------------------------------------
$page = (Invoke-RestMethod http://127.0.0.1:9222/json) | Where-Object { $_.type -eq 'page' -and $_.title -like 'count*' } | Select-Object -First 1
$ws = New-Object System.Net.WebSockets.ClientWebSocket
$ws.ConnectAsync([Uri]$page.webSocketDebuggerUrl, [Threading.CancellationToken]::None).Wait()
$script:cdpId = 0
function Cdp($method, $params) {
  $script:cdpId++; $id = $script:cdpId
  $bytes = [Text.Encoding]::UTF8.GetBytes((@{ id=$id; method=$method; params=$params } | ConvertTo-Json -Depth 6 -Compress))
  $ws.SendAsync([ArraySegment[byte]]::new($bytes), 'Text', $true, [Threading.CancellationToken]::None).Wait()
  $buf = New-Object byte[] 65536
  while ($true) {
    $sb = New-Object Text.StringBuilder
    do { $r = $ws.ReceiveAsync([ArraySegment[byte]]::new($buf), [Threading.CancellationToken]::None).Result; [void]$sb.Append([Text.Encoding]::UTF8.GetString($buf, 0, $r.Count)) } while (-not $r.EndOfMessage)
    $j = $sb.ToString() | ConvertFrom-Json
    if ($j.id -eq $id) { return $j }
  }
}
function CdpMouse {
  $box = (Cdp 'Runtime.evaluate' @{ expression = 'JSON.stringify(document.querySelector("button").getBoundingClientRect())'; returnByValue = $true }).result.result.value | ConvertFrom-Json
  $x = $box.x + $box.width / 2; $y = $box.y + $box.height / 2
  [void](Cdp 'Input.dispatchMouseEvent' @{ type='mousePressed'; x=$x; y=$y; button='left'; clickCount=1 })
  [void](Cdp 'Input.dispatchMouseEvent' @{ type='mouseReleased'; x=$x; y=$y; button='left'; clickCount=1 })
  'CDP Input.dispatchMouseEvent'
}

# --- Trials -------------------------------------------------------------------
if ($Part -eq 2) {
  $cua = 'C:\JunoSpike\juno-cua.exe'
  foreach ($app in 'Visual Studio Code', 'count') { 1..3 | ForEach-Object { $i = New-Object Diagnostics.ProcessStartInfo $cua, "ui-tree --app `"$app`""; $i.UseShellExecute=$false; $i.CreateNoWindow=$true; $i.RedirectStandardOutput=$true; $p=[Diagnostics.Process]::Start($i); [void]$p.StandardOutput.ReadToEnd(); $p.WaitForExit(); Start-Sleep 1 } }
  1..2 | ForEach-Object { [void][Msaa]::Press($edge, 'no such element'); [void][Msaa]::Press($code, 'no such element'); Start-Sleep 1 }
}
$codeWin = $A::FromHandle($code)
$check = $codeWin.FindFirst('Descendants', (New-Object System.Windows.Automation.PropertyCondition($A::AutomationIdProperty, 'showOnStartup')))
function CheckState { $t=$null; if ($check -and $check.TryGetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern,[ref]$t)) { "$($t.Current.ToggleState)" } else { 'none' } }

function Restore {
  if ([P]::SetForegroundWindow($n)) { Start-Sleep -Milliseconds 200; if ((Fg) -eq $np) { return 'plain' } }
  $pid0 = 0; $t = [P]::GetWindowThreadProcessId([P]::GetForegroundWindow(), [ref]$pid0); $me = [P]::GetCurrentThreadId()
  [void][P]::AttachThreadInput($me, $t, $true); [void][P]::SetForegroundWindow($n); [void][P]::AttachThreadInput($me, $t, $false)
  Start-Sleep -Milliseconds 200; if ((Fg) -eq $np) { return 'attach_thread_input' }
  [P]::keybd_event(0x12, 0, 0, [UIntPtr]::Zero); [P]::keybd_event(0x12, 0, 2, [UIntPtr]::Zero); [void][P]::SetForegroundWindow($n)
  Start-Sleep -Milliseconds 200; if ((Fg) -eq $np) { return 'alt_key' }
  'failed'
}
function Trial($name, [scriptblock]$act, [scriptblock]$effect) {
  $before = Fg; $valid = $before -eq $np; $e0 = & $effect; $c0 = Cursor
  $method = try { & $act } catch { 'error: ' + $_.Exception.Message }
  $clock = [Diagnostics.Stopwatch]::StartNew(); $took = $false
  while ($clock.ElapsedMilliseconds -lt 2000) { if ((Fg) -ne $np) { $took = $true; break }; Start-Sleep -Milliseconds 20 }
  $fgAfter = Fg; Start-Sleep -Milliseconds 300; $e1 = & $effect
  $restore = if ($took) { Restore } else { $null }
  [ordered]@{ case=$name; valid=$valid; method="$method"; worked=("$e0" -ne "$e1"); effect=@("$e0","$e1"); cursor_unchanged=($c0 -eq (Cursor)); took_foreground=$took; took_after_ms=$(if ($took) { $clock.ElapsedMilliseconds }); foreground_after=$fgAfter; restore=$restore }
}
$cases = if ($Part -eq 2) { [ordered]@{
  edge_msaa       = @({ [Msaa]::Press($edge, 'Press me') }, { Title $edge })
  vscode_msaa     = @({ [Msaa]::Press($code, 'When checked, this page will be shown on startup.') }, { CheckState })
} } else { [ordered]@{
  edge_cdp_js     = @({ [void](Cdp 'Runtime.evaluate' @{ expression = 'document.querySelector("button").click()' }); 'CDP Runtime.evaluate click()' }, { Title $edge })
  edge_cdp_mouse  = @({ CdpMouse }, { Title $edge })
  wpf_msaa        = @({ [Msaa]::Press($wpf, 'WPF press') }, { Title $wpf })
  edge_msaa       = @({ [Msaa]::Press($edge, 'Press me') }, { Title $edge })
  vscode_msaa     = @({ [Msaa]::Press($code, 'When checked, this page will be shown on startup.') }, { CheckState })
} }
$trials = @()
foreach ($c in $cases.Keys) {
  for ($i=0; $i -lt 10; $i++) {
    $trials += Trial $c $cases[$c][0] $cases[$c][1]
    if ((Fg) -ne $np) { [void](Restore); Start-Sleep -Milliseconds 500 }
  }
}
$summary = foreach ($c in $cases.Keys) {
  $v = @($trials | Where-Object { $_.case -eq $c -and $_.valid })
  [ordered]@{ case=$c; valid=$v.Count; of=@($trials | Where-Object { $_.case -eq $c }).Count; worked=@($v | Where-Object { $_.worked }).Count; took_foreground=@($v | Where-Object { $_.took_foreground }).Count; cursor_moved=@($v | Where-Object { -not $_.cursor_unchanged }).Count; methods=@($v | ForEach-Object { $_.method } | Sort-Object -Unique) }
}
[ordered]@{ windows=[ordered]@{ edge="$edge"; vscode="$code"; wpf="$wpf"; cdp_page="$($page.title)"; vscode_checkbox_found=[bool]$check }; summary=@($summary); trials=$trials } | ConvertTo-Json -Depth 5 | Set-Content "$dir\trials-cdp-msaa-$Part.json" -Encoding UTF8
Stop-Transcript | Out-Null
'done' | Set-Content "$dir\done.txt"
