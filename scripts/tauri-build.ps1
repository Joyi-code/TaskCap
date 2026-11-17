. "$PSScriptRoot\env.ps1"
. "$PSScriptRoot\vs-dev-env.ps1"

$code = Invoke-InVsDevShell -Command "npx tauri build" -WorkingDirectory $PWD.Path
if ($code -ne 0) { exit $code }

# 打包完成后再校验一次产物：确保不是「会连 127.0.0.1 的白屏包」
& "$PSScriptRoot\check-release.ps1"
if ($LASTEXITCODE -ne 0) { exit 1 }
exit 0
