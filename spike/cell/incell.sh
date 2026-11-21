#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
# SPDX-License-Identifier: GPL-3.0-or-later
#
# The in-container half of the #47 cell spike (throwaway; never merged).
# run.sh calls one subcommand per check; each exits 0 on success. Steps that
# touch keys write their output to files inside the container and print only
# their name and exit status. Nothing here prints a key, a keytab or the KDC
# master password; there is no `set -x`.

set -uo pipefail
# shellcheck source=spike/cell/env.sh
. /spike/env.sh

SRV=/etc/openafs/server
DOCS=/usr/local/share/spike-docs
APT_SIG_BAD='NO_PUBKEY|EXPKEYSIG|BADSIG|is not signed|GPG error'
APT_BAD="$APT_SIG_BAD|^W:|^E:|^Err:"

say() { printf '%s\n' "$*"; }
die() { printf 'FAILED: %s\n' "$*" >&2; exit 1; }

# poll <seconds> <command...>: retry every 2 s until it succeeds or time runs
# out; on time-out show the last attempt's output tail.
poll() {
  local deadline=$((SECONDS + $1)) out
  shift
  out=$(mktemp)
  until "$@" >"$out" 2>&1; do
    if ((SECONDS >= deadline)); then
      tail -n 20 "$out" >&2
      rm -f "$out"
      return 1
    fi
    sleep 2
  done
  rm -f "$out"
}

# step <name> <logfile> <command...>: run quietly, report name and status
# only, and keep the output in the container.
step() {
  local name=$1 log=$2 rc
  shift 2
  "$@" >>"$log" 2>&1
  rc=$?
  say "step $name: exit $rc"
  return "$rc"
}

write_krb5_conf() {
  cat >/etc/krb5.conf <<EOF
[libdefaults]
    default_realm = $REALM
    dns_lookup_kdc = false
    dns_lookup_realm = false
    rdns = false

[realms]
    $REALM = {
        kdc = $IP_KDC
        admin_server = $IP_KDC
    }

[domain_realm]
    .$CELL = $REALM
    $CELL = $REALM
EOF
}

write_cell_config() {
  local dir
  for dir in /etc/openafs "$SRV"; do
    mkdir -p "$dir"
    printf '%s\n' "$CELL" >"$dir/ThisCell"
    printf '>%s #synthetic test cell\n%s #db\n' "$CELL" "$IP_DB" >"$dir/CellServDB"
  done
}

# Manual pages and docs, as plain text, from the packages (ADR 0001: package
# documentation only).
find_page() { find "$DOCS" /usr/share/man -name "$1" 2>/dev/null | head -n 1; }
man_text() { # <file>: strip roff markup
  zcat -f "$1" | sed -e '/^\.\\"/d' -e 's/^\.[A-Za-z]* *//' -e 's/\\f[BIRP]//g' -e 's/\\-/-/g' |
    grep -v '^$'
}

find_in_package() { # find_in_package <package> <basename>
  dpkg -L "$1" 2>/dev/null | grep -E "/$2\$" | head -n 1
}

# apt_update <base URI> <CA file or "">: point apt at the snapshot and update.
# Sets APT_OPTS for later apt calls. Returns 1 on a transport failure; a
# signature problem is fatal and never retried over another transport.
APT_OPTS=()
APT_KEY=""
apt_update() {
  local base=$1 ca=$2
  cat >/etc/apt/sources.list.d/cell.sources <<EOF
Types: deb
URIs: $base
Suites: $SNAPSHOT_SUITE
Components: $SNAPSHOT_COMPONENT
Signed-By: $APT_KEY
EOF
  APT_OPTS=(-o Acquire::Retries=5)
  [[ -n $ca ]] && APT_OPTS+=(-o "Acquire::https::CaInfo=$ca")
  rm -rf /var/lib/apt/lists/*
  if apt-get "${APT_OPTS[@]}" update >/tmp/apt-update.log 2>&1 &&
    ! grep -Eq "$APT_BAD" /tmp/apt-update.log; then
    say "apt-get update: signed InRelease accepted from $base${ca:+ (CA file $ca)}"
    return 0
  fi
  tail -n 20 /tmp/apt-update.log
  if grep -Eq "$APT_SIG_BAD" /tmp/apt-update.log; then
    die "apt-get update: signature problem (never retried over another transport)"
  fi
  say "apt-get update: transport failure over $base${ca:+ (CA file $ca)}"
  return 1
}

cmd_install() {
  local arch=$1 key ok=0 transport="" line name pin file got pkg want have
  export DEBIAN_FRONTEND=noninteractive
  printf '#!/bin/sh\nexit 101\n' >/usr/sbin/policy-rc.d
  chmod 0755 /usr/sbin/policy-rc.d

  key=$(sed -n 's/^Signed-By:[[:space:]]*//p' /etc/apt/sources.list.d/debian.sources 2>/dev/null | head -n 1)
  if [[ -z $key || ! -f $key ]]; then
    for key in /usr/share/keyrings/debian-archive-keyring.pgp /usr/share/keyrings/debian-archive-keyring.gpg; do
      [[ -f $key ]] && break
    done
  fi
  [[ -f $key ]] || die "no Debian archive keyring in the image"
  say "archive keyring: $key"
  rm -f /etc/apt/sources.list /etc/apt/sources.list.d/*
  APT_KEY=$key

  # 1. HTTPS with the runner's CA bundle, named explicitly to apt.
  if [[ -s $HOST_CA ]]; then
    say "runner CA bundle: $(grep -c 'BEGIN CERTIFICATE' "$HOST_CA") certificates"
    apt_update "$SNAPSHOT_HTTPS" "$HOST_CA" && { ok=1; transport="https (runner CA bundle)"; }
  else
    say "runner CA bundle: not mounted"
  fi
  # 2. ca-certificates from the same signed snapshot over HTTP, then HTTPS
  #    with that bundle for everything else.
  if ((!ok)) && apt_update "$SNAPSHOT_HTTP" ""; then
    if apt-get "${APT_OPTS[@]}" install -y --no-install-recommends ca-certificates \
      >/tmp/apt-ca.log 2>&1 && [[ -s /etc/ssl/certs/ca-certificates.crt ]]; then
      say "ca-certificates $(dpkg-query -W -f '${Version}' ca-certificates) installed from the signed snapshot"
      if apt_update "$SNAPSHOT_HTTPS" /etc/ssl/certs/ca-certificates.crt; then
        ok=1
        transport="https (ca-certificates from the snapshot)"
      fi
    else
      tail -n 20 /tmp/apt-ca.log
      say "ca-certificates bootstrap failed"
    fi
    # 3. Plain HTTP, reached only after HTTPS failed in transport.
    if ((!ok)) && apt_update "$SNAPSHOT_HTTP" ""; then
      ok=1
      transport="http (fallback after HTTPS transport failures)"
    fi
  fi
  ((ok)) || die "apt-get update failed over every transport"
  say "transport: $transport"

  apt-get "${APT_OPTS[@]}" install -y --no-install-recommends --download-only \
    "${PACKAGES[@]}" >/tmp/apt-download.log 2>&1 ||
    { tail -n 20 /tmp/apt-download.log; die "apt-get download"; }

  while read -r line; do
    [[ -z $line ]] && continue
    read -r a name pin <<<"$line"
    [[ $a == "$arch" ]] || continue
    file=$(compgen -G "/var/cache/apt/archives/${name}_*_${arch}.deb" | head -n 1)
    [[ -n $file && -f $file ]] || die "hash: $name was not downloaded"
    got=$(sha256sum "$file" | cut -d' ' -f1)
    if [[ $pin == *...* ]]; then
      [[ $got == "${pin%%...*}"* && $got == *"${pin##*...}" ]] ||
        die "hash: $name is $got, pin $pin"
      say "hash: $name matches the pinned prefix and suffix"
    else
      [[ $got == "$pin" ]] || die "hash: $name is $got, pin $pin"
      say "hash: $name matches the full pin"
    fi
  done <<<"$HASH_PINS"

  apt-get "${APT_OPTS[@]}" install -y --no-install-recommends "${PACKAGES[@]}" >/tmp/apt-install.log 2>&1 ||
    { tail -n 20 /tmp/apt-install.log; die "apt-get install"; }

  for pkg in "${PACKAGES[@]}"; do
    name=${pkg%%=*}
    want=${pkg#*=}
    have=$(dpkg-query -W -f '${Version}' "$name" 2>/dev/null)
    [[ $have == "$want" ]] || die "version: $name is '$have', pin $want"
    say "version: $name $have"
  done
  if dpkg-query -W -f '${Package}\n' | grep -Ei 'openafs-modules'; then
    die "a kernel module package was installed"
  fi
  say "no openafs-modules package installed"
  # Package documentation (ADR 0001 allows manual pages and package docs):
  # extracted from the verified .deb files, since the image's dpkg
  # configuration may exclude /usr/share/man and /usr/share/doc.
  mkdir -p "$DOCS"
  for file in /var/cache/apt/archives/openafs-*.deb; do
    dpkg-deb --fsys-tarfile "$file" |
      tar -x -C "$DOCS" --wildcards './usr/share/man/*' './usr/share/doc/*' 2>/dev/null
  done
  say "package docs kept: $(find "$DOCS" -type f | wc -l) files"
  rm -rf /var/lib/apt/lists/* /var/cache/apt/archives/*.deb
  rm -f /etc/apt/sources.list.d/cell.sources
}

cmd_kdc_init() {
  local p log=/run/cell/kdc-init.log
  umask 077
  mkdir -p /run/cell
  write_krb5_conf
  mkdir -p /etc/krb5kdc /var/lib/krb5kdc
  cat >/etc/krb5kdc/kdc.conf <<EOF
[kdcdefaults]
    kdc_ports = 88
    kdc_tcp_ports = 88

[realms]
    $REALM = {
        database_name = /var/lib/krb5kdc/principal
        key_stash_file = /etc/krb5kdc/stash
        master_key_type = aes256-cts-hmac-sha1-96
        supported_enctypes = aes256-cts-hmac-sha1-96:normal
    }

[logging]
    kdc = FILE:/var/log/krb5kdc.log
EOF
  # The master password exists only in this shell, for this run.
  # shellcheck disable=SC2016 # expanded by the inner shell, on purpose
  step "kdb5_util create" "$log" sh -c \
    'pw=$(head -c 32 /dev/urandom | base64 -w0) && exec kdb5_util create -s -r "$1" -P "$pw"' \
    sh "$REALM" || return 1
  step "krb5kdc start" "$log" krb5kdc || return 1
  for p in "afs/$CELL" admin user1 user2 writer; do
    step "addprinc $p" "$log" kadmin.local -r "$REALM" -q "addprinc -randkey $p@$REALM" || return 1
  done
  step "ktadd afs" "$log" kadmin.local -r "$REALM" \
    -q "ktadd -k /run/cell/rxkad.keytab -e aes256-cts-hmac-sha1-96:normal afs/$CELL@$REALM" || return 1
  step "ktadd admin" "$log" kadmin.local -r "$REALM" \
    -q "ktadd -k /run/cell/admin.keytab admin@$REALM" || return 1
  [[ -s /run/cell/rxkad.keytab && -s /run/cell/admin.keytab ]] || { say "keytabs missing"; return 1; }
  if poll 30 kinit -k -t /run/cell/admin.keytab "admin@$REALM"; then
    kdestroy >/dev/null 2>&1
    say "kdc answers: kinit with a per-run keytab succeeded"
  else
    say "kdc does not answer kinit"
    return 1
  fi
}

# The key version number only (no key bytes): `klist -k` without -K.
cmd_kvno() {
  klist -k /run/cell/rxkad.keytab 2>/dev/null |
    awk -v p="afs/$CELL@$REALM" '$2 == p { print $1; exit }'
}

cmd_cat_keytab() { cat /run/cell/rxkad.keytab; }

# server-setup <db|fs1|fs2> <kvno>; the cell's keytab arrives on stdin.
cmd_server_setup() {
  local role=$1 kvno=$2
  [[ $kvno =~ ^[0-9]+$ ]] || die "bad kvno"
  write_cell_config
  write_krb5_conf
  mkdir -p /var/lib/openafs/db /var/lib/openafs/local /var/log/openafs
  printf 'admin\n' >"$SRV/UserList"
  (umask 077 && cat >"$SRV/rxkad.keytab")
  [[ -s $SRV/rxkad.keytab ]] || die "no keytab received"
  if step "asetkey add rxkad_krb5" /tmp/keys.log \
    asetkey add rxkad_krb5 "$kvno" 18 "$SRV/rxkad.keytab" "afs/$CELL@$REALM"; then
    :
  else
    step "akeyconvert -all (fallback)" /tmp/keys.log akeyconvert -all || return 1
  fi
  if [[ $role == fs* ]]; then
    mkdir -p /vicepa
    touch /vicepa/AlwaysAttach
  fi
}

cmd_client_setup() {
  write_cell_config
  write_krb5_conf
}

cmd_bosserver() {
  local bos
  bos=$(command -v bosserver || echo /usr/sbin/bosserver)
  exec "$bos" -nofork >/var/log/openafs/bosserver.stdout 2>&1
}

cmd_db_instances() {
  local pt vl
  pt=$(find_in_package openafs-dbserver ptserver)
  vl=$(find_in_package openafs-dbserver vlserver)
  [[ -n $pt && -n $vl ]] || die "ptserver or vlserver binary not found"
  poll 60 bos listhosts "$IP_DB" -localauth || die "bosserver on db does not answer"
  bos create "$IP_DB" ptserver simple "$pt" -localauth || return 1
  bos create "$IP_DB" vlserver simple "$vl" -localauth || return 1
}

cmd_fs_instances() {
  local ip=$1 fsb vol salv
  fsb=$(find_in_package openafs-fileserver fileserver)
  vol=$(find_in_package openafs-fileserver volserver)
  salv=$(find_in_package openafs-fileserver salvager)
  [[ -n $fsb && -n $vol && -n $salv ]] || die "fileserver binaries not found"
  poll 60 bos listhosts "$ip" -localauth || die "bosserver on $ip does not answer"
  bos create "$ip" fs fs "$fsb" "$vol" "$salv" -localauth || return 1
}

instances_running() { # <ip> <instance...>
  local ip=$1 out i
  shift
  out=$(bos status "$ip" -localauth) || return 1
  for i in "$@"; do
    grep -q "Instance $i, currently running normally" <<<"$out" || { printf '%s\n' "$out"; return 1; }
  done
}

cmd_wait_running() { # <ip> <seconds> <instance...>
  local ip=$1 secs=$2
  shift 2
  poll "$secs" instances_running "$ip" "$@" || return 1
  bos status "$ip" -localauth
}

cmd_pts_setup() {
  local u
  poll 180 pts listentries -localauth || die "ptserver does not answer"
  for u in admin user1 user2 writer; do
    pts createuser -name "$u" -localauth >/tmp/pts.log 2>&1 ||
      grep -q 'already exists' /tmp/pts.log || { cat /tmp/pts.log; return 1; }
  done
  pts adduser -user admin -group system:administrators -localauth
}

fs_registered() {
  local out
  out=$(vos listaddrs -noresolve -localauth) || return 1
  grep -qx "$IP_FS1" <<<"$out" && grep -qx "$IP_FS2" <<<"$out"
}

cmd_volumes() {
  local v ip out
  poll 180 fs_registered || die "fileservers not registered in the VLDB"
  for ip in "$IP_FS1" "$IP_FS2"; do
    poll 120 sh -c "vos listpart $ip -localauth | grep -q /vicepa" || die "no /vicepa on $ip"
  done
  for v in root.afs root.cell; do
    vos create "$IP_FS1" a "$v" -localauth || return 1
    vos addsite "$IP_FS1" a "$v" -localauth || return 1
    vos addsite "$IP_FS2" a "$v" -localauth || return 1
    vos release "$v" -localauth || return 1
  done
  out=$(vos listvldb -name root.cell -noresolve -localauth) || return 1
  printf '%s\n' "$out"
  for ip in "$IP_FS1" "$IP_FS2"; do
    grep -Eq "server ${ip//./\\.} partition /vicepa RO Site" <<<"$out" ||
      die "no RO site of root.cell on $ip"
  done
  vos listvol "$IP_FS2" a -localauth | grep -q 'root\.cell\.readonly' ||
    die "root.cell.readonly is not on fs2"
}

cmd_rxdebug() { # <ip>
  poll 30 sh -c "rxdebug $1 7000 -version | grep -q 'AFS version'" || return 1
  rxdebug "$1" 7000 -version | grep 'AFS version'
}

# --- Seeding (rounds 2 and 3) -----------------------------------------------
#
# Four routes, each writing its own file in root.cell's root directory:
#   afsio         kinit with the admin keytab, aklog, afsio write -realm;
#   noauth-afsio  (round 3) afsio write with no credentials while NoAuth is
#                 on (the fileservers then treat every caller as an
#                 administrator);
#   fuse-token    afsd.fuse mount, kinit, aklog, write through the mount,
#                 fs setacl system:anyuser rl;
#   noauth-fuse   bos setauth off on both fileservers (-localauth, from the
#                 db container), an unauthenticated afsd.fuse writes and
#                 sets the ACL, bos setauth on again.
# run.sh releases root.cell after each route, then reads every file from
# containers that hold no keytab, no ticket and no token.

seed_file() { printf '/afs/%s/hello-%s.txt' "$1" "$2"; } # <cell or .cell> <route>

# The admin keytab arrives on stdin (piped from the KDC container) and stays
# in this container's /run/cell, mode 0600. Never printed.
cmd_recv_admin_keytab() {
  write_krb5_conf
  mkdir -p /run/cell
  (umask 077 && cat >/run/cell/admin.keytab)
  [[ -s /run/cell/admin.keytab ]] || die "no admin keytab received"
  say "admin keytab received"
}

cmd_cat_admin_keytab() { cat /run/cell/admin.keytab; }

# kinit from the keytab; print only the principal names (klist without -K/-e).
cmd_kinit_admin() {
  timeout 60 kinit -k -t /run/cell/admin.keytab "admin@$REALM" || return 1
  klist 2>&1 | grep -E 'Default principal|afs/|krbtgt/' | head -n 5
}

# aklog for the synthetic cell, then tokens (cell and expiry only). Passes if
# tokens lists one for this cell.
cmd_aklog_token() {
  local rc out
  timeout 60 aklog -c "$CELL" -k "$REALM"
  rc=$?
  say "aklog exit $rc"
  out=$(timeout 30 tokens 2>&1)
  say "tokens exit $?:"
  printf '%s\n' "$out" | grep -v '^$' | head -n 10
  ((rc == 0)) && grep -q "afs@$CELL" <<<"$out"
}

cmd_afsio_help() {
  local page
  afsio help 2>&1 | head -n 40
  afsio write -help 2>&1 | head -n 20
  afsio read -help 2>&1 | head -n 20
  # The packaged manual page, as text (package documentation).
  page=$(find_page 'afsio.1*')
  if [[ -n $page ]]; then
    say "--- ${page#"$DOCS"} (roff stripped) ---"
    man_text "$page" | head -n 120
  else
    say "no afsio manual page in the packages"
  fi
}

# afsio-write <route> <auth|anon>. auth: with the Kerberos ticket from
# kinit-admin (-realm). anon: no credentials at all, for the NoAuth window,
# where the fileservers treat every caller as an administrator. Round 2:
# afsio write with -realm died with SIGSEGV on all four runners, so each
# attempt prints its exit status (and signal) and tries -clear as well. The
# dotted (read-write) path first, then the plain one; read back what landed.
cmd_afsio_write() {
  local route=$1 how=${2:-auth} path back rc extra out
  local -a auth=()
  if [[ $how == auth ]]; then
    auth=(-realm "$REALM")
  else
    assert_no_credentials
  fi
  for path in "$(seed_file ".$CELL" "$route")" "$(seed_file "$CELL" "$route")"; do
    for extra in "" -clear; do
      out=$(printf '%s\n' "$SEED_TEXT" |
        timeout 120 afsio write -file "$path" -cell "$CELL" "${auth[@]}" ${extra:+"$extra"} -verbose 2>&1)
      rc=$?
      printf '%s\n' "$out" | tail -n 8
      say "afsio write ${extra:-(default)} ($how) to $path: exit $rc$( ((rc > 128)) && echo " (signal $((rc - 128)))")"
      ((rc == 0)) || continue
      back=$(timeout 60 afsio read -file "$path" -cell "$CELL" "${auth[@]}" ${extra:+"$extra"}) || return 1
      [[ $back == "$SEED_TEXT" ]] || die "afsio read-back differs"
      say "afsio: wrote $path and read it back"
      return 0
    done
  done
  return 1
}

# No keytab, no ticket cache, no token in this container.
assert_no_credentials() {
  kdestroy -A >/dev/null 2>&1
  unset KRB5CCNAME
  [[ ! -e /run/cell/admin.keytab ]] || die "this container holds the admin keytab"
  if klist >/dev/null 2>&1; then die "this container holds a ticket"; fi
  say "no keytab, no ticket cache"
}

# Anonymous read through afsio of one route's file, from the plain (RO) path.
cmd_afsio_read_anon() {
  local route=$1 back
  assert_no_credentials
  back=$(timeout 60 afsio read -file "$(seed_file "$CELL" "$route")" -cell "$CELL") || return 1
  [[ $back == "$SEED_TEXT" ]] || die "anonymous afsio read returned other bytes"
  say "anonymous afsio read of $route: ${#back} bytes, as written"
}

# Route (ii) and (iii): afsd.fuse, a user-space cache manager over /dev/fuse.
cmd_fuse_prepare() {
  cmd_client_setup
  write_krb5_conf
  mkdir -p /afs /var/cache/openafs
  printf '/afs:/var/cache/openafs:50000\n' >/etc/openafs/cacheinfo
  command -v afsd.fuse || die "afsd.fuse is not installed"
}

# Round 2: the mount appeared, then every access gave "Transport endpoint is
# not connected", with an empty log. The round-2 form let FUSE fork into the
# background, which drops stdout and stderr, so the reason was lost. Round 3
# runs it in the foreground (FUSE's -f and -d; afsd's -verbose), keeps its
# output and exit status, and varies one thing at a time. afsd's -debug
# trace is not used: its detail could carry internal names into the log,
# which is later read by a model (ADR 0001).
fuse_variant_args() { # <variant>: sets FUSE_ARGS
  case $1 in
    bg) FUSE_ARGS=(-dynroot -fakestat /afs) ;;                 # round 2's form
    fg) FUSE_ARGS=(-dynroot -fakestat -f /afs) ;;              # foreground
    fg-debug) FUSE_ARGS=(-dynroot -fakestat -d /afs) ;;        # FUSE debug (implies -f)
    fg-verbose) FUSE_ARGS=(-dynroot -fakestat -verbose -f /afs) ;;
    fg-nosettime) FUSE_ARGS=(-dynroot -fakestat -nosettime -f /afs) ;;
    fg-bare) FUSE_ARGS=(-f /afs) ;;                            # no afsd options
    fg-single) FUSE_ARGS=(-dynroot -fakestat -f -s /afs) ;;    # FUSE single-threaded
    *) return 1 ;;
  esac
}
FUSE_VARIANTS=(bg fg fg-debug fg-verbose fg-nosettime fg-bare fg-single)

# No procps in the image (round 2's pgrep check always said NOT running).
afsd_pids() {
  local p
  for p in /proc/[0-9]*; do
    [[ $(cat "$p/comm" 2>/dev/null) == afsd.fuse ]] && printf '%s\n' "${p#/proc/}"
  done
}
afsd_alive() { [[ -n $(afsd_pids) ]]; }

fuse_unmount() {
  local pid
  for pid in $(afsd_pids); do kill "$pid" 2>/dev/null; done
  sleep 1
  for pid in $(afsd_pids); do kill -9 "$pid" 2>/dev/null; done
  grep -q ' /afs fuse' /proc/mounts && umount -l /afs 2>/dev/null
  mkdir -p /afs
}

fuse_mounted() { grep -Eq '^[^ ]+ /afs fuse' /proc/mounts; }

# Serving: mounted, a process alive, and the root answers a stat.
fuse_serving() { fuse_mounted && afsd_alive && timeout 5 stat /afs >/dev/null 2>&1; }

fuse_log_tail() {
  say "--- afsd.fuse: $(afsd_alive && echo "running (pid $(afsd_pids | tr '\n' ' '))" || echo NOT running); log tail ---"
  tail -n 30 /var/log/afsd.fuse.log 2>/dev/null
}

# fuse-run <variant>: in the foreground of a detached `docker exec`; the
# output and the exit status go to the log.
cmd_fuse_run() {
  local rc
  fuse_variant_args "${1:-fg}" || die "unknown variant ${1:-}"
  say "afsd.fuse ${FUSE_ARGS[*]}" >/var/log/afsd.fuse.log
  afsd.fuse "${FUSE_ARGS[@]}" >>/var/log/afsd.fuse.log 2>&1
  rc=$?
  say "afsd.fuse exit status $rc$( ((rc > 128)) && echo " (signal $((rc - 128)))")" >>/var/log/afsd.fuse.log
}

cmd_fuse_wait() {
  if poll 60 fuse_serving; then
    grep -E ' /afs fuse' /proc/mounts
    say "afsd.fuse running (pid $(afsd_pids | tr '\n' ' ')), /afs answers stat"
    return 0
  fi
  grep -E ' /afs fuse' /proc/mounts
  fuse_log_tail
  return 1
}

# fuse-diag: each variant for up to 20 s in this container; probe at 6 s;
# print its output (head and tail), its exit status and whether it served.
# The first variant that serves is printed as "fuse-variant-ok: <name>".
cmd_fuse_diag() {
  local v pid rc alive mounted served lsout chosen="" log=/tmp/fuse-diag.log
  say "fuse.conf: $(grep -v '^#' /etc/fuse.conf 2>/dev/null | grep -v '^$' | tr '\n' ' ' || echo absent)"
  say "fusermount: $(command -v fusermount3 fusermount | tr '\n' ' ')"
  say "/dev/fuse: $(ls -l /dev/fuse 2>&1)"
  say "/proc/fs/openafs: $([[ -e /proc/fs/openafs ]] && echo present || echo absent)"
  say "uid: $(id -u); cacheinfo: $(cat /etc/openafs/cacheinfo)"
  say "--- afsd.fuse -help (5 s) ---"
  timeout 5 afsd.fuse -help 2>&1 | head -n 60
  fuse_unmount
  for v in "${FUSE_VARIANTS[@]}"; do
    fuse_variant_args "$v"
    say "=== variant $v: afsd.fuse ${FUSE_ARGS[*]}"
    : >"$log"
    timeout 20 afsd.fuse "${FUSE_ARGS[@]}" >"$log" 2>&1 &
    pid=$!
    sleep 6
    afsd_alive && alive=yes || alive=no
    fuse_mounted && mounted=yes || mounted=no
    served=no
    if lsout=$(timeout 5 ls -la /afs 2>&1); then served=yes; fi
    say "at 6 s: process alive=$alive mounted=$mounted ls /afs: $(head -n 3 <<<"$lsout" | tr '\n' ' ')"
    if [[ $served == yes ]]; then
      say "ls /afs/$CELL: $(timeout 10 ls "/afs/$CELL" 2>&1 | head -n 3 | tr '\n' ' ')"
      say "fs wscell: $(timeout 10 fs wscell 2>&1 | tr '\n' ' ')"
      say "fs listacl /afs/.$CELL: $(timeout 10 fs listacl -path "/afs/.$CELL" 2>&1 | tr '\n' ' ')"
    fi
    wait "$pid"
    rc=$?
    say "exit status $rc$( ((rc == 124)) && echo ' (still running at 20 s, stopped by timeout)')$( ((rc > 128)) && echo " (signal $((rc - 128)))")"
    say "--- output ($(wc -l <"$log") lines; head 40, tail 15) ---"
    if (($(wc -l <"$log") > 55)); then
      head -n 40 "$log"
      say "[...]"
      tail -n 15 "$log"
    else
      cat "$log"
    fi
    if [[ -z $chosen && $served == yes ]]; then chosen=$v; fi
    fuse_unmount
  done
  if [[ -n $chosen ]]; then
    say "fuse-variant-ok: $chosen"
    return 0
  fi
  say "fuse-variant-ok: none"
  return 1
}

# Can the mount be walked at all? The dynroot, then the cell (RO path). And
# does fs reach afsd.fuse (its pioctl path)? fs results are printed only.
cmd_fuse_probe() {
  local rc=0
  timeout 30 ls -la /afs || rc=1
  timeout 30 ls -la "/afs/$CELL" || rc=1
  timeout 30 ls -la "/afs/.$CELL" || rc=1
  say "fs wscell: $(timeout 10 fs wscell 2>&1 | tr '\n' ' ')"
  say "fs listacl: $(timeout 10 fs listacl -path "/afs/.$CELL" 2>&1 | tr '\n' ' ')"
  say "fs examine: $(timeout 10 fs examine -path "/afs/.$CELL" 2>&1 | tr '\n' ' ')"
  ((rc)) && fuse_log_tail
  return "$rc"
}

# Write one route's file through the mount (RW path) and read it back.
cmd_fuse_write() {
  local route=$1 back
  printf '%s\n' "$SEED_TEXT" >"$(seed_file ".$CELL" "$route")" || { fuse_log_tail; return 1; }
  back=$(timeout 30 cat "$(seed_file ".$CELL" "$route")") || { fuse_log_tail; return 1; }
  [[ $back == "$SEED_TEXT" ]] || die "fuse read-back differs"
  say "fuse: wrote $(seed_file ".$CELL" "$route") and read it back"
}

# fs setacl on root.cell's root (RW path), then fs listacl to check it.
cmd_fuse_setacl() {
  local out
  timeout 30 fs setacl -dir "/afs/.$CELL" -acl system:anyuser rl || { fuse_log_tail; return 1; }
  out=$(timeout 30 fs listacl -path "/afs/.$CELL") || { fuse_log_tail; return 1; }
  printf '%s\n' "$out"
  grep -Eq '^[[:space:]]*system:anyuser[[:space:]]+rl$' <<<"$out" || die "system:anyuser rl not on the ACL"
}

cmd_fuse_read_anon() {
  local route=$1 back
  assert_no_credentials
  back=$(timeout 60 cat "$(seed_file "$CELL" "$route")") || { fuse_log_tail; return 1; }
  [[ $back == "$SEED_TEXT" ]] || die "anonymous fuse read returned other bytes"
  say "anonymous fuse read of $route: ${#back} bytes, as written"
}

# --- Diagnostics (round 3) ---------------------------------------------------

# docs: what the packages document about afsd.fuse, its options, ACLs on new
# volumes and pioctl without a kernel module. Package documentation only.
cmd_docs() {
  local f page pat
  say "dpkg path excludes: $(cat /etc/dpkg/dpkg.cfg.d/* 2>/dev/null | grep -c path-exclude) lines"
  say "--- dpkg -L openafs-fuse ---"
  dpkg -L openafs-fuse 2>/dev/null | grep -v '/$' | head -n 40
  say "--- extracted docs: names matching afsd, fuse, vos_create, fs_setacl, README ---"
  find "$DOCS" -type f | grep -Ei 'afsd|fuse|vos_create|fs_setacl|fs_listacl|README|NEWS' | sed "s|^$DOCS||" | head -n 40
  for pat in 'afsd.fuse*' 'afsd.8*'; do
    page=$(find_page "$pat")
    [[ -n $page ]] || { say "no page $pat"; continue; }
    say "--- ${page#"$DOCS"}: synopsis and the options this spike uses ---"
    man_text "$page" | sed -n '1,25p'
    man_text "$page" | grep -n -A4 -E '^-(dynroot|fakestat|nosettime|settime|verbose|debug|mountdir|cachedir|blocks|files|stat|daemons|nomount)\b' | head -n 90
    man_text "$page" | grep -n -i -B1 -A3 -E 'fuse|user.?space|without a kernel|pioctl' | head -n 40
  done
  for f in $(find "$DOCS" -type f -path '*doc/openafs-*' | grep -Ei 'README|NEWS|changelog\.Debian' | head -n 8); do
    say "--- ${f#"$DOCS"}: lines on fuse, afsd.fuse, pioctl, ACLs ---"
    zcat -f "$f" | grep -n -i -B1 -A3 -E 'fuse|pioctl|system:anyuser|default acl' | head -n 40
  done
  page=$(find_page 'vos_create.1*')
  if [[ -n $page ]]; then
    say "--- ${page#"$DOCS"}: what it says about the new volume's ACL ---"
    man_text "$page" | grep -n -i -B2 -A4 -E 'acl|access control|system:' | head -n 40
  else
    say "no vos_create page"
  fi
  page=$(find_page 'fs_setacl.1*')
  [[ -n $page ]] && { say "--- ${page#"$DOCS"}: privilege ---"; man_text "$page" | grep -n -i -A6 'PRIVILEGE REQUIRED' | head -n 12; }
  return 0
}

# Server side: the volume as the VLDB and the fileserver see it.
cmd_server_view() {
  timeout 60 vos examine root.cell -localauth 2>&1 | head -n 30
  timeout 60 vos listvol "$IP_FS1" a -localauth 2>&1 | head -n 12
}

# anon-probe: an anonymous afsio read of a file that does not exist, in
# root.cell's root, before any seeding. Its error tells whether anonymous
# may look up names there (the ACL a new volume gets). Always exits 0; the
# last line is "probe: exit N".
cmd_anon_probe() {
  local rc out
  assert_no_credentials
  out=$(timeout 60 afsio read -file "/afs/$CELL/no-such-file" -cell "$CELL" -verbose 2>&1)
  rc=$?
  printf '%s\n' "$out" | head -n 15
  say "probe: exit $rc$( ((rc > 128)) && echo " (signal $((rc - 128)))")"
}


# setauth <ip> on|off (db container, -localauth).
cmd_setauth() {
  timeout 60 bos setauth "$1" "$2" -localauth || return 1
  say "bos setauth $1 $2: done"
}

# noauth-file: does this server hold a NoAuth file? Prints yes or no.
cmd_noauth_file() {
  if compgen -G '/var/lib/openafs/local/NoAuth' >/dev/null || [[ -e /usr/afs/local/NoAuth ]]; then
    say yes
  else
    say no
  fi
}

cmd_release() {
  timeout 120 vos release root.cell -localauth || return 1
  vos examine root.cell -localauth 2>&1 | grep -E 'RO Site|Last Update' | head -n 6
}

sub=${1:-}
shift || true
case $sub in
  install) cmd_install "$@" ;;
  kdc-init) cmd_kdc_init ;;
  kvno) cmd_kvno ;;
  cat-keytab) cmd_cat_keytab ;;
  server-setup) cmd_server_setup "$@" ;;
  client-setup) cmd_client_setup ;;
  bosserver) cmd_bosserver ;;
  db-instances) cmd_db_instances ;;
  fs-instances) cmd_fs_instances "$@" ;;
  wait-running) cmd_wait_running "$@" ;;
  pts-setup) cmd_pts_setup ;;
  volumes) cmd_volumes ;;
  rxdebug) cmd_rxdebug "$@" ;;
  recv-admin-keytab) cmd_recv_admin_keytab ;;
  cat-admin-keytab) cmd_cat_admin_keytab ;;
  kinit-admin) cmd_kinit_admin ;;
  aklog-token) cmd_aklog_token ;;
  afsio-help) cmd_afsio_help ;;
  afsio-write) cmd_afsio_write "$@" ;;
  afsio-read-anon) cmd_afsio_read_anon "$@" ;;
  fuse-prepare) cmd_fuse_prepare ;;
  fuse-run) cmd_fuse_run "$@" ;;
  fuse-diag) cmd_fuse_diag ;;
  docs) cmd_docs ;;
  server-view) cmd_server_view ;;
  anon-probe) cmd_anon_probe ;;
  fuse-wait) cmd_fuse_wait ;;
  fuse-probe) cmd_fuse_probe ;;
  fuse-write) cmd_fuse_write "$@" ;;
  fuse-setacl) cmd_fuse_setacl ;;
  fuse-read-anon) cmd_fuse_read_anon "$@" ;;
  setauth) cmd_setauth "$@" ;;
  noauth-file) cmd_noauth_file ;;
  release) cmd_release ;;
  *) die "unknown subcommand '$sub'" ;;
esac
