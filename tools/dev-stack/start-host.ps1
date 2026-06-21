#Requires -Version 5.1
# Human: Full Windows dev stack — Docker server + SirNuke pipe server + x4mp-bridge.
# Agent: CALLS start-server.ps1, setup-pipe-server.ps1; RUNS pipe server + bridge on host.
param(
    [switch]$SkipPipeServer,
    [switch]$SkipBridge,
    [switch]$Rebuild
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Resolve-Path (Join-Path $Root "..\..")
$EnvFile = Join-Path $Root "env.example"
$PipePidFile = Join-Path $Root ".cache\pipe-server.pid"

function Import-DevEnv {
    foreach ($path in @((Join-Path $Root ".env"), $EnvFile)) {
        if (-not (Test-Path $path)) { continue }
        Get-Content $path | ForEach-Object {
            if ($_ -match '^\s*([^=]+)=(.*)$' -and $_ -notmatch '^\s*#') {
                [System.Environment]::SetEnvironmentVariable($Matches[1].Trim(), $Matches[2].Trim(), "Process")
            }
        }
    }
}

Import-DevEnv

Write-Host "==> Starting x4mp-server (Docker)"
if ($Rebuild) {
    & (Join-Path $Root "start-server.ps1") -Build
} else {
    & (Join-Path $Root "start-server.ps1") -NoBuild
}

$pipeProc = $null
if (-not $SkipPipeServer) {
    Write-Host "==> SirNuke pipe server (https://github.com/bvbohnen/x4-projects/releases)"
    $exePath = & (Join-Path $Root "setup-pipe-server.ps1")
    if (Test-Path $PipePidFile) {
        $oldPid = Get-Content $PipePidFile -ErrorAction SilentlyContinue
        $existing = $null
        if ($oldPid) {
            $existing = Get-Process -Id $oldPid -ErrorAction SilentlyContinue
        }
        if ($null -ne $existing) {
            Write-Host "Pipe server already running (PID $oldPid)"
        } else {
            $pipeProc = Start-Process -FilePath $exePath -WorkingDirectory (Split-Path $exePath) -PassThru
            $pipeProc.Id | Set-Content $PipePidFile
            Write-Host "Started pipe server PID $($pipeProc.Id)"
        }
    } else {
        $pipeProc = Start-Process -FilePath $exePath -WorkingDirectory (Split-Path $exePath) -PassThru
        $pipeProc.Id | Set-Content $PipePidFile
        Write-Host "Started pipe server PID $($pipeProc.Id)"
    }
    Write-Host "After loading a save: Main Menu -> Extension Options -> Named pipes api (pipe server status)."
}

if ($SkipBridge) {
    Write-Host "Bridge skipped. Run manually:"
    Write-Host "  cd `"$RepoRoot`""
    Write-Host "  `$env:X4MP_SERVER_URL='ws://127.0.0.1:7878/ws'"
    Write-Host "  cargo run -p x4mp-bridge"
    return
}

Write-Host "==> Starting x4mp-bridge (named pipe x4mp_bridge -> Docker server)"
Write-Host "    Server event log: docker logs -f x4mp-server"
Write-Host "    Expect spike.capture.started then spike.ship_sample after Start capture in X4."
Push-Location $RepoRoot
try {
    if (-not $env:X4MP_SERVER_URL) {
        $env:X4MP_SERVER_URL = "ws://127.0.0.1:7878/ws"
    }
    if (-not $env:X4MP_PIPE_NAME) {
        $env:X4MP_PIPE_NAME = "x4mp_bridge"
    }
    if (-not $env:X4MP_GAME_VERSION) {
        $env:X4MP_GAME_VERSION = "9.00"
    }
    if (-not $env:RUST_LOG) {
        $env:RUST_LOG = "x4mp_bridge=info,x4mp_server=info"
    }
    cargo run -p x4mp-bridge
} finally {
    Pop-Location
    if (($null -ne $pipeProc) -and (-not $pipeProc.HasExited)) {
        Write-Host "Stopping pipe server PID $($pipeProc.Id)"
        Stop-Process -Id $pipeProc.Id -Force -ErrorAction SilentlyContinue
        Remove-Item $PipePidFile -ErrorAction SilentlyContinue
    }
}
