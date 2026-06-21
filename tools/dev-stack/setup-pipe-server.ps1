#Requires -Version 5.1
# Human: Download and install SirNuke X4 Python Pipe Server from GitHub releases.
# Agent: READS env.example URLs; WRITES tools/dev-stack/.cache/pipe-server/ + permissions.json.
param(
    [switch]$Force
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $MyInvocation.MyCommand.Path
$CacheDir = Join-Path $Root ".cache\pipe-server"
$InstallDir = Join-Path $CacheDir "current"

$DownloadUrl = "https://github.com/bvbohnen/x4-projects/releases/download/v1.12/sn_x4_python_pipe_server_exe_v1.4.3.zip"
$AssetName = "sn_x4_python_pipe_server_exe_v1.4.3.zip"

foreach ($path in @((Join-Path $Root ".env"), (Join-Path $Root "env.example"))) {
    if (-not (Test-Path $path)) { continue }
    Get-Content $path | ForEach-Object {
        if ($_ -match '^\s*X4MP_PIPE_SERVER_DOWNLOAD_URL=(.+)$') {
            $DownloadUrl = $Matches[1].Trim()
        }
        if ($_ -match '^\s*X4MP_PIPE_SERVER_ASSET=(.+)$') {
            $AssetName = $Matches[1].Trim()
        }
    }
}

New-Item -ItemType Directory -Force -Path $CacheDir | Out-Null
$ZipPath = Join-Path $CacheDir $AssetName

if ($Force -or -not (Test-Path $InstallDir)) {
    Write-Host "Downloading SirNuke pipe server from $DownloadUrl"
    Invoke-WebRequest -Uri $DownloadUrl -OutFile $ZipPath -UseBasicParsing
    if (Test-Path $InstallDir) { Remove-Item $InstallDir -Recurse -Force }
    Expand-Archive -Path $ZipPath -DestinationPath $InstallDir -Force
}

$Exe = Get-ChildItem -Path $InstallDir -Recurse -Filter "X4_Python_Pipe_Server.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $Exe) {
    $Exe = Get-ChildItem -Path $InstallDir -Recurse -Filter "*.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
}
if (-not $Exe) {
    throw "No pipe server .exe found under $InstallDir - check the GitHub release asset."
}

$PermissionsSrc = Join-Path $Root "permissions.json"
$PermissionsDst = Join-Path $Exe.DirectoryName "permissions.json"
if (-not (Test-Path $PermissionsDst) -or $Force) {
    Copy-Item $PermissionsSrc $PermissionsDst -Force
    Write-Host "Wrote permissions.json -> $($Exe.DirectoryName)"
}

Write-Host "Pipe server ready: $($Exe.FullName)"
Write-Output $Exe.FullName
