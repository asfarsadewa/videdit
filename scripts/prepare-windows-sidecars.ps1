[CmdletBinding()]
param(
  [string]$Version = "8.0.1",
  [string]$TargetTriple = "x86_64-pc-windows-msvc",
  [string]$DownloadUrl = "",
  [switch]$Force
)

$ErrorActionPreference = "Stop"

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$repoRoot = Resolve-Path (Join-Path $scriptDir "..")
$binariesDir = Join-Path $repoRoot "src-tauri\binaries"
$ffmpegSidecar = Join-Path $binariesDir "ffmpeg-$TargetTriple.exe"
$ffprobeSidecar = Join-Path $binariesDir "ffprobe-$TargetTriple.exe"

if (-not $DownloadUrl) {
  $DownloadUrl = "https://www.gyan.dev/ffmpeg/builds/packages/ffmpeg-$Version-essentials_build.zip"
}

function Show-ToolVersion {
  param([string]$Path)

  $versionLine = & $Path -version | Select-Object -First 1
  Write-Host $versionLine
}

if ((Test-Path $ffmpegSidecar) -and (Test-Path $ffprobeSidecar) -and -not $Force) {
  Write-Host "FFmpeg sidecars already exist in $binariesDir"
  Show-ToolVersion $ffmpegSidecar
  Show-ToolVersion $ffprobeSidecar
  exit 0
}

New-Item -ItemType Directory -Path $binariesDir -Force | Out-Null

$tempRoot = if ($env:RUNNER_TEMP) { $env:RUNNER_TEMP } else { [System.IO.Path]::GetTempPath() }
$workDir = Join-Path $tempRoot ("videdit-ffmpeg-" + [guid]::NewGuid().ToString("N"))
$archivePath = Join-Path $workDir "ffmpeg.zip"
$extractDir = Join-Path $workDir "extract"

New-Item -ItemType Directory -Path $workDir -Force | Out-Null

try {
  Write-Host "Downloading FFmpeg sidecars from $DownloadUrl"
  Invoke-WebRequest -Uri $DownloadUrl -OutFile $archivePath

  Expand-Archive -Path $archivePath -DestinationPath $extractDir -Force

  $ffmpegExe = Get-ChildItem -Path $extractDir -Recurse -Filter "ffmpeg.exe" |
    Where-Object { $_.FullName -match "\\bin\\ffmpeg\.exe$" } |
    Select-Object -First 1
  $ffprobeExe = Get-ChildItem -Path $extractDir -Recurse -Filter "ffprobe.exe" |
    Where-Object { $_.FullName -match "\\bin\\ffprobe\.exe$" } |
    Select-Object -First 1

  if (-not $ffmpegExe) {
    throw "ffmpeg.exe was not found in the downloaded archive."
  }

  if (-not $ffprobeExe) {
    throw "ffprobe.exe was not found in the downloaded archive."
  }

  Copy-Item -LiteralPath $ffmpegExe.FullName -Destination $ffmpegSidecar -Force
  Copy-Item -LiteralPath $ffprobeExe.FullName -Destination $ffprobeSidecar -Force

  Write-Host "Installed FFmpeg sidecars in $binariesDir"
  Show-ToolVersion $ffmpegSidecar
  Show-ToolVersion $ffprobeSidecar
}
finally {
  if (Test-Path $workDir) {
    Remove-Item -LiteralPath $workDir -Recurse -Force
  }
}
