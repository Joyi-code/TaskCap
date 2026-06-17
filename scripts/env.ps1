$ErrorActionPreference = "Stop"

$project = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot "..")).Path
$root = Split-Path -Parent $project

if (-not $env:CODEX_DEV_ROOT) { $env:CODEX_DEV_ROOT = $root }
if (-not $env:RUSTUP_HOME) { $env:RUSTUP_HOME = Join-Path $root ".rustup" }
if (-not $env:CARGO_HOME) { $env:CARGO_HOME = Join-Path $root ".cargo" }
if (-not $env:npm_config_cache) { $env:npm_config_cache = Join-Path $root ".npm-cache" }
if (-not $env:npm_config_prefix) { $env:npm_config_prefix = Join-Path $root ".npm-global" }
if (-not $env:TEMP) { $env:TEMP = Join-Path $root ".tmp" }
if (-not $env:TMP) { $env:TMP = Join-Path $root ".tmp" }
# 避免 src-tauri/target 被安全软件锁导致 build-script os error 5
if (-not $env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR = Join-Path $root ".build\taskcap-target" }

New-Item -ItemType Directory -Force `
  -Path $env:RUSTUP_HOME, $env:CARGO_HOME, $env:npm_config_cache, $env:npm_config_prefix, $env:TEMP, $env:CARGO_TARGET_DIR, $project `
  | Out-Null

$cargoBin = Join-Path $env:CARGO_HOME "bin"
if (Test-Path -LiteralPath $cargoBin) {
  $env:PATH = "$cargoBin;$env:PATH"
} elseif (Test-Path -LiteralPath "$env:USERPROFILE\.cargo\bin\cargo.exe") {
  $env:CARGO_HOME = Join-Path $env:USERPROFILE ".cargo"
  $env:RUSTUP_HOME = Join-Path $env:USERPROFILE ".rustup"
  $env:PATH = "$env:CARGO_HOME\bin;$env:RUSTUP_HOME\toolchains\stable-x86_64-pc-windows-msvc\bin;$env:PATH"
}

if (Test-Path -LiteralPath $env:npm_config_prefix) {
  $env:PATH = "$env:npm_config_prefix;$env:PATH"
}

Set-Location -LiteralPath $project
