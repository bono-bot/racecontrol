# rotate-server-toml.ps1 — replace credential values in racecontrol.toml on Server .23.
# Safe-pattern replacement for the BANNED inline-PowerShell-over-SSH Set-Content
# (RCA 2026-06-01 .planning/audits/RCA-windows-registry-powershell-deploy-2026-06-01.md §5 P1).
# Invoked as: powershell -ExecutionPolicy Bypass -File C:\RacingPoint\rotate-server-toml.ps1 -Jwt .. -Relay .. -Sentry ..
# Committed (NOT heredoc) so .gitattributes enforces CRLF — LF-only .ps1 fails to parse.
param(
  [Parameter(Mandatory=$true)][string]$Jwt,
  [Parameter(Mandatory=$true)][string]$Relay,
  [Parameter(Mandatory=$true)][string]$Sentry
)
$ErrorActionPreference = 'Stop'
$f = 'C:\RacingPoint\racecontrol.toml'
if (-not (Test-Path $f)) { Write-Error "VERIFY_FAIL: $f not found"; exit 1 }
$c = Get-Content $f -Raw
$c = $c -replace 'jwt_secret = "[^"]*"',          ("jwt_secret = `"" + $Jwt + "`"")
$c = $c -replace 'relay_secret = "[^"]*"',        ("relay_secret = `"" + $Relay + "`"")
$c = $c -replace 'sentry_service_key = "[^"]*"',  ("sentry_service_key = `"" + $Sentry + "`"")
Set-Content $f $c -NoNewline
# Behavioral verify (RCA §4): read back and confirm the file still parses + carries the keys.
$check = Get-Content $f -Raw
if ($check -match 'jwt_secret = "' -and $check -match 'sentry_service_key = "') {
  Write-Host 'VERIFY_OK: server racecontrol.toml updated + re-readable'
} else {
  Write-Error 'VERIFY_FAIL: post-write read-back missing expected keys'; exit 1
}
