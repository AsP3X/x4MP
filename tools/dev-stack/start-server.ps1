#Requires -Version 5.1
# Human: Start x4mp-server in Docker Compose and wait until /health is green.
# Agent: CALLS docker compose; READS X4MP_HOST_PORT from env.example.
param(
    [switch]$Build,
    [switch]$NoBuild
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Resolve-Path (Join-Path $Root "..\..")
$EnvFile = Join-Path $Root "env.example"

if (Test-Path (Join-Path $Root ".env")) {
    Get-Content (Join-Path $Root ".env") | ForEach-Object {
        if ($_ -match '^\s*([^=]+)=(.*)$' -and $_ -notmatch '^\s*#') {
            [System.Environment]::SetEnvironmentVariable($Matches[1].Trim(), $Matches[2].Trim(), "Process")
        }
    }
} elseif (Test-Path $EnvFile) {
    Get-Content $EnvFile | ForEach-Object {
        if ($_ -match '^\s*([^=]+)=(.*)$' -and $_ -notmatch '^\s*#') {
            [System.Environment]::SetEnvironmentVariable($Matches[1].Trim(), $Matches[2].Trim(), "Process")
        }
    }
}

$Port = if ($env:X4MP_HOST_PORT) { $env:X4MP_HOST_PORT } else { "7878" }
Push-Location $RepoRoot
try {
    $Args = @("compose", "up", "-d")
    if ($Build -or -not $NoBuild) { $Args += "--build" }
    & docker @Args
    if ($LASTEXITCODE -ne 0) { throw "docker compose up failed with exit code $LASTEXITCODE" }

    Write-Host "Waiting for x4mp-server health on port $Port..."
    $deadline = (Get-Date).AddSeconds(90)
    while ((Get-Date) -lt $deadline) {
        try {
            $resp = Invoke-WebRequest -Uri "http://127.0.0.1:$Port/health" -UseBasicParsing -TimeoutSec 2
            if ($resp.StatusCode -eq 200) {
                Write-Host "x4mp-server is healthy (http://127.0.0.1:$Port/health)"
                Write-Host "WebSocket endpoint: ws://127.0.0.1:$Port/ws"
                return
            }
        } catch {
            Start-Sleep -Seconds 2
        }
    }
    throw "Timed out waiting for x4mp-server health check."
} finally {
    Pop-Location
}
