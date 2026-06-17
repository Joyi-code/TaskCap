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
