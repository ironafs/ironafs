<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# A `\\afs` path on Windows without a driver: research and measurements (2026-11-19)

**How this note was made.**

- Three kinds of evidence, labelled as in the [research index](README.md):
  - **INSPECTED:** read in Microsoft's documentation, linked with the claim,
    by a research pass; the three pages the answer rests on (alternative
    ports, the SMB2 transports, `netsh portproxy`) were re-read by the
    maintainer's session, and are marked so under Sources;
  - **MEASURED:** as defined in the research index, measured outside
    IronAFS code:
    here, either on disposable virtual machines for this note (section 3;
    the steps are in the appendix, so anyone can repeat them), or, where
    marked, in the maintainer's earlier Windows work, whose raw results are
    private. These are evidence for design, not proof that IronAFS works
    (EXECUTED in the sense of `docs/workflow.md` §3c, but not on IronAFS);
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

**Route 3 gets Windows' SMB client's traffic to a listener on the loopback
address on Windows 10 and 11 without giving up Windows' own file sharing
(MEASURED, section 3, with a probe listener, to the client's first packet;
a full SMB session through it is not yet measured).**
Windows' built-in port proxy listens on
`127.0.0.1:445` and copies bytes to IronAFS's server on a high loopback
port, while Windows' own SMB server keeps `0.0.0.0:445` and `[::]:445`. A
hosts entry sends `afs` to `127.0.0.1`. The proxy must bind before Windows'
kernel listener; one service dependency kept that order across a reboot in
the tests, and whether it survives updates is a spike gate.

| Route | Windows versions | Windows' own file sharing | Status |
|---|---|---|---|
| 1. Our server on another port; a machine policy ("Alternative Port Mappings") sends `\\afs` there | per-mapping alternative ports: 11 24H2 and later (Microsoft Learn); the machine policy: measured on 26H2 only | kept | MEASURED (maintainer's earlier work, 26H2 only). The policy was known from a third-party copy of Microsoft's administrative template; Microsoft's own template for 24H2 was not checked, so 24H2 is to be measured |
| 2. Our server on `127.0.0.1:445` with Windows' SMB server disabled | 10 and 11 | **lost**: no inbound sharing, no `C$` or `ADMIN$` | MEASURED (maintainer's earlier work); now a fallback only |
| **3. Port proxy on `127.0.0.1:445` beside Windows' SMB server** | **10 22H2, 11 24H2, 11 26H2** | **kept** | **MEASURED (this note)** |

Route 3's side effects:

- a literal `\\127.0.0.1\...` reaches IronAFS instead of Windows, and fails
  outright while IronAFS is stopped; `\\localhost` still reaches Windows,
  because it resolves to `::1` first;
- the hosts entry sends the name `afs` to `127.0.0.1` for every program,
  not only for SMB;
- while IronAFS is stopped, its back-end port is free for another local
  process to take (section 3, still to measure).

## 1. Why port 445 is the constraint

- Windows' SMB client connects to alternative TCP ports only on Windows 11
  24H2 and later (and Windows Server 2025), through an elevated
  `New-SmbMapping -TcpPort` or `NET USE /TCPPORT`. INSPECTED:
  [Configure alternative SMB ports](https://learn.microsoft.com/en-us/windows-server/storage/file-server/smb-ports).
- Windows' own SMB server can change only its QUIC listening port: "You
  can't configure TCP or RDMA listening ports." INSPECTED: same page.
- So on Windows 10, and on Windows 11 before 24H2, `\\afs` over SMB's
  direct-TCP transport has to reach our server through port 445, the
  default, on some local address.
- SMB over QUIC (UDP, normally port 443) is a separate transport that the
  Windows 11 client supports and Windows 10 lacks. A QUIC route would need
  IronAFS to implement SMB over QUIC with a certificate the client trusts;
  it is not evaluated further here. NOT VERIFIED beyond Microsoft's
  overview: [SMB over QUIC](https://learn.microsoft.com/en-us/windows-server/storage/file-server/smb-over-quic).
- The port is held by the kernel driver `srvnet`, not by the Server service:
  stopping `LanmanServer`, or `LanmanServer` and `srv2`, leaves
  `0.0.0.0:445` listening; stopping `srvnet` releases it. MEASURED.

## 2. The candidates

| Candidate | Result | Evidence |
|---|---|---|
| Our server on a second loopback address (`127.0.0.2:445`) | The port binds, but **Windows' SMB client refuses** `\\127.0.0.2\...` at once, with no connection attempt ("The network location cannot be reached"), and `\\afs` mapped to `127.0.0.2` fails the same way. | MEASURED |
| Port proxy on `127.0.0.1:445`, bound before `srvnet` (route 3) | Works. | MEASURED, section 3 |
| IronAFS's own service binding `127.0.0.1:445` before `srvnet` (route 3 without the proxy) | Expected to work the same way, since the proxy's listener is an ordinary socket; it would remove the back-end port. But `LanmanServer` would then depend on IronAFS's service, so Windows' own file sharing would wait for, or fail with, IronAFS at boot. | NOT VERIFIED; to measure in the spike |
| NetBIOS over TCP, port 139 | Ruled out: NetBIOS carries SMB dialects 2.0.2 to 3.0.2 only, not 3.1.1. | INSPECTED: [MS-SMB2 Transport](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-smb2/1dfacde4-b5c7-4494-8a14-a09d3ab4cc83) |
| Our server terminating SMB on 445 and passing other shares on to Windows' server | Ruled out: the share name arrives only after authentication, so passing a session on means relaying the user's authentication. | NOT VERIFIED as a measurement; follows from the protocol order |
| Windows' own SMB server sharing a local folder that IronAFS fills | Ruled out: requests reach the filesystem as the System account, so the caller's identity is lost. | MEASURED (maintainer's earlier work) |
| Windows' own NFS client | Ruled out as measured: in the configuration tried (anonymous UID mapping), every Windows user reached the server with the same identity. Per-user identity mapping was not tested. | MEASURED (maintainer's earlier work), for that configuration only |
| WebDAV redirector | The WebDAV path syntax takes an optional port (`\\afs@port\...`); a plain `\\afs\...` reaches WebDAV only if no provider ahead of it in the network provider order (SMB by default) claims the name ([UNC naming and MUP](https://learn.microsoft.com/en-us/windows-hardware/drivers/ifs/support-for-unc-naming-and-mup)). Microsoft has deprecated the WebClient service. | The path syntax INSPECTED by a research pass: [DavGetHTTPFromUNCPath](https://learn.microsoft.com/en-us/windows/win32/api/davclnt/nf-davclnt-davgethttpfromuncpath); the deprecation: [deprecated features](https://learn.microsoft.com/en-us/windows/whats-new/deprecated-features) |
| A loopback adapter or a Hyper-V switch for a separate address | Not needed, and each installs a device. | NOT VERIFIED |

## 3. Route 3, measured

**Machines.** Three fresh, disposable virtual machines, each in a workgroup
with a local test account, Windows Update turned off, measured on
2026-11-19 (builds as Windows reported them):

- Windows 10 Pro 22H2 (build 19045), x86-64, VMware Workstation;
- Windows 11 Pro 24H2 (build 26100), x86-64, VMware Workstation;
- Windows 11 26H2 (build 26300), ARM64, Parallels Desktop.

**Method.** A throwaway TCP listener on `127.0.0.1:1445` recorded the first
bytes of each connection. It speaks no SMB, so the client's attempt ends
with "The specified network name is no longer available"; the recorded
bytes are the evidence. The full steps are in the appendix.

**Results.** All checks ran on all three machines, except SMB from
another machine, which ran on the ARM machine only. Builds are recorded as
Windows reported them (major version only); exact revisions were not
captured.

| Check | Result |
|---|---|
| With `srvnet` stopped first, the port proxy binds `127.0.0.1:445`; Windows' stack then binds `0.0.0.0:445` and `[::]:445` beside it | yes |
| `\\afs\test` from the interactive desktop session reaches our listener | yes: an SMB1 multi-protocol negotiate (`ff 53 4d 42 72`), Windows' usual first packet. The listener then closed the connection, so the client reported "The specified network name is no longer available"; no SMB session was established |
| `\\<computer name>\C$`, `\\<own LAN address>\C$`, `\\localhost\C$` | still served by Windows' own server |
| SMB from another machine to the LAN address (ARM machine only) | still answered by Windows' own server |
| With `LanmanServer` depending on `iphlpsvc`, after a reboot | the same listeners, in the same order, both services running |

**Why this is not a relay.** The port proxy copies bytes, so negotiation,
authentication and signing should run end to end between Windows' SMB
client and IronAFS's server, as when our server holds port 445 itself. That
is the expected result; this note measured the path only to the client's
first packet.

**Still to measure** (the design's early Windows spike):

- the caller's identity, dialect and signing through the proxy, with a real
  SMB 3.1.1 server running SSPI; expected unchanged, since the bytes are;
- protection of the high back-end port from other local processes while
  IronAFS is stopped. A persistent TCP port reservation is a candidate, but
  Microsoft documents that it gives no security between components able to
  obtain reservations, so a Windows Filtering Platform bind rule is the
  other candidate; the spike shows which holds, and that `\\afs` fails
  closed while IronAFS is stopped;
- the boot order across restarts, feature updates and an uninstall, and
  Windows' own sharing after each;
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

On a disposable test machine, in an elevated PowerShell. Record each
original value before changing it, and restore that value to undo.

**1. A throwaway listener** (`C:\test\logger.ps1`), run as a scheduled task
as SYSTEM so that it outlives the shell. It speaks no SMB: it records the
first bytes of each connection and closes it.

```powershell
$l = [System.Net.Sockets.TcpListener]::new([System.Net.IPAddress]::Parse('127.0.0.1'), 1445)
$l.Start()
$log = 'C:\test\logger.txt'
"listening $(Get-Date -Format o)" | Out-File $log -Append
while ($true) {
  $c = $l.AcceptTcpClient()
  $s = $c.GetStream(); $s.ReadTimeout = 3000
  $buf = New-Object byte[] 256; $n = 0
  try { while ($n -lt 256) { $r = $s.Read($buf, $n, 256 - $n); if ($r -le 0) { break }; $n += $r; if ($n -ge 9) { break } } } catch { }
  $hex = ($buf[0..([Math]::Max($n,1)-1)] | ForEach-Object { $_.ToString('x2') }) -join ''
  "conn from $($c.Client.RemoteEndPoint) bytes=$n first=$($hex.Substring(0,[Math]::Min(48,$hex.Length))) $(Get-Date -Format o)" | Out-File $log -Append
  $c.Close()
}
```

```powershell
$a = New-ScheduledTaskAction -Execute 'powershell.exe' -Argument '-NoProfile -ExecutionPolicy Bypass -File C:\test\logger.ps1'
$t = New-ScheduledTaskTrigger -AtStartup
Register-ScheduledTask -TaskName test-logger -Action $a -Trigger $t -User SYSTEM -RunLevel Highest
Start-ScheduledTask test-logger
netstat -ano | Select-String '127.0.0.1:1445 '   # confirm it is listening, again after any reboot
```

**2. Record the originals.**

```powershell
netstat -ano | Select-String ':445 '        # expect 0.0.0.0:445 and [::]:445, owned by PID 4
sc.exe qc LanmanServer                       # note DEPENDENCIES (SamSS and Srv2 on the 26H2 machine)
netsh interface portproxy show all           # expect no rules
```

**3. The port proxy rule, bound before Windows' kernel listener.**

```powershell
netsh interface portproxy add v4tov4 listenaddress=127.0.0.1 listenport=445 connectaddress=127.0.0.1 connectport=1445
net stop server /y; sc.exe stop srv2; sc.exe stop srvnet
Restart-Service iphlpsvc -Force
sc.exe start srvnet; sc.exe start srv2; net start server
netstat -ano | Select-String ':445 '         # expect 0.0.0.0:445, 127.0.0.1:445 and [::]:445
```

**4. Keep that order at boot:** add `iphlpsvc` to the recorded list, for
example `sc.exe config LanmanServer depend= SamSS/Srv2/iphlpsvc`. Reboot and
repeat the `netstat` check.

**5. Send the name to the proxy:** add the line `127.0.0.1 afs` to
`C:\Windows\System32\drivers\etc\hosts`, then `ipconfig /flushdns`.

**6. Test from an ordinary desktop session**, not over SSH, whose network
logon behaves differently, in PowerShell: `Get-ChildItem \\afs\test`
(expected to fail after the listener records a negotiate),
`Get-ChildItem "\\$env:COMPUTERNAME\C$"`, `Get-ChildItem \\localhost\C$`
and `Get-ChildItem \\<own LAN address>\C$` (expected to succeed, served by
Windows), then read `C:\test\logger.txt`.

**Undo:** `netsh interface portproxy delete v4tov4 listenaddress=127.0.0.1
listenport=445`; restore the recorded `LanmanServer` dependencies; remove
the hosts line; unregister the task and delete `C:\test`; reboot, and
check Windows' own shares again.

## Sources

- [Configure alternative SMB ports](https://learn.microsoft.com/en-us/windows-server/storage/file-server/smb-ports) (INSPECTED, re-read)
- [MS-SMB2: Transport](https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-smb2/1dfacde4-b5c7-4494-8a14-a09d3ab4cc83) (INSPECTED, re-read)
- [netsh interface portproxy](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/netsh-interface) (INSPECTED, re-read: the `v4tov4` syntax)
- [Deprecated features in the Windows client](https://learn.microsoft.com/en-us/windows/whats-new/deprecated-features)
- [Mapped drives are not available from an elevated prompt](https://learn.microsoft.com/en-us/troubleshoot/windows-client/networking/mapped-drives-not-available-from-elevated-command)
- [Deploy Folder Redirection](https://learn.microsoft.com/en-us/windows-server/storage/folder-redirection/deploy-folder-redirection)
- [Persistent TCP port reservations](https://learn.microsoft.com/en-us/windows/win32/api/iphlpapi/nf-iphlpapi-createpersistenttcpportreservation) (a candidate for the back-end port; whether it keeps other local processes out is to be measured)
- [SMB over QUIC](https://learn.microsoft.com/en-us/windows-server/storage/file-server/smb-over-quic)
- [SMB NTLM blocking](https://learn.microsoft.com/en-us/windows-server/storage/file-server/smb-ntlm-blocking) (for the spike)
