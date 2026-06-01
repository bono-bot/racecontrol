# rotate-pod-toml.ps1 — replace sentry_service_key in rc-agent.toml on a pod.
# Safe-pattern replacement for the BANNED inline-PS-over-/exec Set-Content
# (RCA 2026-06-01 .planning/audits/RCA-windows-registry-powershell-deploy-2026-06-01.md §5 P1).
# Delivered to the pod as a base64 blob via /exec (cmd-safe; no var/quote mangling) then run -File.
# Committed (NOT heredoc) so .gitattributes enforces CRLF.
param(
  [Parameter(Mandatory=$true)][string]$Sentry
)
$ErrorActionPreference = 'Stop'
$f = 'C:\RacingPoint\rc-agent.toml'
if (-not (Test-Path $f)) { Write-Error "VERIFY_FAIL: $f not found"; exit 1 }
$c = Get-Content $f -Raw
$c = $c -replace 'sentry_service_key = "[^"]*"', ("sentry_service_key = `"" + $Sentry + "`"")
Set-Content $f $c -NoNewline
$check = Get-Content $f -Raw
if ($check -match 'sentry_service_key = "') {
  Write-Host 'VERIFY_OK: pod rc-agent.toml sentry_service_key updated + re-readable'
} else {
  Write-Error 'VERIFY_FAIL: post-write read-back missing sentry_service_key'; exit 1
}
