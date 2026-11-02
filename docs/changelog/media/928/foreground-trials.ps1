# Repeated trials for LAC-6321: which press keeps the foreground, and can a
# background process hand it back? A trial counts only if Notepad is in front
# when it starts. Each press is checked by its effect, not just its return code.
# Handback tries three ways in order and records the first that works: plain
# SetForegroundWindow, AttachThreadInput to the foreground thread, then a
# synthetic Alt key (which makes this process the last to get input).
Add-Type -TypeDefinition 'using System; using System.Text; using System.Runtime.InteropServices; public static class P { [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow(); [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n); [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr h,int x,int y,int w,int ht,bool r); [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h); [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string c,string t); [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l); [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint p); [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId(); [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint a, uint b, bool f); [DllImport("user32.dll")] public static extern void keybd_event(byte k, byte s, uint f, UIntPtr e); }'
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes
$A = [System.Windows.Automation.AutomationElement]
$cua = 'C:\JunoSpike\juno-cua.exe'
$dir = 'C:\JunoSpike\trials'; New-Item -ItemType Directory -Force $dir | Out-Null
function Fg { $h=[P]::GetForegroundWindow(); $s=New-Object Text.StringBuilder 256; [void][P]::GetWindowText($h,$s,256); $s.ToString() }
function Press($sel) { $i = New-Object Diagnostics.ProcessStartInfo $cua, ('call --tool click --args "{\"selector\":\"' + $sel + '\"}"'); $i.UseShellExecute=$false; $i.CreateNoWindow=$true; $i.RedirectStandardOutput=$true; $i.RedirectStandardError=$true; $p=[Diagnostics.Process]::Start($i); [void]$p.StandardOutput.ReadToEnd(); [void]$p.StandardError.ReadToEnd(); $p.WaitForExit(); $p.ExitCode }
function Child($parent, $prop, $value) { $parent.FindFirst('Descendants', (New-Object System.Windows.Automation.PropertyCondition($prop, $value))) }

Get-Process Calculator, CalculatorApp, notepad, charmap, Code -ErrorAction SilentlyContinue | Stop-Process -Force; Start-Sleep 1
Start-Process calc.exe; Start-Sleep 4
Start-Process charmap.exe; Start-Sleep 3
Start-Process notepad.exe; Start-Sleep 3
$n = (Get-Process notepad | Where-Object MainWindowHandle -ne 0 | Select-Object -First 1).MainWindowHandle
[void][P]::MoveWindow($n, 0, 0, 1280, 750, $true); [void][P]::SetForegroundWindow($n); Start-Sleep 2
$np = Fg

$cm = $A::RootElement.FindFirst('Children', (New-Object System.Windows.Automation.PropertyCondition($A::NameProperty, 'Character Map')))
$calc = $A::RootElement.FindFirst('Children', (New-Object System.Windows.Automation.PropertyCondition($A::NameProperty, 'Calculator')))
$select = Child $cm $A::NameProperty 'Select'
$selectHwnd = [IntPtr]$select.Current.NativeWindowHandle
$copy = Child $cm $A::AutomationIdProperty '104'
$display = Child $calc $A::AutomationIdProperty 'CalculatorResults'
function CopyLen { $v=$null; if ($copy -and $copy.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern,[ref]$v)) { $v.Current.Value.Length } else { -1 } }

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
  $code = & $act
  $clock = [Diagnostics.Stopwatch]::StartNew(); $took = $false
  while ($clock.ElapsedMilliseconds -lt 1000) { if ((Fg) -ne $np) { $took = $true; break }; Start-Sleep -Milliseconds 20 }
  $fgAfter = Fg; Start-Sleep -Milliseconds 300; $e1 = & $effect
  $restore = if ($took) { Restore } else { $null }
  [ordered]@{ case=$name; valid=$valid; before=$before; exit=$code; worked=("$e0" -ne "$e1"); took_foreground=$took; took_after_ms=$(if ($took) { $clock.ElapsedMilliseconds }); foreground_after=$fgAfter; restore=$restore }
}

$trials = @()
for ($i=0; $i -lt 10; $i++) {
  $d = if ($i % 2) { '#num6Button' } else { '#num7Button' }
  $trials += Trial 'xaml_uia_invoke' { Press $d } { $display.Current.Name }
  if ((Fg) -ne $np) { [void](Restore); Start-Sleep -Milliseconds 500 }
}
for ($i=0; $i -lt 10; $i++) {
  $trials += Trial 'win32_uia_invoke' { Press 'button:Select' } { CopyLen }
  if ((Fg) -ne $np) { [void](Restore); Start-Sleep -Milliseconds 500 }
}
for ($i=0; $i -lt 10; $i++) {
  $trials += Trial 'win32_bm_click' { [void][P]::PostMessage($selectHwnd, 0x00F5, [IntPtr]::Zero, [IntPtr]::Zero); 0 } { CopyLen }
  if ((Fg) -ne $np) { [void](Restore); Start-Sleep -Milliseconds 500 }
}
$summary = $trials | Group-Object { $_.case } | ForEach-Object {
  $v = @($_.Group | Where-Object { $_.valid })
  [ordered]@{ case=$_.Name; valid=$v.Count; of=$_.Count; worked=@($v|Where-Object worked).Count; took_foreground=@($v|Where-Object took_foreground).Count; restore=@($v | Where-Object restore | Group-Object restore | ForEach-Object { "$($_.Name)=$($_.Count)" }) }
}
[ordered]@{ select_hwnd=$selectHwnd.ToInt64(); copy_field_found=[bool]$copy; summary=@($summary); trials=$trials } | ConvertTo-Json -Depth 5 | Set-Content "$dir\foreground-trials.json" -Encoding UTF8
'done' | Set-Content "$dir\done.txt"
