# LAC-6321 trials, part 2: Edge (Chromium), VS Code (Electron), WPF and
# WinForms, each pressed through juno-cua from behind Notepad. Same rules as
# foreground-trials.ps1: a trial counts only if Notepad is in front when it
# starts, and a press counts only if its effect shows up. The apps are opened
# by discover.ps1 first. With -Part 2 it runs the follow-up cases instead: a
# press from this long-running process rather than from juno-cua, which exits
# right after, and BM_CLICK on the WinForms button. -Part 3 reruns juno-cua on
# WinForms and WPF after the engine started using BM_CLICK for WinForms.
param([int]$Part = 1)
Add-Type -TypeDefinition 'using System; using System.Text; using System.Runtime.InteropServices; public static class P { [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow(); [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern IntPtr FindWindowEx(IntPtr p, IntPtr a, string c, string t); [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n); [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr h,int x,int y,int w,int ht,bool r); [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h); [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l); [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint p); [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId(); [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint a, uint b, bool f); [DllImport("user32.dll")] public static extern void keybd_event(byte k, byte s, uint f, UIntPtr e); }'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$A = [System.Windows.Automation.AutomationElement]
$cua = 'C:\JunoSpike\juno-cua.exe'
$dir = "C:\JunoSpike\trials-apps-$Part"; New-Item -ItemType Directory -Force $dir | Out-Null
Start-Transcript "$dir\transcript.txt" -Force | Out-Null
function Title($h) { $s=New-Object Text.StringBuilder 256; [void][P]::GetWindowText($h,$s,256); $s.ToString() }
function Fg { Title ([P]::GetForegroundWindow()) }
function Cua([string]$a) { $i = New-Object Diagnostics.ProcessStartInfo $cua, $a; $i.UseShellExecute=$false; $i.CreateNoWindow=$true; $i.RedirectStandardOutput=$true; $i.RedirectStandardError=$true; $p=[Diagnostics.Process]::Start($i); $o=$p.StandardOutput.ReadToEnd(); [void]$p.StandardError.ReadToEnd(); $p.WaitForExit(); $o }
function Press($sel) { $o = Cua ('call --tool click --args "{\"selector\":\"' + $sel + '\"}"'); if ($o -match 'Method: ([^,]+)') { $Matches[1] } else { 'failed: ' + $o.Trim() } }
function Window($proc, $like) { Get-Process $proc -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowTitle -like $like } | Select-Object -First 1 -ExpandProperty MainWindowHandle }

Get-Process | Where-Object MainWindowTitle | Format-Table Id, ProcessName, MainWindowTitle -AutoSize | Out-String -Width 200
$edge = Window msedge 'count*'; $code = Window Code '*Visual Studio Code'; $wpf = Window powershell 'WPF*'; $forms = Window powershell 'Forms*'
Get-Process notepad -ErrorAction SilentlyContinue | Stop-Process -Force; Start-Sleep 1
Start-Process notepad.exe; Start-Sleep 3
$n = (Get-Process notepad | Where-Object MainWindowHandle -ne 0 | Select-Object -First 1).MainWindowHandle
[void][P]::MoveWindow($n, 0, 0, 1280, 750, $true); [void][P]::SetForegroundWindow($n); Start-Sleep 2
$np = Fg

# Chromium builds its accessibility tree on first request, so read each tree
# a few times while covered, the way Juno would before acting.
1..3 | ForEach-Object { [void](Cua 'ui-tree --app "Visual Studio Code"'); [void](Cua 'ui-tree --app "count"'); Start-Sleep 1 }

$codeWin = $A::FromHandle($code)
$check = $codeWin.FindFirst('Descendants', (New-Object System.Windows.Automation.PropertyCondition($A::AutomationIdProperty, 'showOnStartup')))
function CheckState { $t=$null; if ($check -and $check.TryGetCurrentPattern([System.Windows.Automation.TogglePattern]::Pattern,[ref]$t)) { "$($t.Current.ToggleState)" } else { 'none' } }
$formsBtnHwnd = [P]::FindWindowEx($forms, [IntPtr]::Zero, [NullString]::Value, 'Forms press')
function ByName($h, $name) { $A::FromHandle($h).FindFirst('Descendants', (New-Object System.Windows.Automation.PropertyCondition($A::NameProperty, $name))) }
$wpfBtn = ByName $wpf 'WPF press'; $edgeBtn = ByName $edge 'Press me'
function InProc($e) { $e.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern).Invoke(); 'UIA Invoke (from this process)' }

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
  $before = Fg; $valid = $before -eq $np; $e0 = & $effect
  $method = & $act
  $clock = [Diagnostics.Stopwatch]::StartNew(); $took = $false
  while ($clock.ElapsedMilliseconds -lt 2000) { if ((Fg) -ne $np) { $took = $true; break }; Start-Sleep -Milliseconds 20 }
  $fgAfter = Fg; Start-Sleep -Milliseconds 300; $e1 = & $effect
  $restore = if ($took) { Restore } else { $null }
  [ordered]@{ case=$name; valid=$valid; method=$method; worked=("$e0" -ne "$e1"); effect=@("$e0","$e1"); took_foreground=$took; took_after_ms=$(if ($took) { $clock.ElapsedMilliseconds }); foreground_after=$fgAfter; restore=$restore }
}

$cases = if ($Part -eq 3) { [ordered]@{
  forms_juno_cua = @({ Press 'button:Forms press' }, { Title $forms })
  wpf_juno_cua   = @({ Press 'button:WPF press' }, { Title $wpf })
} } elseif ($Part -eq 2) { [ordered]@{
  wpf_inproc_invoke  = @({ InProc $wpfBtn }, { Title $wpf })
  edge_inproc_invoke = @({ InProc $edgeBtn }, { Title $edge })
  forms_bm_click     = @({ [void][P]::PostMessage($formsBtnHwnd, 0x00F5, [IntPtr]::Zero, [IntPtr]::Zero); 'BM_CLICK (posted by the script)' }, { Title $forms })
} } else { [ordered]@{
  edge_juno_cua   = @({ Press 'button:Press me' }, { Title $edge })
  vscode_juno_cua = @({ Press '#showOnStartup' }, { CheckState })
  wpf_juno_cua    = @({ Press 'button:WPF press' }, { Title $wpf })
  forms_juno_cua  = @({ Press 'button:Forms press' }, { Title $forms })
  forms_bm_click  = @({ [void][P]::PostMessage($formsBtnHwnd, 0x00F5, [IntPtr]::Zero, [IntPtr]::Zero); 'BM_CLICK (posted by the script)' }, { Title $forms })
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
  [ordered]@{ case=$c; valid=$v.Count; of=@($trials | Where-Object { $_.case -eq $c }).Count; worked=@($v | Where-Object { $_.worked }).Count; took_foreground=@($v | Where-Object { $_.took_foreground }).Count; methods=@($v | ForEach-Object { $_.method } | Sort-Object -Unique) }
}
[ordered]@{ windows=[ordered]@{ edge="$edge"; vscode="$code"; wpf="$wpf"; forms="$forms"; forms_button="$formsBtnHwnd"; vscode_checkbox_found=[bool]$check }; summary=@($summary); trials=$trials } | ConvertTo-Json -Depth 5 | Set-Content "$dir\trials-apps-$Part.json" -Encoding UTF8
Stop-Transcript | Out-Null
'done' | Set-Content "$dir\done.txt"
