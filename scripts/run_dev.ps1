$ErrorActionPreference = 'Stop'
$env:RUST_LOG = 'antcolony=debug,antcolony_sim=debug,antcolony_game=info,antcolony_render=info,wgpu=warn'
Set-Location -Path (Join-Path $PSScriptRoot '..')
cargo run --release
