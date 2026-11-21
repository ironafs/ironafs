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

find_in_package() { # find_in_package <package> <basename>
  dpkg -L "$1" 2>/dev/null | grep -E "/$2\$" | head -n 1
}

cmd_install() {
  local arch=$1 key base ok=0 line name pin file got pkg want have
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

  for base in "$SNAPSHOT_HTTPS" "$SNAPSHOT_HTTP"; do
    cat >/etc/apt/sources.list.d/cell.sources <<EOF
Types: deb
URIs: $base
Suites: $SNAPSHOT_SUITE
Components: $SNAPSHOT_COMPONENT
Signed-By: $key
EOF
    rm -rf /var/lib/apt/lists/*
    if apt-get -o Acquire::Retries=5 update >/tmp/apt-update.log 2>&1 &&
      ! grep -Eq "$APT_BAD" /tmp/apt-update.log; then
      ok=1
      say "apt-get update: signed InRelease accepted from $base"
      break
    fi
    tail -n 20 /tmp/apt-update.log
    if grep -Eq "$APT_SIG_BAD" /tmp/apt-update.log; then
      die "apt-get update: signature problem (never retried over another transport)"
    fi
    say "apt-get update: failed over $base"
  done
  ((ok)) || die "apt-get update failed over every transport"

  apt-get -o Acquire::Retries=5 install -y --no-install-recommends --download-only \
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

  apt-get install -y --no-install-recommends "${PACKAGES[@]}" >/tmp/apt-install.log 2>&1 ||
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

# Seeding route (i): afsio. Its help text is printed for the record.
cmd_afsio_seed() {
  local auth=() back path
  command -v afsio >/dev/null || die "afsio is not installed"
  afsio help 2>&1 | head -n 40
  afsio write -help 2>&1 | head -n 40
  if afsio write -help 2>&1 | grep -q -- '-localauth'; then auth=(-localauth); fi
  say "afsio auth flags: ${auth[*]:-(none)}"
  # The plain path first; the dotted read-write path if that is refused.
  for path in "/afs/$CELL/hello.txt" "/afs/.$CELL/hello.txt"; do
    if printf '%s\n' "$SEED_TEXT" | timeout 120 afsio write -file "$path" "${auth[@]}"; then
      back=$(timeout 60 afsio read -file "$path" "${auth[@]}") || return 1
      [[ $back == "$SEED_TEXT" ]] || die "afsio read-back differs"
      vos release root.cell -localauth || return 1
      say "afsio: wrote $path, read it back and released root.cell"
      return 0
    fi
    say "afsio: write to $path failed"
  done
  return 1
}

# Anonymous read through afsio, in a container that holds no key.
cmd_afsio_read_anon() {
  local back
  back=$(timeout 60 afsio read -file "/afs/$CELL/hello.txt") || return 1
  [[ $back == "$SEED_TEXT" ]] || die "anonymous afsio read returned other bytes"
  say "anonymous afsio read: ${#back} bytes, as written"
}

# Route (ii): afsd.fuse, a user-space cache manager over /dev/fuse.
cmd_fuse_prepare() {
  cmd_client_setup
  mkdir -p /afs /var/cache/openafs
  printf '/afs:/var/cache/openafs:50000\n' >/etc/openafs/cacheinfo
  command -v afsd.fuse || die "afsd.fuse is not installed"
}

cmd_fuse_run() { # [with-mountpoint]
  if [[ ${1:-} == with-mountpoint ]]; then
    exec afsd.fuse -dynroot -fakestat /afs >/var/log/afsd.fuse.log 2>&1
  fi
  exec afsd.fuse -dynroot -fakestat >/var/log/afsd.fuse.log 2>&1
}

fuse_mounted() { grep -Eq '^[^ ]+ /afs fuse' /proc/mounts; }

cmd_fuse_wait() {
  if poll 60 fuse_mounted; then
    grep -E ' /afs fuse' /proc/mounts
    return 0
  fi
  tail -n 20 /var/log/afsd.fuse.log 2>/dev/null
  return 1
}

cmd_fuse_read_anon() {
  local back
  ls -la "/afs/$CELL" || return 1
  back=$(timeout 60 cat "/afs/$CELL/hello.txt") || return 1
  [[ $back == "$SEED_TEXT" ]] || die "anonymous fuse read returned other bytes"
  say "anonymous fuse read: ${#back} bytes, as written"
}

# Does the fs command reach afsd.fuse at all (a harmless query)?
cmd_fuse_fs_reach() { timeout 30 fs whereis "/afs/$CELL"; }

# Seeding through the fuse mount: write a file and grant anonymous read. No
# token is held here, so this records what an unauthenticated mount allows.
cmd_fuse_seed() {
  # The dotted name is the read-write path under -dynroot.
  printf '%s\n' "$SEED_TEXT" >"/afs/.$CELL/hello.txt" || return 1
  timeout 30 fs setacl -dir "/afs/.$CELL" -acl system:anyuser rl || return 1
  say "fuse: wrote a file and set an ACL"
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
  afsio-seed) cmd_afsio_seed ;;
  afsio-read-anon) cmd_afsio_read_anon ;;
  fuse-prepare) cmd_fuse_prepare ;;
  fuse-run) cmd_fuse_run "$@" ;;
  fuse-wait) cmd_fuse_wait ;;
  fuse-read-anon) cmd_fuse_read_anon ;;
  fuse-fs-reach) cmd_fuse_fs_reach ;;
  fuse-seed) cmd_fuse_seed ;;
  *) die "unknown subcommand '$sub'" ;;
esac
