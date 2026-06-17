. "$PSScriptRoot\env.ps1"
. "$PSScriptRoot\vs-dev-env.ps1"

exit (Invoke-InVsDevShell -Command "npx tauri dev" -WorkingDirectory $PWD.Path)
