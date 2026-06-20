# Durable backup of all trained-brain checkpoints. bench/ is gitignored and
# local-only, so a wiped bench/ or a dead kokonoe loses every trained brain.
# This copies every *.safetensors (HAC checkpoints) + the v1 MLP baseline to an
# out-of-repo archive on kokonoe, records SHA256 provenance, and (optionally)
# mirrors the archive to cnc-server for an off-machine copy.
#
# Run after EVERY training run (part of the "no training forgotten" discipline).
# Idempotent: re-copies only when the source SHA differs from the manifest.
#
#   powershell -ExecutionPolicy Bypass -File scripts/backup_checkpoints.ps1
#   ... -NoCnc            # skip the cnc mirror
param(
    [string]$Repo       = "J:\antcolony",
    [string]$ArchiveRoot = "J:\antcolony-archive\checkpoints",
    [string]$CncHost    = "cnc-server",
    [string]$CncDest    = "/opt/antcolony-archive/checkpoints",
    [switch]$NoCnc
)
$ErrorActionPreference = "Stop"
$bench = Join-Path $Repo "bench"
New-Item -ItemType Directory -Force -Path $ArchiveRoot | Out-Null

# Collect the artifacts worth preserving: every HAC checkpoint + the v1 MLP.
$files = @()
$files += Get-ChildItem -Path $bench -Recurse -Filter *.safetensors -File
$v1 = Join-Path $bench "iterative-fsp\round_1\mlp_weights_v1.json"
if (Test-Path $v1) { $files += Get-Item $v1 }

$manifestPath = Join-Path $ArchiveRoot "MANIFEST.csv"
$rows = @()
$copied = 0; $skipped = 0
foreach ($f in $files) {
    $rel = $f.FullName.Substring($bench.Length).TrimStart('\','/')
    $dest = Join-Path $ArchiveRoot $rel
    New-Item -ItemType Directory -Force -Path (Split-Path $dest) | Out-Null
    $srcHash = (Get-FileHash -Algorithm SHA256 -Path $f.FullName).Hash
    $needCopy = $true
    if (Test-Path $dest) {
        $dstHash = (Get-FileHash -Algorithm SHA256 -Path $dest).Hash
        if ($dstHash -eq $srcHash) { $needCopy = $false }
    }
    if ($needCopy) { Copy-Item -Path $f.FullName -Destination $dest -Force; $copied++ }
    else { $skipped++ }
    $rows += [pscustomobject]@{
        rel_path = ($rel -replace '\\','/')
        bytes    = $f.Length
        sha256   = $srcHash
        mtime    = $f.LastWriteTime.ToString("s")
    }
}
$rows | Sort-Object rel_path | Export-Csv -Path $manifestPath -NoTypeInformation -Encoding utf8
Write-Output "archive: $ArchiveRoot"
Write-Output "checkpoints: $($files.Count) total | copied $copied | unchanged $skipped"
Write-Output "manifest: $manifestPath ($($rows.Count) rows)"

if (-not $NoCnc) {
    Write-Output "--- mirroring archive to ${CncHost}:${CncDest} ---"
    ssh $CncHost "mkdir -p $CncDest"
    # scp -r preserves the run-subdir structure; safetensors + manifest only.
    scp -r "$ArchiveRoot\*" "${CncHost}:${CncDest}/"
    if ($LASTEXITCODE -ne 0) { throw "cnc mirror failed ($LASTEXITCODE)" }
    Write-Output "cnc mirror OK -> ${CncHost}:${CncDest}"
}
Write-Output "DONE"
