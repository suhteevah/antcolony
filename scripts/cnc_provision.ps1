# Provisions cnc-server (192.168.168.100) with the antcolony source
# trimmed to sim-only, then builds the smoke_10yr_ai release binary.
# One-time per source-tree refresh — re-run if simulation.rs changes.
#
# Prereqs:
# - ssh cnc-server works passwordless (or ssh-agent caches the password)
# - cnc has rustc 1.85+ at /root/.cargo/bin (verified during brainstorming)
# - cnc has 8GB swap at /var/swapfile (created during brainstorming)

$ErrorActionPreference = 'Stop'
$LocalRoot = 'J:\antcolony'
$RemoteRoot = '/opt/antcolony'
$RemoteHost = 'cnc-server'

Write-Host "==> Ensuring $RemoteRoot exists on cnc..."
ssh $RemoteHost "sudo mkdir -p $RemoteRoot && sudo chown -R `$(whoami) $RemoteRoot"
if ($LASTEXITCODE -ne 0) { throw "Failed to prepare remote dir" }

Write-Host "==> Building local source tarball (whitelist of source dirs/files)..."
$tarball = "$env:TEMP\antcolony_src_$(Get-Random).tar"
Push-Location $LocalRoot
try {
    # bsdtar's --exclude is unreliable with path patterns; --exclude='./bench'
    # also wipes crates/antcolony-sim/src/bench. Use whitelist instead.
    $included = @(
        'Cargo.toml',
        'Cargo.lock',
        'rust-toolchain.toml',
        'crates',
        'assets',
        'docs',
        'scripts'
    )
    $existing = $included | Where-Object { Test-Path $_ }
    tar -cf $tarball @existing
    if ($LASTEXITCODE -ne 0) { throw "Local tar failed" }
} finally {
    Pop-Location
}
Write-Host "    Tarball size: $([Math]::Round((Get-Item $tarball).Length / 1MB, 2)) MB"

Write-Host "==> Transferring tarball to cnc via scp..."
scp -q $tarball "${RemoteHost}:/tmp/antcolony_src.tar"
if ($LASTEXITCODE -ne 0) { Remove-Item $tarball -ErrorAction SilentlyContinue; throw "scp failed" }
Remove-Item $tarball -ErrorAction SilentlyContinue

Write-Host "==> Extracting on cnc..."
ssh $RemoteHost "tar -xf /tmp/antcolony_src.tar -C $RemoteRoot && rm /tmp/antcolony_src.tar"
if ($LASTEXITCODE -ne 0) { throw "Remote extract failed" }

Write-Host "==> Overwriting rust-toolchain.toml on cnc (project file pins Windows triple)..."
$LinuxToolchain = @'
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
profile = "minimal"
'@
$EncodedToolchain = [System.Convert]::ToBase64String([System.Text.Encoding]::UTF8.GetBytes($LinuxToolchain))
ssh $RemoteHost "echo '$EncodedToolchain' | base64 -d > $RemoteRoot/rust-toolchain.toml"
if ($LASTEXITCODE -ne 0) { throw "Failed to write Linux-friendly rust-toolchain.toml" }

Write-Host "==> Trimming workspace Cargo.toml on cnc to sim-only..."
$TrimmedCargo = @'
[workspace]
resolver = "2"
members = ["crates/antcolony-sim"]

[workspace.package]
edition = "2024"
rust-version = "1.85"
version = "0.1.0"
license = "MIT OR Apache-2.0"

[workspace.dependencies]
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
anyhow = "1"
rand = "0.8"
rand_chacha = "0.3"
glam = { version = "0.29", features = ["serde"] }
rayon = "1.10"
toml = "0.8"
wide = "0.7"

[profile.release]
lto = "thin"
codegen-units = 1
'@

# Write via heredoc on cnc to avoid PowerShell-quoting issues.
$EncodedCargo = [System.Convert]::ToBase64String([System.Text.Encoding]::UTF8.GetBytes($TrimmedCargo))
ssh $RemoteHost "echo '$EncodedCargo' | base64 -d > $RemoteRoot/Cargo.toml"
if ($LASTEXITCODE -ne 0) { throw "Failed to write Cargo.toml" }

Write-Host "==> Verifying trimmed Cargo.toml..."
ssh $RemoteHost "head -20 $RemoteRoot/Cargo.toml"

Write-Host "==> Building smoke_10yr_ai release binary on cnc (this is the long step, ~10-15 min)..."
# Override RUSTC_WRAPPER to bypass cnc's globally-configured sccache
# (its daemon can't start in this environment).
ssh $RemoteHost "cd $RemoteRoot && RUSTC_WRAPPER= CARGO_BUILD_RUSTC_WRAPPER= cargo build --release --example smoke_10yr_ai 2>&1 | tail -40"
if ($LASTEXITCODE -ne 0) { throw "Cargo build on cnc failed" }

Write-Host "==> Verifying binary exists..."
ssh $RemoteHost "ls -lh $RemoteRoot/target/release/examples/smoke_10yr_ai"
if ($LASTEXITCODE -ne 0) { throw "Smoke binary missing after build" }

Write-Host ""
Write-Host "==> cnc provisioned successfully. Smoke binary ready at:" -ForegroundColor Green
Write-Host "    ${RemoteHost}:$RemoteRoot/target/release/examples/smoke_10yr_ai"
