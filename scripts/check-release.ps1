# Guard for release artifacts. Checks three things:
#   1. the exe exists
#   2. its FileVersion matches package.json
#   3. the frontend assets are embedded (otherwise the app loads devUrl and shows a white screen)
# Usage: powershell -ExecutionPolicy Bypass -File scripts/check-release.ps1 [-Exe <path>]
param(
  [string]$Exe = ""
)

$ErrorActionPreference = "Stop"
. "$PSScriptRoot\env.ps1"

if (-not $Exe) {
  $Exe = Join-Path $env:CARGO_TARGET_DIR "release\taskcap.exe"
}

if (-not (Test-Path -LiteralPath $Exe)) {
  throw "build artifact not found: $Exe (run npm run tauri:build first)"
}

$package = Get-Content -LiteralPath "package.json" -Encoding utf8 | ConvertFrom-Json
$expected = $package.version
$actual = (Get-Item -LiteralPath $Exe).VersionInfo.FileVersion
if ($actual -ne $expected) {
  throw "version mismatch: package.json=$expected, exe=$actual"
}

# Latin1 maps bytes 1:1, so a plain substring search works on the binary.
# Without this marker the frontend assets were not embedded and the app
# would try to reach devUrl (127.0.0.1:5181) and show a white screen.
$bytes = [System.IO.File]::ReadAllBytes($Exe)
$latin1 = [System.Text.Encoding]::GetEncoding(28591)
$text = $latin1.GetString($bytes)
if ($text.IndexOf("assets/index-") -lt 0) {
  throw "frontend assets are NOT embedded: this is a devUrl build and must not be released. Rebuild with npm run tauri:build (custom-protocol)."
}

$size = [math]::Round((Get-Item -LiteralPath $Exe).Length / 1MB, 2)
Write-Host "release binary ok: $Exe (v$actual, $size MB, frontend assets embedded)"
