# The Windows spike test (LAC-6321, docs/plans/windows.md, "The spike").
#
# With Notepad in front of Calculator, juno-cua presses 7 x 6 = through UI
# Automation and reads back 42. Pass means the result is 42 and the cursor
# position and foreground window are the same before and after.
#
# Then three measurements with no pass or fail: how long a tree read takes for
# File Explorer and for VS Code (Electron), and whether a covered Explorer
# window will take a rename through UI Automation.
#
# Run it in the logged-on desktop session, not over SSH: a session with no
# desktop has no windows for UI Automation to see. A scheduled task with /IT
# works. Every juno-cua call runs without a console window, so nothing the
# script starts can take the foreground away from Notepad.

param(
  [string]$Cua = (Join-Path $PSScriptRoot 'juno-cua.exe'),
  [string]$Out = (Join-Path $PSScriptRoot 'spike-out')
)
$ErrorActionPreference = 'Stop'
New-Item -ItemType Directory -Force $Out | Out-Null
Start-Transcript (Join-Path $Out 'transcript.txt') -Force | Out-Null

Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
using System.Text;
public static class JunoSpike {
  [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X; public int Y; }
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern bool GetCursorPos(out POINT p);
  [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr h, int x, int y, int w, int height, bool repaint);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr FindWindow(string cls, string title);
}
'@
Add-Type -AssemblyName UIAutomationClient, UIAutomationTypes

function Get-DesktopState {
  $hwnd = [JunoSpike]::GetForegroundWindow()
  $title = New-Object System.Text.StringBuilder 512
  [void][JunoSpike]::GetWindowText($hwnd, $title, 512)
  $point = New-Object JunoSpike+POINT
  [void][JunoSpike]::GetCursorPos([ref]$point)
  [ordered]@{
    foreground_hwnd  = $hwnd.ToInt64()
    foreground_title = $title.ToString()
    cursor           = @($point.X, $point.Y)
  }
}

# Run juno-cua with a raw argument line, so the JSON arguments reach it intact
# on both Windows PowerShell 5.1 and PowerShell 7.
function Invoke-Cua([string]$Arguments) {
  $info = New-Object System.Diagnostics.ProcessStartInfo $Cua, $Arguments
  $info.UseShellExecute = $false
  $info.CreateNoWindow = $true
  $info.RedirectStandardOutput = $true
  $info.RedirectStandardError = $true
  $clock = [Diagnostics.Stopwatch]::StartNew()
  $process = [Diagnostics.Process]::Start($info)
  $stderrTask = $process.StandardError.ReadToEndAsync()
  $stdout = $process.StandardOutput.ReadToEnd()
  $process.WaitForExit()
  [ordered]@{
    args   = $Arguments
    exit   = $process.ExitCode
    ms     = $clock.ElapsedMilliseconds
    stdout = $stdout.Trim()
    stderr = $stderrTask.Result.Trim()
  }
}

function Invoke-CuaTool([string]$Tool, [string]$Selector) {
  Invoke-Cua ('call --tool ' + $Tool + ' --args "{\"selector\":\"' + $Selector + '\"}"')
}

function Wait-Window([scriptblock]$Find) {
  for ($i = 0; $i -lt 80; $i++) {
    $hwnd = & $Find
    if ($hwnd -and $hwnd -ne [IntPtr]::Zero) { return $hwnd }
    Start-Sleep -Milliseconds 250
  }
  throw 'Window did not appear'
}

function Get-NotepadWindow {
  Wait-Window { Get-Process notepad -ErrorAction SilentlyContinue | Where-Object MainWindowHandle -ne 0 | Select-Object -First 1 -ExpandProperty MainWindowHandle }
}

$summary = [ordered]@{ started = (Get-Date).ToString('o'); windows = (Get-CimInstance Win32_OperatingSystem).Caption + ' ' + [Environment]::OSVersion.Version }

# --- The test: Calculator behind Notepad ------------------------------------
Get-Process CalculatorApp, Calculator, notepad -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Seconds 1
Start-Process calc.exe
$calc = Wait-Window { [JunoSpike]::FindWindow($null, 'Calculator') }
[void][JunoSpike]::MoveWindow($calc, 140, 120, 340, 520, $true)
Start-Sleep -Seconds 2
# Notepad covers Calculator's whole keypad, so every button juno-cua presses
# is hidden. Only the display stays in view, so a recording shows the result.
Start-Process notepad.exe
$notepad = Get-NotepadWindow
[void][JunoSpike]::MoveWindow($notepad, 60, 330, 1000, 410, $true)
[void][JunoSpike]::SetForegroundWindow($notepad)
Start-Sleep -Seconds 2

$before = Get-DesktopState
$focusBefore = Invoke-Cua 'focused-element'
$presses = @()
foreach ($id in 'clearButton', 'num7Button', 'multiplyButton', 'num6Button', 'equalButton') {
  $presses += Invoke-CuaTool 'click' "#$id"
  Start-Sleep -Milliseconds 1200  # so each step shows up in a recording
}
$display = Invoke-CuaTool 'get_element_attributes' '#CalculatorResults'
$after = Get-DesktopState
$focusAfter = Invoke-Cua 'focused-element'

$label = ''
try { $label = ($display.stdout | ConvertFrom-Json).label } catch { }

# Each pressed button's bounds, from juno-cua, must sit inside Notepad's
# rectangle (60,330 to 1060,740), so none of them was ever visible.
$buttonBounds = [ordered]@{}
foreach ($id in 'num7Button', 'multiplyButton', 'num6Button', 'equalButton') {
  try { $buttonBounds[$id] = @(((Invoke-CuaTool 'get_element_attributes' "#$id").stdout | ConvertFrom-Json).properties.bounds) } catch { $buttonBounds[$id] = $null }
}
$underNotepad = foreach ($b in $buttonBounds.Values) {
  $b -and $b[0] -ge 60 -and $b[1] -ge 330 -and ($b[0] + $b[2]) -le 1060 -and ($b[1] + $b[3]) -le 740
}
$checks = [ordered]@{
  every_press_succeeded   = -not ($presses | Where-Object { $_.exit -ne 0 })
  display_reads_42        = $label -match '\b42$'
  cursor_unchanged        = ($before.cursor -join ',') -eq ($after.cursor -join ',')
  foreground_unchanged    = $before.foreground_hwnd -eq $after.foreground_hwnd
  notepad_was_in_front    = $before.foreground_hwnd -eq $notepad.ToInt64()
  buttons_under_notepad   = -not ($underNotepad -contains $false)
}
$summary.test = [ordered]@{
  passed       = -not ($checks.Values -contains $false)
  checks       = $checks
  display      = $label
  button_bounds = $buttonBounds
  before       = $before
  after        = $after
  focus_before = $focusBefore
  focus_after  = $focusAfter
  presses      = $presses
  read_display = $display
}

# --- Measurement: File Explorer tree read, and a rename while covered --------
$folder = 'C:\JunoSpike\Rename Test'
Remove-Item $folder -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $folder | Out-Null
1..30 | ForEach-Object { Set-Content (Join-Path $folder ("file-{0:D2}.txt" -f $_)) "file $_" }
New-Item -ItemType Directory -Force (Join-Path $folder 'rename-me') | Out-Null
Start-Process explorer.exe $folder
$explorer = Wait-Window { [JunoSpike]::FindWindow('CabinetWClass', $null) }
[void][JunoSpike]::MoveWindow($explorer, 160, 140, 820, 560, $true)
Start-Sleep -Seconds 3
$explorerReads = @(1..3 | ForEach-Object { Invoke-Cua 'ui-tree --app "Rename Test"' })

[void][JunoSpike]::SetForegroundWindow($notepad)
Start-Sleep -Seconds 1
$renameBefore = Get-DesktopState
$rename = [ordered]@{ attempted = $false }
try {
  $A = [System.Windows.Automation.AutomationElement]
  $root = $A::RootElement
  $window = $root.FindFirst('Children', (New-Object System.Windows.Automation.PropertyCondition($A::NameProperty, 'Rename Test')))
  $item = $window.FindFirst('Descendants', (New-Object System.Windows.Automation.PropertyCondition($A::NameProperty, 'rename-me')))
  $rename.item_patterns = @($item.GetSupportedPatterns() | ForEach-Object { $_.ProgrammaticName })
  $target = $null
  $valuePattern = $null
  if ($item.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$valuePattern)) { $target = 'item' }
  else {
    $edit = $item.FindFirst('Descendants', (New-Object System.Windows.Automation.PropertyCondition($A::ControlTypeProperty, [System.Windows.Automation.ControlType]::Edit)))
    if ($edit -and $edit.TryGetCurrentPattern([System.Windows.Automation.ValuePattern]::Pattern, [ref]$valuePattern)) { $target = 'name cell' }
  }
  $rename.value_target = $target
  if ($valuePattern) {
    $rename.read_only = $valuePattern.Current.IsReadOnly
    $rename.attempted = $true
    try { $valuePattern.SetValue('renamed-by-uia') } catch { $rename.set_value_error = $_.Exception.Message }
    Start-Sleep -Seconds 1
  }
} catch { $rename.error = $_.Exception.Message }
$rename.renamed = Test-Path (Join-Path $folder 'renamed-by-uia')
$rename.before = $renameBefore
$rename.after = Get-DesktopState
$summary.explorer = [ordered]@{ tree_reads = $explorerReads | ForEach-Object { $j = $_.stdout | ConvertFrom-Json; [ordered]@{ elapsed_ms = $j.elapsed_ms; node_count = $j.node_count; process_ms = $_.ms } }; rename = $rename }

# --- Measurement: VS Code (Electron) tree read ------------------------------
$code = Join-Path $env:LOCALAPPDATA 'Programs\Microsoft VS Code\Code.exe'
if (Test-Path $code) {
  Start-Process $code -ArgumentList '--new-window', $folder
  [void](Wait-Window { Get-Process Code -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowTitle -like '*Visual Studio Code*' } | Select-Object -First 1 -ExpandProperty MainWindowHandle })
  Start-Sleep -Seconds 8
  [void][JunoSpike]::SetForegroundWindow($notepad)
  Start-Sleep -Seconds 1
  $codeReads = @(1..3 | ForEach-Object { Invoke-Cua 'ui-tree --app "Visual Studio Code"'; Start-Sleep -Seconds 1 })
  $summary.vscode = @($codeReads | ForEach-Object { $j = $_.stdout | ConvertFrom-Json; [ordered]@{ elapsed_ms = $j.elapsed_ms; node_count = $j.node_count; process_ms = $_.ms } })
} else {
  $summary.vscode = 'VS Code is not installed'
}

$summary.finished = (Get-Date).ToString('o')
$summary | ConvertTo-Json -Depth 12 | Set-Content (Join-Path $Out 'result.json') -Encoding UTF8
Stop-Transcript | Out-Null
'done' | Set-Content (Join-Path $Out 'done.txt')
