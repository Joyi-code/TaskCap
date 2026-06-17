. "$PSScriptRoot\env.ps1"
. "$PSScriptRoot\vs-dev-env.ps1"

exit (Invoke-InVsDevShell -Command "cargo check" -WorkingDirectory (Join-Path $PWD "src-tauri"))
