$ErrorActionPreference = 'Stop'
Set-Location -Path (Join-Path $PSScriptRoot '..')

$exe = (Resolve-Path 'target/release/examples/smoke_10yr_ai.exe').Path
$out = 'bench/smoke-10yr-ai'
New-Item -ItemType Directory -Force -Path "$out/_logs" | Out-Null

$species = @(
    'aphaenogaster_rudis',
    'camponotus_pennsylvanicus',
    'formica_fusca',
    'formica_rufa',
    'lasius_niger',
    'pogonomyrmex_occidentalis',
    'tapinoma_sessile',
    'tetramorium_immigrans'
)

$pids = @()
foreach ($sp in $species) {
    $log = (Resolve-Path $out).Path + "\_logs\$sp.log"
    $args = @('--years', '2', '--no-mlp', '--species', $sp, '--out', $out)
    $p = Start-Process -FilePath $exe -ArgumentList $args `
        -WorkingDirectory (Resolve-Path '.').Path `
        -RedirectStandardOutput $log -RedirectStandardError "$log.err" `
        -WindowStyle Hidden -PassThru
    Write-Host "spawned $sp pid=$($p.Id) log=$log"
    $pids += [pscustomobject]@{ species = $sp; pid = $p.Id; log = $log }
}

$pids | ConvertTo-Json | Set-Content -Encoding utf8 "$out/_logs/pids.json"
Write-Host "wrote $out/_logs/pids.json"
