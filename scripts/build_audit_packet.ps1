# Substitutes {{INSERT <path>}} placeholders in docs/audit-packet-gemini.md
# with the actual file contents. Output: docs/audit-packet-gemini.rendered.md
#
# Run before sending the packet to an external auditor (Gemini, etc.).

$ErrorActionPreference = 'Stop'
Set-Location J:\antcolony

$src = 'docs\audit-packet-gemini.md'
$dst = 'docs\audit-packet-gemini.rendered.md'

if (-not (Test-Path $src)) {
    throw "Template not found: $src"
}

$content = Get-Content $src -Raw -Encoding utf8
$pattern = '\{\{INSERT (.+?)\}\}'

$rendered = [regex]::Replace($content, $pattern, {
    param($m)
    $relPath = $m.Groups[1].Value.Trim()
    $absPath = Join-Path 'J:\antcolony' $relPath
    if (Test-Path $absPath) {
        $body = Get-Content $absPath -Raw -Encoding utf8
        # Trim trailing newline so the surrounding ``` block formatting stays clean.
        return $body.TrimEnd("`r","`n")
    } else {
        return "[MISSING FILE: $relPath]"
    }
})

# Capture git rev so the packet is reproducible.
$gitRev = (git rev-parse HEAD).Trim()
$header = "<!-- Audit packet rendered $(Get-Date -Format o) at git $gitRev -->`r`n"

Set-Content -Path $dst -Value ($header + $rendered) -Encoding utf8
Write-Output "Rendered: $dst ($(((Get-Item $dst).Length) / 1024) KB)"
