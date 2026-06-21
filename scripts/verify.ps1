$ErrorActionPreference = "Stop"
. "$PSScriptRoot\env.ps1"

Write-Host "== TaskCap verify =="

$package = Get-Content -LiteralPath "package.json" -Encoding utf8 | ConvertFrom-Json
$version = $package.version
$requiredFiles = @(
  "README.md",
  "RELEASE_NOTES.md",
  "LICENSE",
  "screenshots\overview.png",
  "screenshots\wechat-qrcode.png",
  "src-tauri\windows\installer-hooks.nsh"
)

foreach ($path in $requiredFiles) {
  if (-not (Test-Path -LiteralPath $path)) {
    throw "Required release file missing: $path"
  }
}

$packageLockVersion = (Select-String -LiteralPath "package-lock.json" -Pattern '^  "version": "([^"]+)",$').Matches[0].Groups[1].Value
$tauriConfig = Get-Content -LiteralPath "src-tauri\tauri.conf.json" -Encoding utf8 | ConvertFrom-Json
$cargoVersion = (Select-String -LiteralPath "src-tauri\Cargo.toml" -Pattern '^version = "([^"]+)"$').Matches[0].Groups[1].Value
$frontendVersion = (Select-String -LiteralPath "src\version.ts" -Pattern 'APP_VERSION = "v([^"]+)"').Matches[0].Groups[1].Value

$versions = @{
  "package-lock.json" = $packageLockVersion
  "src-tauri/tauri.conf.json" = $tauriConfig.version
  "src-tauri/Cargo.toml" = $cargoVersion
  "src/version.ts" = $frontendVersion
}
foreach ($entry in $versions.GetEnumerator()) {
  if ($entry.Value -ne $version) {
    throw "Version mismatch: package.json=$version, $($entry.Key)=$($entry.Value)"
  }
}

Write-Host "release files ok; version=$version"

Push-Location (Join-Path $PWD "src-tauri")
cargo test
if ($LASTEXITCODE -ne 0) { Pop-Location; exit $LASTEXITCODE }
Pop-Location

npm run build
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$installer = Join-Path $env:CARGO_TARGET_DIR "release\bundle\nsis\TaskCap_${version}_x64-setup.exe"
if (Test-Path -LiteralPath $installer) {
  $size = (Get-Item -LiteralPath $installer).Length
  $sha256 = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash
  Write-Host "NSIS installer: $installer ($size bytes)"
  Write-Host "SHA256: $sha256"
} else {
  throw "NSIS installer missing: $installer. Run npm run tauri:build"
}

Write-Host "verify ok"
