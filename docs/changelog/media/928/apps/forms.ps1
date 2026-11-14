# A WinForms window with one button that counts presses in the title.
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
$f = New-Object System.Windows.Forms.Form
$f.Text = 'Forms 0'; $f.StartPosition = 'Manual'; $f.Location = New-Object System.Drawing.Point(450, 100); $f.Size = New-Object System.Drawing.Size(300, 200)
$b = New-Object System.Windows.Forms.Button
$b.Text = 'Forms press'; $b.Dock = 'Fill'
$script:n = 0
$b.Add_Click({ $script:n++; $f.Text = "Forms $script:n" })
$f.Controls.Add($b)
[void]$f.ShowDialog()
