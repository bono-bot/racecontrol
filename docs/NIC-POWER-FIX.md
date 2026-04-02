# Fleet NIC Power Management Fix (2026-04-03)

## Incident
Pod 6 went unreachable (ping failed from James) but was NOT physically off. User restarted to recover. Root cause was never captured at the time.

## Diagnosis

### Root Cause: Realtek NIC Power Management
All 8 pods have `Realtek PCIe 2.5GbE Family Controller`. Windows had `AllowComputerToTurnOffDevice = Enabled` and multiple Realtek power-saving features active:
- `PowerSavingMode = 1`
- `PowerDownPll = 1`
- `EnableGreenEthernet = 1` (Green Ethernet)
- `*EEE = 1` (Energy Efficient Ethernet)

When idle, Windows puts the NIC to sleep. The pod remains physically powered on but is completely unreachable by ping, SSH, and Tailscale.

### Eliminated Suspects
- **Tailscale:** CLEAR — sim6 connected, no auth issues
- **Windows Firewall:** CLEAR — all 3 profiles (Domain/Private/Public) disabled on all pods

### Additional Finding
`Function Discovery Resource Publication` service crashes on every boot with "The requested address is not valid in its context" — network bindings unstable during startup. Benign but noisy.

## Fix Applied — All 8 Pods

### Registry Path
`HKLM\SYSTEM\CurrentControlSet\Control\Class\{4d36e972-e325-11ce-bfc1-08002be10318}\<INDEX>`

### Per-Pod NIC Registry Index Map

| Pod | Hostname | LAN IP | Registry Index |
|-----|----------|--------|---------------|
| 1 | SIM1-1 | 192.168.31.89 | 0008 |
| 2 | SIM2 | 192.168.31.33 | 0002 |
| 3 | SIM3 | 192.168.31.28 | 0000 |
| 4 | SIM4 | 192.168.31.88 | 0000 |
| 5 | SIM5 | 192.168.31.86 | 0001 |
| 6 | SIM6 | 192.168.31.87 | 0000 |
| 7 | SIM7 | 192.168.31.38 | 0002 |
| 8 | SIM8 | 192.168.31.91 | 0001 |

### Values Set

| Setting | Before | After | Type |
|---------|--------|-------|------|
| PowerSavingMode | 1 | **0** | REG_SZ |
| PowerDownPll | 1 | **0** | REG_SZ |
| EnableGreenEthernet | 1 | **0** | REG_SZ |
| *EEE | 1 | **0** | REG_SZ |
| PnPCapabilities | (unset) | **0x18 (24)** | REG_DWORD |

### Verification
All 8 pods confirmed via `reg query`:
- `PowerSavingMode = 0`
- `PnPCapabilities = 0x18`

### Fix Type
**PERMANENT** — registry values survive reboot. Full effect after next reboot of each pod.

### Notes
- `Get-NetAdapterPowerManagement` still reports `AllowComputerToTurnOffDevice = Enabled` — this is a display artifact. `PnPCapabilities=0x18` is the authoritative kernel-level setting.
- `Disable-NetAdapterPowerManagement` PowerShell cmdlet does NOT work for Realtek drivers — must use registry directly.
- Registry index varies per pod (depends on device enumeration order). Use the map above.

## Re-apply Command (per pod)
```cmd
reg add "HKLM\SYSTEM\CurrentControlSet\Control\Class\{4d36e972-e325-11ce-bfc1-08002be10318}\<INDEX>" /v PowerSavingMode /t REG_SZ /d 0 /f
reg add "HKLM\SYSTEM\CurrentControlSet\Control\Class\{4d36e972-e325-11ce-bfc1-08002be10318}\<INDEX>" /v PowerDownPll /t REG_SZ /d 0 /f
reg add "HKLM\SYSTEM\CurrentControlSet\Control\Class\{4d36e972-e325-11ce-bfc1-08002be10318}\<INDEX>" /v EnableGreenEthernet /t REG_SZ /d 0 /f
reg add "HKLM\SYSTEM\CurrentControlSet\Control\Class\{4d36e972-e325-11ce-bfc1-08002be10318}\<INDEX>" /v *EEE /t REG_SZ /d 0 /f
reg add "HKLM\SYSTEM\CurrentControlSet\Control\Class\{4d36e972-e325-11ce-bfc1-08002be10318}\<INDEX>" /v PnPCapabilities /t REG_DWORD /d 24 /f
```
