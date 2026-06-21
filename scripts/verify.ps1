$ErrorActionPreference = "Stop"
. "$PSScriptRoot\env.ps1"

Write-Host "== TaskCap verify =="

Push-Location (Join-Path $PWD "src-tauri")
cargo test
if ($LASTEXITCODE -ne 0) { Pop-Location; exit $LASTEXITCODE }
Pop-Location

npm run build
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$installer = Join-Path $env:CARGO_TARGET_DIR "release\bundle\nsis\TaskCap_0.1.0_x64-setup.exe"
if (Test-Path -LiteralPath $installer) {
  $size = (Get-Item -LiteralPath $installer).Length
  Write-Host "NSIS installer: $installer ($size bytes)"
} else {
  Write-Host "NSIS installer not built yet. Run: npm run tauri:build"
}

Write-Host "verify ok"
