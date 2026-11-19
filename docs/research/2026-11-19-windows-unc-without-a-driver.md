<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# A `\\afs` path on Windows without a driver: research and measurements (2026-11-19)

**How this note was made.**

- Three kinds of evidence, labelled as in the [research index](README.md):
  - **INSPECTED:** read in Microsoft's documentation, linked with the claim,
    by a research pass; the three pages the answer rests on (alternative
    ports, the SMB2 transports, `netsh portproxy`) were re-read by the
    maintainer's session, and are marked so under Sources;
  - **MEASURED:** measured for this note on disposable virtual machines, as
    described in section 3, or (where marked) in the maintainer's earlier
    Windows work, whose raw results are private;
  - **NOT VERIFIED:** reasoned or reported by others, not measured or read
    in a primary source.
- Two research passes from different model families (Claude, GPT) listed the
  candidates; every claim below that a decision rests on was then checked
  at its source or measured.
- No AFS implementation's source was read for this note.

## Answer

Windows sends UNC file I/O only to its own kernel redirectors or to a
third-party driver. IronAFS installs no driver, so a `\\afs\cell\path` path
means Windows' own SMB client talking to an SMB 3.1.1 server that IronAFS
runs on the same machine. The problem is the port: Windows 10's SMB client
connects only to port 445, and Windows' own SMB server already holds 445.

**Route 3 solves this on Windows 10 and 11 without giving up Windows' own
file sharing (MEASURED, section 3).** Windows' built-in port proxy listens on
`127.0.0.1:445` and copies bytes to IronAFS's server on a high loopback
port, while Windows' own SMB server keeps `0.0.0.0:445` and `[::]:445`. A
hosts entry sends `afs` to `127.0.0.1`. The proxy must bind before Windows'
kernel listener, which one service dependency arranges at every boot.

| Route | Windows versions | Windows' own file sharing | Status |
|---|---|---|---|
| 1. Our server on another port; a machine policy sends `\\afs` there | 11 24H2 and later | kept | MEASURED (maintainer's earlier work, 26H2) |
| 2. Our server on `127.0.0.1:445` with Windows' SMB server disabled | 10 and 11 | **lost**: no inbound sharing, no `C$` or `ADMIN$` | MEASURED (maintainer's earlier work); now a fallback only |
| **3. Port proxy on `127.0.0.1:445` beside Windows' SMB server** | **10 22H2, 11 24H2, 11 26H2** | **kept** | **MEASURED (this note)** |

The only side effect of route 3: a literal `\\127.0.0.1\...` reaches
IronAFS instead of Windows. `\\localhost` still reaches Windows, because it
resolves to `::1` first.

## 1. Why port 445 is the constraint

- Windows' SMB client connects to alternative TCP ports only on Windows 11
  24H2 and later (and Windows Server 2025), through an elevated
  `New-SmbMapping -TcpPort` or `NET USE /TCPPORT`. INSPECTED:
  [Configure alternative SMB ports](https://learn.microsoft.com/en-us/windows-server/storage/file-server/smb-ports).
- Windows' own SMB server can change only its QUIC listening port: "You
  can't configure TCP or RDMA listening ports." INSPECTED: same page.
- So on Windows 10, and on Windows 11 before 24H2, `\\afs` has to reach our
  server through port 445 on some local address.
- The port is held by the kernel driver `srvnet`, not by the Server service:
  stopping `LanmanServer`, or `LanmanServer` and `srv2`, leaves
  `0.0.0.0:445` listening; stopping `srvnet` releases it. MEASURED.

## 2. The candidates

| Candidate | Result | Evidence |
|---|---|---|
| Our server on a second loopback address (`127.0.0.2:445`) | The port binds, but **Windows' SMB client refuses** `\\127.0.0.2\...` at once, with no connection attempt ("The network location cannot be reached"), and `\\afs` mapped to `127.0.0.2` fails the same way. | MEASURED |
| Port proxy on `127.0.0.1:445`, bound before `srvnet` (route 3) | Works. | MEASURED, section 3 |
| NetBIOS over TCP, port 139 | Ruled out: NetBIOS carries SMB dialects 2.0.2 to 3.0.2 only, not 3.1.1. | INSPECTED: [MS-SMB2 Transport](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-smb2/1dfacde4-b5c7-4494-8a14-a09d3ab4cc83) |
| Our server terminating SMB on 445 and passing other shares on to Windows' server | Ruled out: the share name arrives only after authentication, so passing a session on means relaying the user's authentication. | NOT VERIFIED as a measurement; follows from the protocol order |
| Windows' own SMB server sharing a local folder that IronAFS fills | Ruled out: requests reach the filesystem as the System account, so the caller's identity is lost. | MEASURED (maintainer's earlier work) |
| Windows' own NFS client | Ruled out: one anonymous identity for every Windows user. | MEASURED (maintainer's earlier work) |
| WebDAV redirector | Only with explicit `\\afs@port\...` paths; Microsoft has deprecated the WebClient service. | INSPECTED: [deprecated features](https://learn.microsoft.com/en-us/windows/whats-new/deprecated-features) |
| A loopback adapter or a Hyper-V switch for a separate address | Not needed, and each installs a device. | NOT VERIFIED |

## 3. Route 3, measured

**Machines.** Three fresh, disposable virtual machines, each in a workgroup
with a local test account, Windows Update turned off:

- Windows 10 Pro 22H2 (build 19045), x86-64, VMware Workstation;
- Windows 11 Pro 24H2 (build 26100), x86-64, VMware Workstation;
- Windows 11 26H2 (build 26300), ARM64, Parallels Desktop.

**Method.** A throwaway TCP listener on `127.0.0.1:1445` recorded the first
bytes of each connection. It speaks no SMB, so the client's attempt ends
with "The specified network name is no longer available"; the recorded
bytes are the evidence. The full steps are in the appendix.

**Results, the same on all three machines:**

| Check | Result |
|---|---|
| With `srvnet` stopped first, the port proxy binds `127.0.0.1:445`; Windows' stack then binds `0.0.0.0:445` and `[::]:445` beside it | yes |
| `\\afs\test` from the interactive desktop session reaches our listener | yes: an SMB1 multi-protocol negotiate (`ff 53 4d 42 72`), Windows' usual first packet |
| `\\<computer name>\C$`, `\\<own LAN address>\C$`, `\\localhost\C$` | still served by Windows' own server |
| SMB from another machine to the LAN address | still answered by Windows' own server (tested on the ARM machine) |
| With `LanmanServer` depending on `iphlpsvc`, after a reboot | the same listeners, in the same order, both services running |

**Why this is not a relay.** The port proxy copies bytes. Negotiation,
authentication and signing run end to end between Windows' SMB client and
IronAFS's server, exactly as when our server holds port 445 itself.

**Still to measure** (the design's early Windows spike):

- the caller's identity, dialect and signing through the proxy, with a real
  SMB 3.1.1 server running SSPI; expected unchanged, since the bytes are;
- protection of the high back-end port from other local processes while
  IronAFS is stopped (a persistent TCP port reservation, or a Windows
  Filtering Platform bind rule);
- whether a Windows feature update resets the `LanmanServer` dependency;
- Kerberos on a domain-joined machine and on Entra; NTLM local mode with
  SMB client NTLM blocking on, and without `BackConnectionHostNames`;
- remote administration of the machine (`ADMIN$`, `IPC$` named pipes) from
  another host with credentials.

## 4. Why a UNC path matters

- OpenAFS's Windows client documents `\\AFS\cell\path` as the canonical
  name and advises UNC paths over drive letters, especially for roaming
  profiles and redirected folders. NOT VERIFIED here (reported by a research
  pass; [OpenAFS for Windows release notes](https://docs.openafs.org/ReleaseNotesWindows/)).
- Drive letters belong to one logon session: services, scheduled tasks and
  elevated processes may not see them. INSPECTED:
  [mapped drives and elevation](https://learn.microsoft.com/en-us/troubleshoot/windows-client/networking/mapped-drives-not-available-from-elevated-command).
- Folder Redirection and roaming profiles take UNC paths. INSPECTED:
  [Folder Redirection](https://learn.microsoft.com/en-us/windows-server/storage/folder-redirection/deploy-folder-redirection).

## Appendix: reproducing route 3

On a test machine, in an elevated PowerShell. Every change has its undo.

```powershell
# 1. A throwaway listener that records the first bytes of each connection.
#    (Run it as a scheduled task as SYSTEM, so it outlives the shell.)
$l = [System.Net.Sockets.TcpListener]::new([Net.IPAddress]::Parse('127.0.0.1'), 1445); $l.Start()
# ... accept, read up to 256 bytes, log them, close ...

# 2. The port proxy rule.                      Undo: netsh interface portproxy delete v4tov4 listenaddress=127.0.0.1 listenport=445
netsh interface portproxy add v4tov4 listenaddress=127.0.0.1 listenport=445 connectaddress=127.0.0.1 connectport=1445

# 3. Let the proxy bind first, then bring Windows' SMB stack back.
net stop server /y; sc.exe stop srv2; sc.exe stop srvnet
Restart-Service iphlpsvc -Force
sc.exe start srvnet; sc.exe start srv2; net start server
netstat -ano | Select-String ':445 '   # expect 0.0.0.0:445, 127.0.0.1:445 and [::]:445

# 4. Keep that order at boot.                  Undo: sc.exe config LanmanServer depend= SamSS/Srv2
sc.exe config LanmanServer depend= SamSS/Srv2/iphlpsvc

# 5. Send the name to the proxy.               Undo: remove the line
Add-Content C:\Windows\System32\drivers\etc\hosts "`r`n127.0.0.1 afs"

# 6. From an ordinary desktop session (not over SSH, whose network logon
#    behaves differently): dir \\afs\test, dir \\%COMPUTERNAME%\C$,
#    dir \\localhost\C$, then read the listener's log.
```

## Sources

- [Configure alternative SMB ports](https://learn.microsoft.com/en-us/windows-server/storage/file-server/smb-ports) (INSPECTED, re-read)
- [MS-SMB2: Transport](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-smb2/1dfacde4-b5c7-4494-8a14-a09d3ab4cc83) (INSPECTED, re-read)
- [netsh interface portproxy](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/netsh-interface) (INSPECTED, re-read: the `v4tov4` syntax)
- [Deprecated features in the Windows client](https://learn.microsoft.com/en-us/windows/whats-new/deprecated-features)
- [Mapped drives are not available from an elevated prompt](https://learn.microsoft.com/en-us/troubleshoot/windows-client/networking/mapped-drives-not-available-from-elevated-command)
- [Deploy Folder Redirection](https://learn.microsoft.com/en-us/windows-server/storage/folder-redirection/deploy-folder-redirection)
- [Persistent TCP port reservations](https://learn.microsoft.com/en-us/windows/win32/api/iphlpapi/nf-iphlpapi-createpersistenttcpportreservation) (for the back-end port, to be measured)
- [SMB NTLM blocking](https://learn.microsoft.com/en-us/windows-server/storage/file-server/smb-ntlm-blocking) (for the spike)
