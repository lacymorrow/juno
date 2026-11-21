# A WPF window with one button that counts presses in the title.
Add-Type -AssemblyName PresentationFramework
$w = New-Object System.Windows.Window
$w.Title = 'WPF 0'; $w.Width = 300; $w.Height = 200; $w.Left = 100; $w.Top = 100; $w.WindowStartupLocation = 'Manual'
$b = New-Object System.Windows.Controls.Button
$b.Content = 'WPF press'
$script:n = 0
$b.Add_Click({ $script:n++; $w.Title = "WPF $script:n" })
$w.Content = $b
[void]$w.ShowDialog()
