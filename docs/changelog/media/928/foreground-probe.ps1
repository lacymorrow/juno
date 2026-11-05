# Which UIA presses take the foreground? One press per case, foreground read before and after.
Add-Type -TypeDefinition 'using System; using System.Text; using System.Runtime.InteropServices; public static class P { [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow(); [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n); [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr h,int x,int y,int w,int ht,bool r); [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h); [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern IntPtr FindWindow(string c,string t); }'
$cua = 'C:\JunoSpike\juno-cua.exe'
function Fg { $h=[P]::GetForegroundWindow(); $s=New-Object Text.StringBuilder 256; [void][P]::GetWindowText($h,$s,256); $s.ToString() }
function Press($sel) { $i = New-Object Diagnostics.ProcessStartInfo $cua, ('call --tool click --args "{\"selector\":\"' + $sel + '\"}"'); $i.UseShellExecute=$false; $i.CreateNoWindow=$true; $i.RedirectStandardOutput=$true; $i.RedirectStandardError=$true; $p=[Diagnostics.Process]::Start($i); $o=$p.StandardOutput.ReadToEnd()+$p.StandardError.ReadToEnd(); $p.WaitForExit(); $o.Trim() }
$out = [ordered]@{}
Get-Process Calculator, CalculatorApp, notepad, charmap -ErrorAction SilentlyContinue | Stop-Process -Force; Start-Sleep 1
Start-Process calc.exe; Start-Sleep 4
Start-Process charmap.exe; Start-Sleep 3
$cm = [P]::FindWindow([NullString]::Value, 'Character Map'); [void][P]::MoveWindow($cm, 600, 40, 500, 400, $true)
Start-Process notepad.exe; Start-Sleep 3
$n = (Get-Process notepad | Where-Object MainWindowHandle -ne 0 | Select-Object -First 1).MainWindowHandle
[void][P]::MoveWindow($n, 0, 0, 1280, 750, $true); [void][P]::SetForegroundWindow($n); Start-Sleep 2
foreach ($case in @(@('xaml_num7_fully_covered','#num7Button'), @('xaml_num6_second_press','#num6Button'), @('win32_charmap_select','button:Select'))) {
  [void][P]::SetForegroundWindow($n); Start-Sleep 1
  $b = Fg; $r = Press $case[1]; Start-Sleep 1; $a = Fg
  $out[$case[0]] = [ordered]@{ before = $b; after = $a; kept_foreground = ($a -eq $b); result = $r.Substring(0,[Math]::Min(160,$r.Length)) }
}
$out | ConvertTo-Json -Depth 4 | Set-Content C:\JunoSpike\debug\probe.json
'done' | Set-Content C:\JunoSpike\debug\probe-done.txt
