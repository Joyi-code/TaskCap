# 截取屏幕顶部中央区域，聚焦灵动岛启动表现
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\env.ps1"
$exe = Join-Path $env:CARGO_TARGET_DIR "release\taskcap.exe"
$outDir = Join-Path $PSScriptRoot "..\verify-startup"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

function Save-TopCrop([string]$path) {
  $screen = [System.Windows.Forms.Screen]::PrimaryScreen
  $w = [Math]::Min(900, $screen.Bounds.Width)
  $h = 120
  $x = [int](($screen.Bounds.Width - $w) / 2)
  $y = 0
  $bmp = New-Object System.Drawing.Bitmap $w, $h
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($x, $y, 0, 0, (New-Object System.Drawing.Size $w, $h))
  $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose(); $bmp.Dispose()
}

Get-Process taskcap -ErrorAction SilentlyContinue | Stop-Process -Force
Start-Sleep -Milliseconds 300
$proc = Start-Process -FilePath $exe -PassThru
$marks = @(50, 200, 500, 1000, 2000, 3500)
$elapsed = 0
foreach ($target in $marks) {
  $wait = $target - $elapsed
  if ($wait -gt 0) { Start-Sleep -Milliseconds $wait }
  $elapsed = $target
  Save-TopCrop (Join-Path $outDir ("top_{0}ms.png" -f $target))
}
Write-Output "pid=$($proc.Id) alive=$([bool](Get-Process -Id $proc.Id -ErrorAction SilentlyContinue))"