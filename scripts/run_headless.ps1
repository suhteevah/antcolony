$ErrorActionPreference = 'Stop'
$env:RUST_LOG = 'antcolony_sim=info'
Set-Location -Path (Join-Path $PSScriptRoot '..')
cargo test --workspace
