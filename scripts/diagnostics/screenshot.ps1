Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

# Get full virtual screen bounds
$left = [System.Windows.Forms.SystemInformation]::VirtualScreen.Left
$top = [System.Windows.Forms.SystemInformation]::VirtualScreen.Top
$width = [System.Windows.Forms.SystemInformation]::VirtualScreen.Width
$height = [System.Windows.Forms.SystemInformation]::VirtualScreen.Height

Write-Output "Virtual screen: ${width}x${height} at (${left},${top})"

# Capture full virtual screen
$bitmap = New-Object System.Drawing.Bitmap($width, $height)
$graphics = [System.Drawing.Graphics]::FromImage($bitmap)
$graphics.CopyFromScreen($left, $top, 0, 0, (New-Object System.Drawing.Size($width, $height)))
$graphics.Dispose()

$timestamp = Get-Date -Format 'yyyy-MM-dd_HHmmss'

# Save full screenshot
$fullPath = "C:\Users\bono\Pictures\Screenshots\full_${timestamp}.png"
$bitmap.Save($fullPath, [System.Drawing.Imaging.ImageFormat]::Png)
Write-Output "Full: $fullPath"

# Crop right half (DeskIn window)
$halfWidth = [int]($width / 2)
$cropRect = New-Object System.Drawing.Rectangle($halfWidth, 0, $halfWidth, $height)
$cropped = $bitmap.Clone($cropRect, $bitmap.PixelFormat)
$rightPath = "C:\Users\bono\Pictures\Screenshots\right_${timestamp}.png"
$cropped.Save($rightPath, [System.Drawing.Imaging.ImageFormat]::Png)
Write-Output "Right half: $rightPath"

$cropped.Dispose()
$bitmap.Dispose()
