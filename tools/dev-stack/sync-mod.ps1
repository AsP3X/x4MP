#Requires -Version 5.1
# Human: Deploy the spike mod (extension/spike) into one or more X4 install extensions folders.
# Human: Default mode COPIES the files; -Junction makes each install point at the repo (instant live edits).
# Agent: READS X4MP_X4_EXTENSIONS_DIR from .env/env.example (semicolon-separated); WRITES <dir>\x4mp_spike.
# Agent: -Junction DELETES an existing plain copy at the destination first (reproducible from repo), then
# Agent: creates a directory junction so the install tracks the repo with no re-copy (fixes stale-copy drift).
# Agent: Failure modes: missing source, no extensions dir configured, no write permission to the X4 install.
param(
    # Replace the destination with a directory junction to the repo instead of copying files.
    [switch]$Junction
)

$ErrorActionPreference = "Stop"
$Root = Split-Path -Parent $MyInvocation.MyCommand.Path
# The mod source = repo extension/spike (this script lives in tools/dev-stack).
$Source = (Resolve-Path (Join-Path $Root "..\..\extension\spike")).Path
$ModName = "x4mp_spike"

if (-not (Test-Path $Source)) {
    throw "Mod source not found: $Source"
}

# Resolve target extensions dir(s): .env wins over env.example; semicolon-separated for multiple installs.
$ExtDirsRaw = $null
foreach ($path in @((Join-Path $Root ".env"), (Join-Path $Root "env.example"))) {
    if (-not (Test-Path $path)) { continue }
    Get-Content $path | ForEach-Object {
        if ($_ -match '^\s*X4MP_X4_EXTENSIONS_DIR=(.+)$') {
            $ExtDirsRaw = $Matches[1].Trim()
        }
    }
    if ($ExtDirsRaw) { break }
}

if (-not $ExtDirsRaw) {
    throw "X4MP_X4_EXTENSIONS_DIR not set. Add it to tools/dev-stack/.env (see env.example)."
}

$ExtDirs = $ExtDirsRaw.Split(';') | ForEach-Object { $_.Trim() } | Where-Object { $_ }

foreach ($extDir in $ExtDirs) {
    if (-not (Test-Path $extDir)) {
        Write-Warning "Extensions dir not found, skipping: $extDir"
        continue
    }
    $dst = Join-Path $extDir $ModName

    if ($Junction) {
        $existing = Get-Item $dst -ErrorAction SilentlyContinue
        if ($existing -and $existing.LinkType -eq "Junction") {
            Write-Host "Junction already present: $dst -> $($existing.Target)"
            continue
        }
        if ($existing) {
            # Reproducible mod copy — safe to remove before re-pointing at the repo.
            Remove-Item -Path $dst -Recurse -Force
        }
        New-Item -ItemType Junction -Path $dst -Target $Source | Out-Null
        Write-Host "Junction created: $dst -> $Source"
    }
    else {
        # robocopy /E copies all files/subdirs and overwrites changed ones; it does NOT delete extras.
        # Exit codes 0-7 are success (8+ are failures); normalize so the script doesn't false-fail.
        robocopy $Source $dst /E /NFL /NDL /NJH /NJS | Out-Null
        if ($LASTEXITCODE -ge 8) {
            throw "robocopy failed ($LASTEXITCODE) copying to $dst"
        }
        $global:LASTEXITCODE = 0
        Write-Host "Copied mod -> $dst"
    }
}

Write-Host "sync-mod complete."
