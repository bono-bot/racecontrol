# ConspitLink Wheelbase Software Audit
**Date:** 2026-03-21
**Pod:** Pod 8 (192.168.31.91) -- canary pod
**Tool:** Sysinternals Process Monitor v3.x
**Purpose:** Determine if ConspitLink software exhibits behaviors that could trigger anti-cheat detection

## ConspitLink Context
- Software: ConspitLink (Conspit Ares 8Nm wheelbase management)
- Wheelbase: Conspit Ares 8Nm -- OpenFFBoard VID:0x1209 PID:0xFFB0
- Runs on all 8 pods during every session
- Internal behavior is opaque -- no source code access

## ConspitLink Audit

## Audit Checklist

The following must be checked during a ProcMon capture session:

### 1. Kernel Drivers
- [ ] Does ConspitLink load any kernel-mode drivers (.sys files)?
- [ ] If yes, are those drivers signed with a valid certificate?
- [ ] Driver names found: _______________

### 2. DLL Injection
- [ ] Does ConspitLink inject DLLs into other processes?
- [ ] Specifically, does it inject into game processes (acs.exe, iRacingSim64DX11.exe, LMU.exe, F1_25.exe)?
- [ ] DLL injection targets found: _______________

### 3. Process Handle Activity
- [ ] Does ConspitLink open handles to game processes?
- [ ] What access rights are requested (PROCESS_VM_READ, PROCESS_ALL_ACCESS, etc.)?
- [ ] Process handles found: _______________

### 4. Shared Memory / Memory Mapping
- [ ] Does ConspitLink use OpenFileMapping/MapViewOfFile?
- [ ] What memory map names does it access?
- [ ] Memory maps found: _______________

### 5. Registry Activity
- [ ] What registry keys does ConspitLink read/write during a game session?
- [ ] Any HKLM writes during game sessions?
- [ ] Registry paths found: _______________

### 6. Network Activity
- [ ] Does ConspitLink open any network connections?
- [ ] What ports/addresses?
- [ ] Network activity found: _______________

## ProcMon Configuration

### Filter Setup (apply in ProcMon before capture):
1. Process Name contains "ConspitLink" -- Include
2. Process Name contains "conspit" -- Include
3. Operation is "Load Image" -- Include (catches DLL/driver loads)
4. Operation is "CreateFile" -- Include
5. Path contains ".sys" -- Include (kernel driver loads)
6. Path contains ".dll" -- Include (DLL activity)

### Capture Procedure:
1. Start ProcMon on Pod 8 (run as Administrator)
2. Apply filters above
3. Launch Assetto Corsa (no anti-cheat -- safe to test)
4. Drive for 2-3 minutes with wheelbase active
5. Stop capture
6. Export to CSV: File > Save > Events displayed using current filter > CSV
7. Save CSV as `conspit-link-procmon-capture.csv`

### Analysis Priority:
- CRITICAL: Any `.sys` driver load by ConspitLink
- HIGH: Any DLL injection into a game process (check "CreateRemoteThread" operations)
- HIGH: Any OpenProcess call targeting game PID
- MEDIUM: Shared memory access to game memory maps
- LOW: Standard HID communication (USB device I/O) -- this is expected and safe

## Findings

[To be filled after ProcMon capture]

### Verdict: DEFERRED -- pending ProcMon capture

**SAFE:** ConspitLink only uses standard HID device communication and does not interact with game processes. No action needed -- keep running during all game sessions.

**RISKY:** ConspitLink performs some behaviors that could contribute to anti-cheat detection (e.g., shared memory access, registry writes during game sessions). Add to safe mode gating list -- suspend/restart around game sessions.

**CRITICAL:** ConspitLink loads kernel drivers or injects DLLs into game processes. Contact Conspit for a signed/compatible version. Until resolved, ConspitLink must be stopped before protected game sessions and restarted after.

## Certificate Status

ConspitLink binary signed: [YES/NO -- check with signtool verify /pa ConspitLink.exe]
