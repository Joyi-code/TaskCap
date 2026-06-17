# 启动 release taskcap.exe 并在多个时间点截屏，用于检查启动透明边框闪烁
$ErrorActionPreference = "Stop"
. "$PSScriptRoot\env.ps1"
$exe = Join-Path $env:CARGO_TARGET_DIR "release\taskcap.exe"
$outDir = Join-Path $PSScriptRoot "..\verify-startup"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

Add-Type -AssemblyName System.Windows.Forms
Add-Type -AssemblyName System.Drawing

function Save-Screenshot([string]$path) {
  $screen = [System.Windows.Forms.Screen]::PrimaryScreen
  $bmp = New-Object System.Drawing.Bitmap $screen.Bounds.Width, $screen.Bounds.Height
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.CopyFromScreen($screen.Bounds.Location, [System.Drawing.Point]::Empty, $screen.Bounds.Size)
  $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
  $g.Dispose()
  $bmp.Dispose()
}

$proc = Start-Process -FilePath $exe -PassThru
$times = @(0, 150, 400, 800, 1500, 2500)
foreach ($ms in $times) {
  if ($ms -gt 0) { Start-Sleep -Milliseconds $ms }
  $file = Join-Path $outDir ("startup_{0}ms.png" -f $ms)
  Save-Screenshot $file
}

if (-not $proc.HasExited) {
  Write-Output "taskcap_running=true pid=$($proc.Id)"
} else {
  Write-Output "taskcap_running=false exit=$($proc.ExitCode)"
}

Write-Output "screenshots=$outDir"