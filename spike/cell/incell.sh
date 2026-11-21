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

# --- Seeding (rounds 2 to 4) -----------------------------------------------
#
# Two routes, each writing its own file in root.cell's root directory, both
# through afsd.fuse (round 3: afsio dies with SIGSEGV in Heimdal's
# libkrb5.so.26 on every call, with or without credentials, so its write
# routes are dropped and one diagnostic call is kept):
#   fuse-token    afsd.fuse mount, kinit, aklog, write through the mount,
#                 fs setacl system:anyuser rl if fs reaches afsd.fuse;
#   noauth-fuse   bos setauth off on both fileservers (-localauth, from the
#                 db container), an unauthenticated afsd.fuse writes through
#                 /afs/.<cell>/ and sets the ACL if fs reaches it, vos
#                 release, bos setauth on again, NoAuth checked gone.
# Then a fresh afsd.fuse in a container that holds no keytab, no ticket and
# no token reads every file that was written. Where fs could not set the
# ACL, the read shows whether a new volume's default ACL lets anonymous read.

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

# No keytab, no ticket cache, no token in this container.
assert_no_credentials() {
  kdestroy -A >/dev/null 2>&1
  unset KRB5CCNAME
  [[ ! -e /run/cell/admin.keytab ]] || die "this container holds the admin keytab"
  if klist >/dev/null 2>&1; then die "this container holds a ticket"; fi
  say "no keytab, no ticket cache"
}

# afsd.fuse, a user-space cache manager over /dev/fuse.
#
# Round 4: every flag-only start died within 6 s with "afsd: ASSERT:
# cacheFiles 1000  diskblocks -26" (cacheinfo /afs:<cache>:50000 in
# /etc/openafs, a 512 MiB tmpfs under the cache), leaving a dead mount; and
# every start with a valued switch failed to parse ("Too many values after
# switch -files" with `... -files 2000 -f /afs`). Round 5's variants test:
#   H1  afsd.fuse does not read /etc/openafs/cacheinfo (the cache size it
#       uses is neither 50000 nor the free space): it reads
#       /usr/vice/etc/cacheinfo (H1a, a copy of cacheinfo there), or all its
#       configuration from /usr/vice/etc (H1b, /usr/vice/etc -> /etc/openafs),
#       or -confdir names the directory (H1c);
#   H2  the FUSE arguments (-f, -s, the mount point) are taken out before
#       afsd's switches are parsed, so a trailing mount point becomes another
#       value of the last valued switch: valued switches first and a flag last
#       parse (H2a), the mount point first parses (H2b), `--` ends the values
#       (H2c), and without a mount point the switches parse (H2d);
#   H3  -blocks overrides the cache size, for a disk cache (H3a) and a memory
#       cache (H3b).
# The cacheinfo format (afsd and cacheinfo manual pages): one line,
# <mount dir>:<cache dir>:<cache size in 1 KiB blocks>.
CACHE_TMPFS=/var/cache/openafs
CACHE_BLOCKS=50000

cmd_fuse_prepare() {
  cmd_client_setup
  write_krb5_conf
  mkdir -p /afs "$CACHE_TMPFS"
  fuse_variant_args "${1:-etc-flags}" || die "unknown variant ${1:-}"
  fuse_cache_prepare
  command -v afsd.fuse || die "afsd.fuse is not installed"
}

# fuse_variant_args <variant>: sets FUSE_ARGS, CACHE_DIR and VICE (where
# /usr/vice/etc comes from: none, file = a copy of cacheinfo only, link = a
# symlink to /etc/openafs), and HYPOTHESIS (one line for the log). afsd's
# -debug trace is not used: its detail could carry internal names into the
# log, which is later read by a model (ADR 0001).
fuse_variant_args() {
  CACHE_DIR=$CACHE_TMPFS
  VICE="none"
  local -a flags=(-dynroot -fakestat)
  local -a valued=(-blocks "$CACHE_BLOCKS" -files 2000)
  case $1 in
    etc-flags)
      HYPOTHESIS="control: round 4's form, cacheinfo in /etc/openafs only (expect the ASSERT)"
      FUSE_ARGS=("${flags[@]}" -f /afs) ;;
    vice-file-flags)
      VICE="file"
      HYPOTHESIS="H1a: cacheinfo is read from /usr/vice/etc/cacheinfo"
      FUSE_ARGS=("${flags[@]}" -f /afs) ;;
    vice-link-flags)
      VICE="link"
      HYPOTHESIS="H1b: all client configuration is read from /usr/vice/etc"
      FUSE_ARGS=("${flags[@]}" -f /afs) ;;
    valued-first)
      HYPOTHESIS="H2a + H3a: valued switches first, then flags, -f and /afs; -blocks sets the size"
      FUSE_ARGS=("${valued[@]}" "${flags[@]}" -f /afs) ;;
    valued-first-blocks)
      HYPOTHESIS="H2a + H3a with one valued switch (-blocks only; files derived)"
      FUSE_ARGS=(-blocks "$CACHE_BLOCKS" "${flags[@]}" -f /afs) ;;
    mount-first)
      HYPOTHESIS="H2b: the mount point and -f first, afsd switches after"
      FUSE_ARGS=(/afs -f "${valued[@]}" "${flags[@]}") ;;
    dashdash)
      HYPOTHESIS="H2c: -- ends the afsd switches; the FUSE part after it"
      FUSE_ARGS=("${flags[@]}" "${valued[@]}" -- -f /afs) ;;
    no-mountpoint)
      HYPOTHESIS="H2d: valued switch last and no mount point argument (mount dir from cacheinfo?)"
      FUSE_ARGS=("${flags[@]}" "${valued[@]}" -f) ;;
    confdir-first)
      HYPOTHESIS="H1c (+ H2a order): -confdir /etc/openafs names the configuration directory"
      FUSE_ARGS=(-confdir /etc/openafs "${flags[@]}" -f /afs) ;;
    memcache-first)
      HYPOTHESIS="H3b (+ H2a order): a memory cache sized by -blocks, no disk cache"
      FUSE_ARGS=(-memcache -blocks 20000 "${flags[@]}" -f /afs) ;;
    vice-link-valued)
      VICE="link"
      HYPOTHESIS="H1b + H2a + H3a combined, with -cachedir"
      FUSE_ARGS=(-cachedir "$CACHE_TMPFS" "${valued[@]}" "${flags[@]}" -f /afs) ;;
    vice-link-single)
      VICE="link"
      HYPOTHESIS="H1b with FUSE's single-threaded form (-s)"
      FUSE_ARGS=("${flags[@]}" -f -s /afs) ;;
    *) return 1 ;;
  esac
}
FUSE_VARIANTS=(etc-flags vice-file-flags vice-link-flags valued-first valued-first-blocks
  mount-first dashdash no-mountpoint confdir-first memcache-first vice-link-valued
  vice-link-single)

# An empty cache directory, a cacheinfo naming it, and /usr/vice/etc as the
# variant asks, before each start.
fuse_cache_prepare() {
  mkdir -p "$CACHE_DIR"
  chmod 0700 "$CACHE_DIR"
  find "$CACHE_DIR" -mindepth 1 -delete 2>/dev/null
  printf '/afs:%s:%s\n' "$CACHE_DIR" "$CACHE_BLOCKS" >/etc/openafs/cacheinfo
  rm -rf /usr/vice/etc
  case $VICE in
    file)
      mkdir -p /usr/vice/etc
      cp /etc/openafs/cacheinfo /usr/vice/etc/cacheinfo ;;
    link)
      mkdir -p /usr/vice
      ln -s /etc/openafs /usr/vice/etc ;;
  esac
}

vice_view() {
  if [[ -L /usr/vice/etc ]]; then
    printf 'symlink to %s' "$(readlink /usr/vice/etc)"
  elif [[ -d /usr/vice/etc ]]; then
    printf 'directory: %s' "$(find /usr/vice/etc -mindepth 1 -maxdepth 1 -printf '%f ')"
  else
    printf 'absent'
  fi
}

# The cache home as the kernel sees it (type, size, free space).
cache_fs_view() {
  local d=$CACHE_TMPFS
  say "cache home $d: $(stat -f -c 'type %T, block %S, blocks %b, free %f' "$d" 2>&1); $(df -k "$d" 2>&1 | awk 'NR == 2 { print $2 " KiB, " $4 " KiB free" }')"
}

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
  fuse_variant_args "${1:-etc-flags}" || die "unknown variant ${1:-}"
  fuse_cache_prepare
  say "afsd.fuse ${FUSE_ARGS[*]} (cacheinfo: $(cat /etc/openafs/cacheinfo); /usr/vice/etc: $(vice_view))" >/var/log/afsd.fuse.log
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

# PARSE_BAD: output lines that mean the command line was refused.
PARSE_BAD='Too many values|[Uu]nrecognized|[Uu]nknown (switch|option)|[Aa]mbiguous|[Uu]sage:|missing (mount|argument)|[Ii]nvalid (argument|option)'

# probe_now: process alive, /afs a FUSE mount, /afs answers stat.
# Sets ALIVE, MOUNTED, ANSWERS.
probe_now() {
  afsd_alive && ALIVE=yes || ALIVE=no
  fuse_mounted && MOUNTED=yes || MOUNTED=no
  ANSWERS=no
  [[ $MOUNTED == yes ]] && timeout 5 stat /afs >/dev/null 2>&1 && ANSWERS=yes
}

# fuse-diag: each variant for up to 25 s in this container, probed at 6, 15
# and 20 s. Each prints its exact argv, the hypothesis, whether it parsed,
# alive/mounted/answering at each probe, the ASSERT line if any, its exit
# status and output, and one machine-read line:
#   variant-row: <name>|<argv>|<vice>|<parsed>|<6 s>|<15 s>|<20 s>|<assert>|<exit>
# where each probe is alive/mounted/answers as y or n. The first variant that
# answered at all three probes is printed as "fuse-variant-ok: <name>".
cmd_fuse_diag() {
  local v pid rc parsed assert lsout row chosen="" log=/tmp/fuse-diag.log t now
  local -a probes
  say "fuse.conf: $(grep -v '^#' /etc/fuse.conf 2>/dev/null | grep -v '^$' | tr '\n' ' ' || echo absent)"
  say "fusermount: $(command -v fusermount3 fusermount | tr '\n' ' ')"
  say "/dev/fuse: $(ls -l /dev/fuse 2>&1)"
  say "/proc/fs/openafs: $([[ -e /proc/fs/openafs ]] && echo present || echo absent)"
  say "uid: $(id -u); /etc/openafs: $(find /etc/openafs -maxdepth 1 -type f -printf '%f ' 2>&1)"
  cache_fs_view
  say "tmpfs mount: $(grep " $CACHE_TMPFS " /proc/mounts || echo 'none (not mounted)')"
  fuse_unmount
  for v in "${FUSE_VARIANTS[@]}"; do
    fuse_variant_args "$v"
    fuse_cache_prepare
    say "=== variant $v"
    say "hypothesis: $HYPOTHESIS"
    say "argv: afsd.fuse $(printf '%q ' "${FUSE_ARGS[@]}")"
    say "cacheinfo /etc/openafs: $(cat /etc/openafs/cacheinfo); /usr/vice/etc: $(vice_view); /usr/vice/etc/cacheinfo: $(cat /usr/vice/etc/cacheinfo 2>&1)"
    : >"$log"
    timeout 25 afsd.fuse "${FUSE_ARGS[@]}" >"$log" 2>&1 &
    pid=$!
    probes=()
    now=0
    for t in 6 15 20; do
      sleep $((t - now))
      now=$t
      probe_now
      probes+=("${ALIVE:0:1}/${MOUNTED:0:1}/${ANSWERS:0:1}")
      say "at $t s: alive=$ALIVE mounted=$MOUNTED answers=$ANSWERS"
      if [[ $t == 6 && $ANSWERS == yes ]]; then
        lsout=$(timeout 5 ls -la /afs 2>&1)
        say "ls /afs: $(head -n 4 <<<"$lsout" | tr '\n' ' ')"
        say "ls /afs/$CELL: $(timeout 10 ls "/afs/$CELL" 2>&1 | head -n 3 | tr '\n' ' ')"
        say "fs wscell: $(timeout 10 fs wscell 2>&1 | tr '\n' ' ')"
      fi
    done
    say "cache files: $(find "$CACHE_DIR" -type f 2>/dev/null | wc -l), $(du -sk "$CACHE_DIR" 2>/dev/null | cut -f1) KiB"
    wait "$pid"
    rc=$?
    say "exit status $rc$( ((rc == 124)) && echo ' (still running at 25 s, stopped by timeout)')$( ((rc > 128)) && echo " (signal $((rc - 128)))")"
    if grep -Eq "$PARSE_BAD" "$log"; then
      parsed="no: $(grep -Em1 "$PARSE_BAD" "$log")"
    else
      parsed=yes
    fi
    assert=$(grep -m1 'ASSERT' "$log")
    say "parsed: $parsed"
    say "assert: ${assert:-none}"
    say "--- output ($(wc -l <"$log") lines; head 40, tail 15) ---"
    if (($(wc -l <"$log") > 55)); then
      head -n 40 "$log"
      say "[...]"
      tail -n 15 "$log"
    else
      cat "$log"
    fi
    row="$v|afsd.fuse ${FUSE_ARGS[*]}|$VICE|${parsed//|//}|${probes[0]}|${probes[1]}|${probes[2]}|${assert//|//}|$rc"
    say "variant-row: ${row//$'\t'/ }"
    if [[ -z $chosen && ${probes[0]} == y/y/y && ${probes[1]} == y/y/y && ${probes[2]} == y/y/y ]]; then
      chosen=$v
    fi
    fuse_unmount
  done
  rm -rf /usr/vice/etc
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

# fs-reach: does fs reach afsd.fuse (its pioctl path)? fs wscell must name
# the synthetic cell.
cmd_fs_reach() {
  local out rc
  out=$(timeout 15 fs wscell 2>&1)
  rc=$?
  say "fs wscell: exit $rc: $out"
  ((rc == 0)) && grep -q "$CELL" <<<"$out"
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
  # afsd.fuse's page is the afsd page (round 3 printed it twice).
  for pat in 'afsd.8*' 'cacheinfo.5*'; do
    page=$(find_page "$pat")
    [[ -n $page ]] || { say "no page $pat"; continue; }
    say "--- ${page#"$DOCS"}: synopsis and the options this spike uses ---"
    man_text "$page" | sed -n '/^NAME$/,/^OPTIONS$/p' | head -n 30
    man_text "$page" | grep -n -A4 -E '^-(memcache|verbose|mountdir|cachedir|blocks|files|confdir|stat|dcache|chunksize)\b' | head -n 80
    say "--- the cache size and cacheinfo, as the page describes them ---"
    man_text "$page" | grep -n -i -B1 -A4 -E 'cacheinfo|cache size|percent|partition|disk cache' | head -n 80
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

# anon-probe: the one afsio call left (round 3: it dies with SIGSEGV in
# Heimdal's libkrb5.so.26 on every call). An anonymous read of a missing
# name in root.cell's root, before any seeding, with the Kerberos library it
# links. Always exits 0; the last line is "probe: exit N".
cmd_anon_probe() {
  local rc out
  assert_no_credentials
  say "afsio links: $(ldd "$(command -v afsio)" 2>&1 | grep -Ei 'krb5|gssapi|heimdal' | awk '{ print $1 " -> " $3 }' | tr '\n' ' ')"
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
  fuse-prepare) cmd_fuse_prepare ;;
  fuse-run) cmd_fuse_run "$@" ;;
  fuse-diag) cmd_fuse_diag ;;
  docs) cmd_docs ;;
  server-view) cmd_server_view ;;
  anon-probe) cmd_anon_probe ;;
  fuse-wait) cmd_fuse_wait ;;
  fs-reach) cmd_fs_reach ;;
  fuse-probe) cmd_fuse_probe ;;
  fuse-write) cmd_fuse_write "$@" ;;
  fuse-setacl) cmd_fuse_setacl ;;
  fuse-read-anon) cmd_fuse_read_anon "$@" ;;
  setauth) cmd_setauth "$@" ;;
  noauth-file) cmd_noauth_file ;;
  release) cmd_release ;;
  *) die "unknown subcommand '$sub'" ;;
esac
