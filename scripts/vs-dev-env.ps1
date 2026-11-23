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

  $vsDevCmd = Get-VsDevCmdPath
  $winsdk = Get-PortableWinSdk
  if ($vsDevCmd -or $winsdk) {
    $prelude = ""
    if ($winsdk) {
      # When the system SDK is missing, use the portable nuget SDK for INCLUDE/LIB.
      $prelude = @"
set "INCLUDE=$($winsdk.Include)"
set "LIB=$($winsdk.Lib)"
set "PATH=$($winsdk.Path)"
set "RC=$($winsdk.Rc)"
"@
    }
    $callVs = if ($vsDevCmd) { "call `"$vsDevCmd`" -arch=x64 -host_arch=x64 >nul" } else { "" }
    $cmd = @"
$callVs
$prelude
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

# Portable Windows SDK: D:\codex\winsdk\env.ps1 provides INCLUDE/LIB/PATH/RC.
# TASKCAP_WINSDK_ENV overrides the path; returns $null when not found (use the system SDK).
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
