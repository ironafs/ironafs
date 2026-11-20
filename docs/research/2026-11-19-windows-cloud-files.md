<!-- SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com> -->
<!-- SPDX-License-Identifier: GPL-3.0-or-later -->

# Windows Cloud Files for an AFS client: research notes (2026-11-19)

**How this note was made.**

- Two kinds of evidence, labelled as in the [research index](README.md):
  - **INSPECTED:** read in Microsoft's documentation, linked with the claim;
  - **MEASURED:** measured by the maintainer on disposable virtual machines
    running Windows 11 Pro 26H2
    (build 26300) and Windows 10 Pro 22H2 (build 19045), roots on local
    NTFS, in October and November 2026. The raw results are not public.
    These are EXECUTED results in the sense of `docs/workflow.md` §3c, but
    not on IronAFS code and not reproducible from this repository, so they
    are evidence for design, not proof that IronAFS works;
  - **NOT VERIFIED:** reasoned or reported, not measured or documented.
- Where Microsoft's documentation and a measurement agree, both are given.
  Where they disagree, the measurement is stated and the difference named.
- No AFS implementation's source was read for this note.

## Answer

Of the two NTFS-backed APIs, Cloud Files is the one to use, and ProjFS is
not. Neither is a filesystem interface: both keep file data as ordinary
NTFS files and call the provider only to fill them. So a Cloud Files layer
is a sync engine with its own authorisation, write and coherency design,
built around five constraints:

1. No callback carries the caller's user or token.
2. Hydrated data is served by NTFS without the provider, so NTFS DACLs, not
   AFS ACLs, decide who reads it.
3. No callback reports a write; the provider finds changes itself.
4. A provider that dies at the wrong moment can wedge its sync root until
   reboot.
5. Antivirus scanning, memory maps and first writes hydrate whole files.

Constraints 1 and 2 are why an online `\\afs` path, through IronAFS's own
SMB server with every request in an authenticated session, is the
alternative; its transport was measured beside Windows' own file sharing
on Windows 10 and 11 ([the UNC note](2026-11-19-windows-unc-without-a-driver.md)).
The owner decided on 2026-11-19 to build that path first; ADR 0004 (the
architecture decision) is its record. This note's evidence serves Cloud
Files as a possible offline companion.

## 1. The API model

- **Components.** A minifilter, `cldflt.sys`, sits between applications and
  the provider: it presents placeholders as files and asks the provider for
  data. It supports NTFS only. The provider is a user-mode desktop
  application; the API arrived in Windows 10 1709 (INSPECTED,
  [Build a cloud sync engine](https://learn.microsoft.com/en-us/windows/win32/cfapi/build-a-cloud-file-sync-engine)).
  The filter is present and running on stock Windows 10 and 11, and the
  correctness APIs all exist on Windows 10; Windows 11 adds Shell features
  only (MEASURED).
- **File states.** A placeholder holds metadata only; a full file has been
  hydrated and "could be dehydrated by the system if space is needed"; a
  pinned full file stays local (INSPECTED, same page).
- **Callbacks.** The platform asks the provider to fetch data or directory
  contents, to validate data, and to approve a dehydrate, delete or rename;
  it also reports open, close and completion events after the fact
  (INSPECTED,
  [CF_CALLBACK_TYPE](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/ne-cfapi-cf_callback_type)).
  - There is no callback for a write, a lock or a vetoable open. The open
    notification arrives after the open, does not block it, and carries no
    access mask (INSPECTED, same page; MEASURED).
  - Callbacks run concurrently on a thread pool.
- **Responses.** The provider answers through `CfExecute` (INSPECTED,
  [CfExecute](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/nf-cfapi-cfexecute)).
  In the configuration measured (the default policies, with the root
  connected), creating and updating placeholders worked only from the
  process holding the sync root's connection, and from any other process
  failed with `0x8007018B` (MEASURED). Microsoft documents placeholder
  management policies a provider can opt into at registration, and
  different rules for a root with no active connection
  ([CfRegisterSyncRoot](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/nf-cfapi-cfregistersyncroot)),
  so this is the default behaviour, not a fixed rule (NOT VERIFIED beyond
  the measured case).
- **Placement.** A sync root must be on a fixed NTFS volume, not a volume
  root, and not inside a connected root (MEASURED, refused with distinct
  errors; INSPECTED for NTFS). Registration above another root was allowed
  (MEASURED), but Microsoft documents that sync root trees must not overlap
  (INSPECTED,
  [CfRegisterSyncRoot](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/nf-cfapi-cfregistersyncroot)),
  so that observation is unsupported behaviour, not something to rely on. The filter attaches to a volume only at the first registration
  there, so tests need fresh machines (MEASURED).
- **Namespace.** A root is a local directory, not a `\\server` path. A
  per-session drive letter over a root works for Explorer, `cmd` and Win32
  programs, but elevated processes do not see it (MEASURED).
- **NTFS surface AFS never had.** Hard links, junctions, symbolic links,
  alternate data streams and EFS work inside a root and have no AFS
  equivalent, so the provider must refuse or skip them (MEASURED).

## 2. Caller identity: none

- A callback can carry the calling process's id, image path, package name,
  application id, command line and session id, on request
  (`CF_CONNECT_FLAG_REQUIRE_PROCESS_INFO`). It carries no user SID and no
  token (INSPECTED,
  [CF_PROCESS_INFO](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/ns-cfapi-cf_process_info),
  [CF_CONNECT_FLAGS](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/ne-cfapi-cf_connect_flags)).
- Turning a process id into a user is racy, and a provider running as a
  low-privilege service account could not resolve the caller's SID at all
  (MEASURED).
- Opens that arrive over SMB loopback show up as process 4 in session 0,
  with no user (MEASURED).
- `CfRegisterSyncRoot` takes an opaque identity blob; it names no user
  (INSPECTED,
  [CfRegisterSyncRoot](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/nf-cfapi-cfregistersyncroot)).

So authority cannot be decided per callback. It has to be bound to the root:
each sync root belongs to one Windows user and one AFS identity, and the
provider acts for that pair only.

## 3. Per-user access

### Why the provider must not run as the user

- Placeholders created by a provider running as the user are owned by the
  user, even under a root owned by another account. The user's other
  programs can then rewrite their DACLs and grant themselves access the AFS
  ACLs deny (MEASURED).
- Registering a root needs `WRITE_DATA` or `WRITE_DAC` on it; with
  read-and-execute only, registration was refused (MEASURED). Microsoft
  documents the same rights for updating a placeholder (INSPECTED,
  [CfUpdatePlaceholder](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/nf-cfapi-cfupdateplaceholder)).
  A user who registers their own root therefore holds rights that let them
  change it.
- Granting the user write access only while registering was unsafe: during
  the grant the user planted a hard link that survived the revoke, and a
  directory handle opened in the window kept its right afterwards
  (MEASURED).

### The approach: a service-account provider and NTFS DACLs it owns

- The provider runs under another account, such as `LOCAL SERVICE`
  ([LocalService account](https://learn.microsoft.com/en-us/windows/win32/services/localservice-account)),
  and owns every root. Each per-user root gets a DACL that lets its user
  read, and, where writing is allowed, change files, but not change, rename
  or re-ACL the root itself. All of the user's attempts to change the root
  were then refused (MEASURED).
- A new root stays private to the service until it is registered, and the
  user's access is granted last. `CF_PLACEHOLDER_CREATE_INFO` has no
  security-descriptor field, and DACL edits raise no callback, so drift can
  only be found by checking (MEASURED; INSPECTED,
  [CF_PLACEHOLDER_CREATE_INFO](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/ns-cfapi-cf_placeholder_create_info)).
- A `NO_READ_UP` mandatory label at medium integrity on the root keeps out
  the user's own low-integrity processes; only an account that can write the
  root's SACL can set it (MEASURED;
  [Mandatory Integrity Control](https://learn.microsoft.com/en-us/windows/win32/secauthz/mandatory-integrity-control)).
- Every fetch, listing and upload is checked against the user's rights in
  AFS. Uploads are also checked with an access check against the user's own
  Windows token, held by the provider, because a service account cannot
  expand the user's domain groups and a check by SID alone would over-grant
  (MEASURED;
  [AccessCheck](https://learn.microsoft.com/en-us/windows/win32/api/securitybaseapi/nf-securitybaseapi-accesscheck)).
- Revoking access by re-propagating an inherited DACL walks every object, at
  about 0.2 ms each: 1.6 s for 10,000 objects, 22 s for 100,000, 236 s for a
  million (MEASURED). Re-granting inheritable entries on a directory that
  contains a sync root started failing populations, so roots sit in a parent
  that is never re-ACLed (MEASURED).

### What stays open

- Hydrated bytes are plain NTFS data. They stay readable with the provider
  stopped, and with no AFS token, by anyone the DACL admits; there is no
  per-open re-check (MEASURED; consistent with INSPECTED,
  [Build a cloud sync engine](https://learn.microsoft.com/en-us/windows/win32/cfapi/build-a-cloud-file-sync-engine)).
  Backup-semantics opens bypass DACLs (NOT VERIFIED, reasoned).
- The Shell's sync-root registry area is writable by the user, so a user can
  redirect their own Explorer entry until the provider repairs it, and the
  privileged layout used to give a service-held root a full Explorer entry
  is not one Microsoft commits to (MEASURED).
- Identity is per Windows user and per root: there is no per-session or
  per-process identity, so nothing like a PAG.

## 4. Hydration and dehydration

- **Policies.** Primary hydration policies are always-full, full,
  progressive and partial; the stronger of the application's and the
  provider's applies (INSPECTED,
  [Build a cloud sync engine](https://learn.microsoft.com/en-us/windows/win32/cfapi/build-a-cloud-file-sync-engine)).
  `CF_HYDRATION_POLICY_PARTIAL` "is not currently supported" (INSPECTED,
  [CF_HYDRATION_POLICY_PRIMARY](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/ne-cfapi-cf_hydration_policy_primary);
  MEASURED). Microsoft's sync-engine overview lists it among the policies
  without that caveat, so the two references disagree; the enum's caveat
  and the measurement agree.
- **Progressive fetches** ask for a small required range plus an optional
  range to the end of the file, and "may not assume that hydration callbacks
  arrive sequentially from offset 0" (INSPECTED, same page). A superseded
  fetch fails with `ERROR_CLOUD_FILE_REQUEST_CANCELED` and no cancel
  callback, and fetch keys are reused by later requests (MEASURED). One
  stream per file, a full required range and capped read-ahead worked
  (MEASURED).
- **Cancellation** is delivered: `CANCEL_FETCH_DATA` arrives on
  `CancelIoEx` and when the reading process is killed (MEASURED; INSPECTED,
  [CF_CALLBACK_TYPE](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/ne-cfapi-cf_callback_type)).
- **Whole-file hydration is forced** by:
  - a first write to a dehydrated file, which fetches the rest of the file;
    `CREATE_ALWAYS` silently turns a placeholder into a plain file with the
    same file id (MEASURED);
  - a memory-mapped section, with or without antivirus (MEASURED);
  - Microsoft Defender real-time protection: any data open of a dehydrated
    placeholder fetches the whole file inside `CreateFile` (MEASURED on
    Windows 11 and 10). On Windows 10 a 5 GiB read under real-time
    protection ran at about 1.3 MB/s. Microsoft notes that antivirus
    scanning can cause implicit hydration (INSPECTED,
    [CF_CONNECT_FLAGS](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/ne-cfapi-cf_connect_flags)).
  OneDrive registers full hydration, so it pays the same cost (MEASURED).
- **Read speed** with no antivirus in the way: 64 KiB fetches took 1 to 5 ms
  (MEASURED).
- **Refreshing content from the server.** An update without
  `CF_UPDATE_FLAG_DEHYDRATE` can leave stale bytes served; without
  `CF_UPDATE_FLAG_VERIFY_IN_SYNC` an update silently discards local edits
  (MEASURED). Microsoft documents the in-sync check as the guard against
  that race, and that dehydrating needs an exclusive handle the platform
  "does not validate" (INSPECTED,
  [CfUpdatePlaceholder](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/nf-cfapi-cfupdateplaceholder)).
  Updates reach handles that are already open, including running
  executables, so a refresh takes a share-none metadata open first and
  waits while any data handle is open (MEASURED).
- **Dehydration** from any process other than the connected one hangs
  unless automatic dehydration is allowed (MEASURED). Without a dehydration
  policy the local copy only grows: cache-size management is the provider's
  job.

## 5. Writing

Writing is local-first: the local NTFS file is the record of pending work,
and the server is updated after it.

- **Finding changes.** No callback reports a created or modified plain file,
  or a modified hydrated placeholder (MEASURED; INSPECTED,
  [CF_CALLBACK_TYPE](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/ne-cfapi-cf_callback_type)).
  A recursive directory watcher
  ([ReadDirectoryChangesW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-readdirectorychangesw))
  plus scans at connection, on watcher overflow and periodically found them
  (MEASURED). A USN journal reader needs a volume handle that a service
  account cannot have (MEASURED).
- **What is pending.** A plain file, or a placeholder that is not in sync, is
  pending; a cold placeholder never is, whatever its in-sync flag says. Save
  patterns differ: an in-place write keeps the file id; a temporary file
  renamed over the target gives a new id; `ReplaceFile`, as Word uses it,
  is a rename then a delete, and needs the user able to set the file's DACL
  (MEASURED).
- **Marking in sync.** `CfSetInSyncState` documents a USN input that closes
  the race between upload and marking (INSPECTED,
  [CfSetInSyncState](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/nf-cfapi-cfsetinsyncstate)).
  It did not work as documented: it accepted only the current USN plus the
  length of a record it writes itself, and any other writer on the volume
  broke that (MEASURED). What worked: hold a handle that shares read only,
  check that the USN has not changed since the upload read, and mark in sync
  with no USN while holding it (MEASURED). Writes through a mapped view reach
  the disk and the journal only when flushed, so they can escape this check
  (MEASURED; whether the in-sync mark is cleared at flush was not measured).
- **Publishing.** AFS has no conditional store, so a publish never replaces
  a name: new content goes to a temporary name and is linked in, and the
  replaced version is moved aside and kept as a conflict copy if it no
  longer matches the base. Create and five save patterns (Notepad, a
  temporary file renamed over the target, Word, and `ReplaceFile` with and
  without a backup) passed acceptance on Windows 11 this way (MEASURED). A
  window of milliseconds remains between the last check and the final
  unlink.
- **Conflicts are kept.** The server version keeps the name; the local
  version is renamed beside it with a conflict suffix and uploaded as a new
  file, one copy per version. The two-writer acceptance case did not
  converge in the recorded runs, and its fix is not yet re-measured
  (MEASURED). An editor that loaded an
  older version can still save over a newer one, because the base is the
  placeholder's version, not the version the application read (MEASURED).
- **Deletes are local-first, with tombstones.** Windows 10 ignores a delete
  veto, and the Recycle Bin path sends the delete before the rename that
  hydrates a cold file, so server-first deletes cannot be enforced
  (MEASURED). The delete is acknowledged at once, and the server delete
  fires a few seconds later, once the item is gone by file id and by name;
  a pending upload of the same name cancels it. File and folder deletes
  passed acceptance; the Recycle Bin case failed, and non-empty folders
  cannot go to the Recycle Bin (MEASURED). Kept only in memory, a tombstone
  is lost on restart and the file reappears.
- **Renames are server-first.** The rename is done on the server inside the
  callback, under a short deadline, and a server refusal refuses the local
  rename; Windows 10 and 11 both honour that veto (MEASURED; INSPECTED,
  [CF_CALLBACK_TYPE](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/ne-cfapi-cf_callback_type)).
  A rename clears the in-sync mark (MEASURED). A move out of the root
  arrives as a rename with no completion, and is treated as a delete
  (MEASURED).
- **The provider's own changes** raise no callbacks to it (MEASURED).
- **Offline.** With the provider down, new files and edits of hydrated files
  go through and nothing reports them; deletes and renames of placeholders
  are refused (MEASURED). A full scan at each connection finds the edits.
- **Latency.** A local save became in sync in about 3 to 4 s; a second AFS
  client saw it after 57 to 115 s, including its own refresh interval
  (MEASURED).

## 6. Failure modes

- **The `CfExecute` wedge.** Killing the provider while it is inside
  `CfExecute` wedged its root until reboot, and slowed every other root, in
  9 of 10 trials on Windows 11 and 10 (MEASURED). Moving data production
  into a separate process made 10 of 10 kills of that process clean, and a
  crash handler that disconnects every root survived 5 of 5 trials; killing
  the process that holds the connection during a call can still need a
  reboot (MEASURED). Microsoft does not document this.
- **Timeouts.** Every callback has "a fixed 60 second timeout", and any
  valid operation resets the timers of all pending requests (INSPECTED,
  [CF_CALLBACK_TYPE](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/ne-cfapi-cf_callback_type),
  [CfExecute](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/nf-cfapi-cfexecute)).
  The reset applied across roots (MEASURED), so the 60 s timer is no bound:
  the provider needs its own deadlines per read, per fetch and per
  namespace operation. With the server unreachable, a cold read failed after
  about 4 s with Windows' cloud time-out text (MEASURED).
- **Directory listings.** An empty listing sent as a non-final transfer
  stalled for 60 s, and full population ignores enumeration filters
  (MEASURED). Re-enabling on-demand population on a populated directory
  re-fetches it (MEASURED). With the provider down, listings fail loudly
  (MEASURED).
- **Defender.** Besides forcing whole-file hydration (§4), Defender's
  machine-learning model quarantined an unsigned provider on Windows 10; the
  shipped files must be signed (MEASURED).
- **Session 0.** A session-0 process whose image is outside `%SystemRoot%`
  gets `ERROR_CLOUD_FILE_ACCESS_DENIED` on a dehydrated placeholder, and the
  provider receives no callback (MEASURED on Windows 11 and 10). Services and
  batch jobs therefore cannot read cold content. Microsoft documents a
  different `%SystemRoot%` rule, for which processes see placeholders'
  reparse points (INSPECTED,
  [Build a cloud sync engine](https://learn.microsoft.com/en-us/windows/win32/cfapi/build-a-cloud-file-sync-engine)),
  but not this failure.
- **Logoff and uninstall.** Logoff is not a service stop, and needs its own
  handling of per-session state. Unregistering a root removes cold
  placeholders and leaves hydrated files, owned by the service account
  (MEASURED).
- **Explorer.** Placeholders created in an open window appear only after a
  refresh unless created under a temporary name and renamed; Windows 10
  views drop shell change notifications; on Windows 11 an open view kept a
  case-only rename stale (MEASURED).

## 7. ProjFS, and why Cloud Files was chosen

ProjFS also projects a backing store into NTFS through a user-mode provider
(INSPECTED,
[ProjFS](https://learn.microsoft.com/en-us/windows/win32/projfs/projected-file-system)).

- Microsoft positions it for "high-speed backing data stores": it has no
  progress reporting and no online or offline state per file, and for slow
  stores Microsoft says to "consider instead using the Cloud Files API"
  (INSPECTED, same page).
- It is an optional component that must be enabled with administrator
  rights (INSPECTED,
  [Enabling ProjFS](https://learn.microsoft.com/en-us/windows/win32/projfs/enabling-windows-projected-file-system)).
  Cloud Files is on by default (MEASURED).
- It has no callback per write and no lock notification; it reports a
  modification only when a handle closes (INSPECTED,
  [PRJ_NOTIFICATION](https://learn.microsoft.com/en-us/windows/win32/api/projectedfslib/ne-projectedfslib-prj_notification)).
- Measured against Cloud Files, in one provider configuration (MEASURED):
  - open handles kept serving stale bytes after a server change;
  - there was no way to take a server change while keeping local edits;
  - hydration was whole-file, with no dehydration;
  - data was fetched inside `CreateFile`, with an undocumented 240 s wait;
  - no cancellation was delivered;
  - with the provider down, a short listing was returned as complete, and
    offline changes to full files went unnoticed.
- Two of those observations are about that configuration, not the API:
  Microsoft documents ranged data requests
  ([PRJ_GET_FILE_DATA_CB](https://learn.microsoft.com/en-us/windows/win32/api/projectedfslib/nc-projectedfslib-prj_get_file_data_cb))
  and an optional cancellation callback
  ([PRJ_CANCEL_COMMAND_CB](https://learn.microsoft.com/en-us/windows/win32/api/projectedfslib/nc-projectedfslib-prj_cancel_command_cb))
  (INSPECTED by a research pass). Whole-file hydration and missing
  cancellation are therefore NOT VERIFIED as limits of ProjFS.

Both share the structural gaps: no caller identity, no per-write callback,
no byte-range locks, no UNC namespace. On the evidence available, Cloud
Files is the better-supported candidate: Microsoft recommends it over ProjFS
for slow stores and ProjFS is an optional component, and in the
configurations measured Cloud Files was better on progressive hydration,
cancellation, loud failures when the provider is down, refused offline
deletes and the in-sync guards. A working write path was built on Cloud
Files with a change watcher and scans (MEASURED). Whether ProjFS's
notifications support an equivalent write recovery was not tested in a
comparable configuration, so it stays open.

## 8. Open problems

- **Hydrated data without a token.** Cached files stay readable to anyone
  the DACL admits, with no AFS token and no per-open re-check (§3).
- **The connection-holder crash** can still need a reboot (§6).
- **Whole-file hydration** under Defender, memory maps and first writes;
  excluding the root from real-time scanning is an administrator's choice.
- **Session-0 readers** cannot hydrate (§6).
- **Silent loss through mapped views** flushed after the in-sync mark: never
  measured (§5).
- **Stale editors** can overwrite a newer server version (§5).
- **No durable record across restarts:** in-memory bases and tombstones turn
  pending overwrites into conflict pairs and bring deleted files back.
- **Offline deletes and renames** are refused, not queued.
- **No locks or share modes** reach the server: no Office lock files, no
  co-authoring, no server-side locks.
- **Cache size:** dehydration and pinning policy are not designed yet.
- **Shell registration** depends on a registry layout Microsoft does not
  commit to.
- **Windows 10 writing:** no recorded write acceptance run.
- **No identity finer than the user:** no per-session or per-process
  tokens, and no `\\server` path.

## Sources

- Microsoft, Cloud Files: [Build a cloud sync engine](https://learn.microsoft.com/en-us/windows/win32/cfapi/build-a-cloud-file-sync-engine), [Cloud Filter API reference](https://learn.microsoft.com/en-us/windows/win32/cfapi/cloud-filter-reference), [CF_CALLBACK_TYPE](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/ne-cfapi-cf_callback_type), [CfExecute](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/nf-cfapi-cfexecute), [CF_PROCESS_INFO](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/ns-cfapi-cf_process_info), [CF_CONNECT_FLAGS](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/ne-cfapi-cf_connect_flags), [CF_HYDRATION_POLICY_PRIMARY](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/ne-cfapi-cf_hydration_policy_primary), [CfUpdatePlaceholder](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/nf-cfapi-cfupdateplaceholder), [CfSetInSyncState](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/nf-cfapi-cfsetinsyncstate), [CfRegisterSyncRoot](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/nf-cfapi-cfregistersyncroot), [CF_PLACEHOLDER_CREATE_INFO](https://learn.microsoft.com/en-us/windows/win32/api/cfapi/ns-cfapi-cf_placeholder_create_info)
- Microsoft, ProjFS: [Projected File System](https://learn.microsoft.com/en-us/windows/win32/projfs/projected-file-system), [Enabling ProjFS](https://learn.microsoft.com/en-us/windows/win32/projfs/enabling-windows-projected-file-system), [PRJ_NOTIFICATION](https://learn.microsoft.com/en-us/windows/win32/api/projectedfslib/ne-projectedfslib-prj_notification)
- Microsoft, security and files: [LocalService account](https://learn.microsoft.com/en-us/windows/win32/services/localservice-account), [Mandatory Integrity Control](https://learn.microsoft.com/en-us/windows/win32/secauthz/mandatory-integrity-control), [AccessCheck](https://learn.microsoft.com/en-us/windows/win32/api/securitybaseapi/nf-securitybaseapi-accesscheck), [ReadDirectoryChangesW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-readdirectorychangesw)
- Measurements: the maintainer's, on the virtual machines named above.
