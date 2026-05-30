# Ship antcolony-sim + antcolony-trainer + assets (sans the 4.2M assets/gen
# sprites) + the stripped cnc manifest to cnc-server, unpack to
# /opt/antcolony-cuda, and stage Cargo.toml. Build is driven separately so it
# can be backgrounded + monitored.
#
# Uses Windows bsdtar (System32\tar.exe) which handles J:\ drive paths that
# Git-Bash GNU tar mangles as host:path.
$ErrorActionPreference = "Stop"

$repo    = "J:\antcolony"
$tarexe  = "$env:SystemRoot\System32\tar.exe"
$tar     = "$env:TEMP\antcolony-cuda-ship.tar"
$remote  = "cnc-server"
$dest    = "/opt/antcolony-cuda"

if (Test-Path $tar) { Remove-Item $tar }

# Pack sources + needed assets (species TOMLs are include_str!'d at compile
# time, so they MUST be present during the build). Exclude assets/gen.
& $tarexe -cf $tar -C $repo --exclude="assets/gen" `
    crates/antcolony-sim crates/antcolony-trainer assets
if ($LASTEXITCODE -ne 0) { throw "tar pack failed ($LASTEXITCODE)" }

# Append the stripped manifest from scripts/.
& $tarexe -rf $tar -C "$repo\scripts" Cargo.cnc-trainer.toml
if ($LASTEXITCODE -ne 0) { throw "tar append manifest failed ($LASTEXITCODE)" }

$bytes = (Get-Item $tar).Length
Write-Output "Packed $bytes bytes -> $tar"

scp $tar "${remote}:/tmp/antcolony-cuda-ship.tar"
if ($LASTEXITCODE -ne 0) { throw "scp failed ($LASTEXITCODE)" }

# Unpack remotely + stage Cargo.toml.
$remoteCmd = @"
set -e
rm -rf $dest
mkdir -p $dest
tar -xf /tmp/antcolony-cuda-ship.tar -C $dest
mv $dest/Cargo.cnc-trainer.toml $dest/Cargo.toml
rm -f $dest/Cargo.lock
echo '--- staged tree ---'
ls $dest
ls $dest/crates
echo SHIP_OK
"@
ssh $remote $remoteCmd
if ($LASTEXITCODE -ne 0) { throw "remote unpack failed ($LASTEXITCODE)" }
Write-Output "DONE"
