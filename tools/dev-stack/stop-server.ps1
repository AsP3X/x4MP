#Requires -Version 5.1
# Human: Stop Docker Compose stack for x4mp-server.
# Agent: CALLS docker compose down; does not remove the x4mp-data volume.
$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Resolve-Path (Join-Path $Root "..\..")
Push-Location $RepoRoot
try {
    docker compose down
} finally {
    Pop-Location
}
