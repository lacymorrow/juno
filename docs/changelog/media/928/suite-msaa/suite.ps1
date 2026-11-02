# Runs every juno-cua trial against the current build: Calculator, Character
# Map, then Edge, VS Code, WPF and WinForms.
Get-CimInstance Win32_Process -Filter "Name='powershell.exe'" | Where-Object { $_.CommandLine -like '*apps\wpf.ps1*' -or $_.CommandLine -like '*apps\forms.ps1*' } | ForEach-Object { Stop-Process -Id $_.ProcessId -Force }
Get-Process msedge -ErrorAction SilentlyContinue | Stop-Process -Force
& C:\JunoSpike\foreground-trials.ps1
& C:\JunoSpike\apps\discover.ps1 2>$null
& C:\JunoSpike\apps\trials-apps.ps1 -Part 1
'done' | Set-Content C:\JunoSpike\suite-done.txt
