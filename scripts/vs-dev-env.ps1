$ErrorActionPreference = "Stop"

function Get-VsDevCmdPath {
  if ($env:VSDEVCMD_PATH -and (Test-Path -LiteralPath $env:VSDEVCMD_PATH)) {
    return $env:VSDEVCMD_PATH
  }

  $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
  if (Test-Path -LiteralPath $vswhere) {
    $installPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if ($installPath) {
      $candidate = Join-Path $installPath "Common7\Tools\VsDevCmd.bat"
      if (Test-Path -LiteralPath $candidate) {
        return $candidate
      }
    }
  }

  $candidates = @(
    (Join-Path ${env:ProgramFiles} "Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat"),
    (Join-Path ${env:ProgramFiles} "Microsoft Visual Studio\2022\Community\Common7\Tools\VsDevCmd.bat"),
    (Join-Path ${env:ProgramFiles} "Microsoft Visual Studio\2022\Professional\Common7\Tools\VsDevCmd.bat"),
    (Join-Path ${env:ProgramFiles} "Microsoft Visual Studio\2022\Enterprise\Common7\Tools\VsDevCmd.bat"),
    (Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\2022\BuildTools\Common7\Tools\VsDevCmd.bat")
  )

  foreach ($candidate in $candidates) {
    if (Test-Path -LiteralPath $candidate) {
      return $candidate
    }
  }

  return $null
}

function Invoke-InVsDevShell {
  param(
    [Parameter(Mandatory = $true)]
    [string] $Command,
    [string] $WorkingDirectory = $PWD.Path
  )

  $toolchain = Get-WinToolchain
  if (-not $toolchain) {
    $toolchain = Get-PortableWinSdk
  }

  if ($toolchain) {
    # Set the whole toolchain explicitly: VsDevCmd relies on SDK registry entries
    # that may be missing even when the SDK files are installed.
    $cmd = @"
set "INCLUDE=$($toolchain.Include)"
set "LIB=$($toolchain.Lib)"
set "PATH=$($toolchain.Path);%PATH%"
set "RC=$($toolchain.Rc)"
cd /d "$WorkingDirectory"
$Command
"@

    $cmdFile = Join-Path $env:TEMP "taskcap-vs-dev.cmd"
    Set-Content -LiteralPath $cmdFile -Value $cmd -Encoding ASCII
    cmd /c $cmdFile
    return $LASTEXITCODE
  }

  $vsDevCmd = Get-VsDevCmdPath
  if ($vsDevCmd) {
    $cmd = @"
call "$vsDevCmd" -arch=x64 -host_arch=x64 >nul
cd /d "$WorkingDirectory"
$Command
"@

    $cmdFile = Join-Path $env:TEMP "taskcap-vs-dev.cmd"
    Set-Content -LiteralPath $cmdFile -Value $cmd -Encoding ASCII
    cmd /c $cmdFile
    return $LASTEXITCODE
  }

  Push-Location -LiteralPath $WorkingDirectory
  try {
    Invoke-Expression $Command
    if ($null -eq $LASTEXITCODE) {
      return 0
    }
    return $LASTEXITCODE
  } finally {
    Pop-Location
  }
}

# Installed Windows SDK paths. Returns $null when Lib/Include are incomplete.
# Set TASKCAP_FORCE_PORTABLE_SDK=1 to skip it and use the portable copy.
function Get-SystemSdkPaths {
  if ($env:TASKCAP_FORCE_PORTABLE_SDK -eq "1") { return $null }
  $roots = @(
    "${env:ProgramFiles(x86)}\Windows Kits\10",
    "$env:ProgramFiles\Windows Kits\10"
  )
  foreach ($root in $roots) {
    $libRoot = Join-Path $root "Lib"
    $incRoot = Join-Path $root "Include"
    $binRoot = Join-Path $root "bin"
    if (-not (Test-Path -LiteralPath $libRoot) -or -not (Test-Path -LiteralPath $incRoot)) { continue }
    $versions = Get-ChildItem -LiteralPath $libRoot -Directory -ErrorAction SilentlyContinue |
      Where-Object { $_.Name -match '^10\.' } | Sort-Object Name -Descending
    foreach ($version in $versions) {
      $ver = $version.Name
      $lib = Join-Path $libRoot "$ver\um\x64\OleAut32.lib"
      $inc = Join-Path $incRoot "$ver\um\windows.h"
      if (-not (Test-Path -LiteralPath $lib) -or -not (Test-Path -LiteralPath $inc)) { continue }
      return @{
        Include = "$incRoot\$ver\ucrt;$incRoot\$ver\um;$incRoot\$ver\shared;$incRoot\$ver\winrt"
        Lib     = "$libRoot\$ver\ucrt\x64;$libRoot\$ver\um\x64"
        Path    = "$binRoot\$ver\x64"
        Rc      = "$binRoot\$ver\x64\rc.exe"
      }
    }
  }
  return $null
}

# MSVC compiler paths from the latest Visual Studio Build Tools install.
function Get-MsvcPaths {
  $vsDevCmd = Get-VsDevCmdPath
  if (-not $vsDevCmd) { return $null }
  $tools = Split-Path -Parent $vsDevCmd
  $common7 = Split-Path -Parent $tools
  $root = Split-Path -Parent $common7
  $msvcRoot = Join-Path $root "VC\Tools\MSVC"
  if (-not (Test-Path -LiteralPath $msvcRoot)) { return $null }
  $versions = Get-ChildItem -LiteralPath $msvcRoot -Directory -ErrorAction SilentlyContinue |
    Sort-Object Name -Descending
  foreach ($version in $versions) {
    $bin = Join-Path $version.FullName "bin\Hostx64\x64"
    if (-not (Test-Path -LiteralPath (Join-Path $bin "link.exe"))) { continue }
    return @{
      Bin     = $bin
      Include = Join-Path $version.FullName "include"
      Lib     = Join-Path $version.FullName "lib\x64"
    }
  }
  return $null
}

# Full toolchain: system MSVC + system Windows SDK. Null when either is unusable.
function Get-WinToolchain {
  $sdk = Get-SystemSdkPaths
  $msvc = Get-MsvcPaths
  if (-not $sdk -or -not $msvc) { return $null }

  $pathParts = @($msvc.Bin, $sdk.Path)
  foreach ($cargoHome in @($env:CARGO_HOME, "D:\codex\.cargo", (Join-Path $env:USERPROFILE ".cargo"))) {
    if (-not $cargoHome) { continue }
    $bin = Join-Path $cargoHome "bin"
    if (Test-Path -LiteralPath (Join-Path $bin "cargo.exe")) { $pathParts += $bin }
  }

  Write-Host "toolchain: system MSVC + Windows SDK"
  return @{
    Include = "$($msvc.Include);$($sdk.Include)"
    Lib     = "$($msvc.Lib);$($sdk.Lib)"
    Path    = ($pathParts -join ";")
    Rc      = $sdk.Rc
  }
}

# Portable Windows SDK: D:\codex\winsdk\env.ps1 provides INCLUDE/LIB/PATH/RC.
# TASKCAP_WINSDK_ENV overrides the path; returns $null when not found.
function Get-PortableWinSdk {
  $candidates = @()
  if ($env:TASKCAP_WINSDK_ENV) { $candidates += $env:TASKCAP_WINSDK_ENV }
  $candidates += "D:\codex\winsdk\env.ps1"

  foreach ($path in $candidates) {
    if (-not (Test-Path -LiteralPath $path)) { continue }
    $probe = @"
. '$path' *> `$null
'INC|' + `$env:INCLUDE
'LIB|' + `$env:LIB
'PATH|' + `$env:PATH
'RC|' + `$env:RC
"@
    $lines = & powershell -NoProfile -Command $probe 2>$null
    $values = @{}
    foreach ($line in $lines) {
      if ($line -match '^(INC|LIB|PATH|RC)\|(.*)$') {
        $values[$Matches[1]] = $Matches[2]
      }
    }
    if ($values.ContainsKey("INC") -and $values.ContainsKey("LIB")) {
      Write-Host "portable Windows SDK: $path"
      return @{
        Include = $values["INC"]
        Lib     = $values["LIB"]
        Path    = $values["PATH"]
        Rc      = $values["RC"]
      }
    }
  }
  return $null
}
