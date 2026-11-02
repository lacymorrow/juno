# Open each test app, then record what juno-cua and UIA see for its button.
$o = 'C:\JunoSpike\discover'; New-Item -ItemType Directory -Force $o | Out-Null
$cua = 'C:\JunoSpike\juno-cua.exe'
Get-Process msedge, Code -ErrorAction SilentlyContinue | Stop-Process -Force; Start-Sleep 2
Start-Process powershell -ArgumentList '-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File C:\JunoSpike\apps\wpf.ps1'
Start-Process powershell -ArgumentList '-NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -File C:\JunoSpike\apps\forms.ps1'
Start-Process msedge -ArgumentList '--no-first-run --no-default-browser-check --new-window file:///C:/JunoSpike/apps/counter.html'
Start-Process (Join-Path $env:LOCALAPPDATA 'Programs\Microsoft VS Code\Code.exe') -ArgumentList '--new-window C:\JunoSpike\apps'
Start-Sleep 15
foreach ($s in 'button:Press me', 'button:WPF press', 'button:Forms press') {
  & $cua call --tool get_element_attributes --args ('{"selector":"' + $s + '"}') 2>&1 | Out-File "$o\attr-$($s -replace '[^a-zA-Z]','').txt" -Encoding utf8
}
& $cua ui-tree --app 'Visual Studio Code' 2>&1 | Out-File "$o\vscode-tree.json" -Encoding utf8
& $cua ui-tree --app 'count' 2>&1 | Out-File "$o\edge-tree.json" -Encoding utf8
Get-Process | Where-Object MainWindowTitle | Select-Object Id, ProcessName, MainWindowTitle | Out-File "$o\windows.txt"
'done' | Set-Content "$o\done.txt"
