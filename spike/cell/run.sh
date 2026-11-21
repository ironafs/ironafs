#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Spike for #47 (throwaway branch, never merged): on one GitHub-hosted runner,
# which is the least-privileged way to run a disposable AFS cell in Docker,
# and how is file data seeded without an AFS kernel module?
#
# 1. Record the host: kernel, Docker, AppArmor and seccomp, /dev/fuse, and
#    the loaded modules.
# 2. Pull debian:trixie by this architecture's pinned digest, with an empty
#    Docker configuration (anonymous pull).
# 3. In a builder container, install the pinned packages from the dated
#    snapshot (apt checks the signed InRelease; incell.sh checks the recorded
#    .deb hashes and the installed versions), then commit a local image.
# 4. For each mode in SPIKE_MODES (a: unprivileged; b: minimal added
#    capabilities, first passing set wins; c: --privileged), fresh containers
#    on a fresh network: KDC, db server, two fileservers, a client. Per-run
#    keys are made inside the KDC container and piped to the servers (and the
#    admin keytab to the seeding containers), never printed or stored on the
#    runner. Record: services up, volumes with RO replicas on both
#    fileservers, both fileservers answering from another container and from
#    the host's network namespace, and no AFS kernel module on the host.
#    In the modes in SEED_MODES: the packages' documentation on afsd.fuse
#    and its cache, a 20 s foreground run of each afsd.fuse variant (cache
#    home and options) with its output and exit status, a server-side view,
#    one afsio diagnostic call (afsio is unusable in this build), then two
#    seeding routes through afsd.fuse (with a ticket; under bos setauth
#    off), each with fs setacl where fs reaches afsd.fuse and a release, and
#    anonymous reads from a fresh afsd.fuse holding no keytab, ticket or
#    token.
# 5. Print a summary table (stdout and the job summary). Exit 0 only if the
#    brief's pass rule holds on this runner.

set -uo pipefail

here=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=spike/cell/env.sh
. "$here/env.sh"

runner_label=${RUNNER_LABEL:-local}
# Rounds 3 and 4: mode a only (round 2's mode c control passed on all four
# runners).
read -r -a spike_modes <<<"${SPIKE_MODES:-a}"
read -r -a seed_modes <<<"${SEED_MODES:-a}"
# The tmpfs is afsd.fuse's cache home (round 3: the cache it sized was far
# below cacheinfo's; incell.sh's CACHE_TMPFS).
FUSE_FLAGS=(--device /dev/fuse --cap-add SYS_ADMIN --security-opt apparmor=unconfined
  --tmpfs "/var/cache/openafs:rw,size=512m,mode=0700")
FUSE_VARIANT="tmpfs" # replaced by the first variant the fuse-diag step finds serving
run_id="$(date -u +%Y%m%d%H%M%S)-$$"
prefix="spike47-$run_id"
work=$(mktemp -d)
chmod 0700 "$work"
mkdir -m 0700 "$work/docker-config" "$work/logs"
export DOCKER_CONFIG="$work/docker-config"
export REGISTRY_AUTH_FILE="$work/auth.json"
printf '{}\n' >"$REGISTRY_AUTH_FILE"
unset DOCKER_AUTH_CONFIG
results="$work/results.tsv"
: >"$results"
image_local="ironafs-spike-cell:$run_id"

log() { printf '[%s] %s\n' "$(date -u +%H:%M:%S)" "$*"; }
rec() {
  printf '%s\t%s\t%s\n' "$1" "$2" "$3" >>"$results"
  log "result: mode=$1 $2=$3"
}
get() { awk -F'\t' -v m="$1" -v k="$2" '$1 == m && $2 == k { v = $3 } END { print v }' "$results"; }

# try <name> <command...>: run, keep the output in a log, show its tail on
# failure. The commands given here print nothing secret (incell.sh keeps key
# steps' output inside the container).
try() {
  local name=$1 rc logf
  shift
  logf="$work/logs/$(tr -c 'A-Za-z0-9_.-' '_' <<<"$name").log"
  "$@" >"$logf" 2>&1
  rc=$?
  if ((rc == 0)); then
    log "ok: $name"
  else
    log "FAILED ($rc): $name"
    tail -n 25 "$logf" | sed 's/^/    | /'
  fi
  return "$rc"
}

# show <name> <command...>: like try, but always print the output (help
# texts, status listings; nothing secret).
show() {
  local rc
  try "$@"
  rc=$?
  ((rc == 0)) && sed 's/^/    | /' "$work/logs/$(tr -c 'A-Za-z0-9_.-' '_' <<<"$1").log"
  return "$rc"
}

afs_modules() { lsmod | awk 'NR > 1 { print $1 }' | grep -i afs || true; }

cleanup_all() {
  local -a ids
  mapfile -t ids < <(docker ps -aq --filter "name=^$prefix" 2>/dev/null)
  ((${#ids[@]})) && docker rm -f "${ids[@]}" >/dev/null 2>&1
  mapfile -t ids < <(docker network ls -q --filter "name=^$prefix" 2>/dev/null)
  ((${#ids[@]})) && docker network rm "${ids[@]}" >/dev/null 2>&1
  docker image rm -f "$image_local" >/dev/null 2>&1
  rm -rf "$work"
}

# --- 1. The host -----------------------------------------------------------

echo "::group::host"
log "runner: $runner_label"
log "kernel: $(uname -r), machine: $(uname -m)"
log "docker: $(docker version --format 'client {{.Client.Version}}, server {{.Server.Version}}' 2>&1)"
log "docker security options: $(docker info --format '{{json .SecurityOptions}}' 2>&1)"
log "apparmor enabled: $(cat /sys/module/apparmor/parameters/enabled 2>/dev/null || echo unknown)"
log "/dev/fuse: $(ls -l /dev/fuse 2>&1)"
baseline_modules=$(afs_modules)
log "AFS modules before: ${baseline_modules:-none}"
# Available, not loaded: would a host-side kernel client be a fallback?
log "modinfo kafs: $(modinfo -F filename kafs 2>&1 | head -n 1)"
log "modinfo rxrpc: $(modinfo -F filename rxrpc 2>&1 | head -n 1)"
log "modinfo openafs: $(modinfo -F filename openafs 2>&1 | head -n 1)"
echo "::endgroup::"

trap cleanup_all EXIT

case $(uname -m) in
  x86_64) arch=amd64 platform_digest=$IMAGE_AMD64 ;;
  aarch64) arch=arm64 platform_digest=$IMAGE_ARM64 ;;
  *) log "no pinned image for $(uname -m)"; exit 1 ;;
esac

# --- 2. The pinned image ---------------------------------------------------

echo "::group::pull and install"
image="docker.io/library/debian@sha256:$platform_digest"
t0=$SECONDS
try "docker pull" docker pull "$image" || exit 1
image_arch=$(docker image inspect --format '{{.Architecture}}' "$image")
[[ $image_arch == "$arch" ]] || { log "image architecture $image_arch, expected $arch"; exit 1; }
log "image: debian@sha256:$platform_digest ($arch; index sha256:$IMAGE_INDEX)"

# --- 3. Verified install ---------------------------------------------------

builder="$prefix-builder"
ca_mount=()
[[ -f /etc/ssl/certs/ca-certificates.crt ]] &&
  ca_mount=(-v "/etc/ssl/certs/ca-certificates.crt:$HOST_CA:ro")
try "builder start" docker run -d --init --name "$builder" -v "$here:/spike:ro" \
  "${ca_mount[@]}" "$image" sleep infinity || exit 1
show "install pinned packages" docker exec "$builder" bash /spike/incell.sh install "$arch" || exit 1
transport=$(sed -n 's/^transport: //p' "$work/logs/$(tr -c 'A-Za-z0-9_.-' '_' <<<"install pinned packages").log" | tail -n 1)
log "apt transport: ${transport:-unknown}"
try "commit image" docker commit "$builder" "$image_local" || exit 1
docker rm -f "$builder" >/dev/null
install_secs=$((SECONDS - t0))
log "install took ${install_secs}s"
after_install=$(afs_modules)
log "AFS modules after install: ${after_install:-none}"
echo "::endgroup::"

echo "::group::package documentation (afsd.fuse, ACLs)"
show "package docs" docker run --rm --network none -v "$here:/spike:ro" "$image_local" \
  bash /spike/incell.sh docs
echo "::endgroup::"

# --- 4. The modes ----------------------------------------------------------

mode_flags() {
  case $1 in
    a) ;;
    b-sys-resource) echo "--cap-add SYS_RESOURCE" ;;
    b-net-admin) echo "--cap-add NET_ADMIN" ;;
    b-both) echo "--cap-add SYS_RESOURCE --cap-add NET_ADMIN" ;;
    c) echo "--privileged" ;;
  esac
}

ex() { # ex <mode> <role> <incell args...>
  local mode=$1 role=$2
  shift 2
  docker exec "$prefix-$mode-$role" bash /spike/incell.sh "$@"
}

start_container() { # <mode> <role> <ip> <extra flags...>
  local mode=$1 role=$2 ip=$3
  shift 3
  docker run -d --init --name "$prefix-$mode-$role" --hostname "$role" \
    --network "$prefix-$mode" --ip "$ip" "$@" \
    -v "$here:/spike:ro" "$image_local" sleep infinity
}

# Pipe the cell key from the KDC container to a server container. It never
# touches the runner's disk or the log.
send_key() { # <mode> <role> <kvno>
  docker exec "$prefix-$1-kdc" bash /spike/incell.sh cat-keytab |
    docker exec -i "$prefix-$1-$2" bash /spike/incell.sh server-setup "$2" "$3"
}

teardown_mode() {
  local mode=$1
  local -a ids
  mapfile -t ids < <(docker ps -aq --filter "name=^$prefix-$mode-")
  ((${#ids[@]})) && docker rm -f "${ids[@]}" >/dev/null 2>&1
  docker network rm "$prefix-$mode" >/dev/null 2>&1
}

# Pipe the admin keytab from the KDC container to another container. It never
# touches the runner's disk or the log.
send_admin_keytab() { # <mode> <role>
  docker exec "$prefix-$1-kdc" bash /spike/incell.sh cat-admin-keytab |
    docker exec -i "$prefix-$1-$2" bash /spike/incell.sh recv-admin-keytab
}

# fuse_container <mode> <role> <ip>: start a container with an afsd.fuse
# mount at /afs, run as FUSE_VARIANT (foreground, in a detached exec, so
# its output and exit status stay in its log). Round 1 showed the default
# AppArmor profile refuses the mount on all four runners, so the fuse
# containers start unconfined. Ready means mounted, alive and answering.
fuse_container() {
  local mode=$1 role=$2 ip=$3
  docker rm -f "$prefix-$mode-$role" >/dev/null 2>&1
  try "$mode: $role container" start_container "$mode" "$role" "$ip" "${flags[@]}" "${FUSE_FLAGS[@]}" || return 1
  try "$mode: $role prepare" ex "$mode" "$role" fuse-prepare || return 1
  docker exec -d "$prefix-$mode-$role" bash /spike/incell.sh fuse-run "$FUSE_VARIANT"
  show "$mode: $role mount (variant $FUSE_VARIANT)" ex "$mode" "$role" fuse-wait
}

# fuse_diag <mode>: run every afsd.fuse variant for 20 s in a container of
# its own and keep the first that stays alive and serves /afs as
# FUSE_VARIANT.
fuse_diag() {
  local mode=$1 logf chosen
  if ! try "$mode: fuse-diag container" start_container "$mode" fuse-diag "$IP_FUSE_DIAG" "${flags[@]}" "${FUSE_FLAGS[@]}" ||
    ! try "$mode: fuse-diag prepare" ex "$mode" fuse-diag fuse-prepare; then
    rec "$mode" fuse_variant "fail (no container)"
    return
  fi
  logf="$work/logs/$mode-fuse-diag.log"
  log "$mode: afsd.fuse variants, 20 s each (output below)"
  ex "$mode" fuse-diag fuse-diag >"$logf" 2>&1
  sed 's/^/    | /' "$logf"
  chosen=$(sed -n 's/^fuse-variant-ok: //p' "$logf" | tail -n 1)
  if [[ -n $chosen && $chosen != none ]]; then
    FUSE_VARIANT=$chosen
    rec "$mode" fuse_variant "$chosen"
  else
    rec "$mode" fuse_variant "none served (routes use $FUSE_VARIANT)"
  fi
  docker rm -f "$prefix-$mode-fuse-diag" >/dev/null 2>&1
}

# host_kernel_log: kernel log lines about these programs (segfaults, FUSE);
# no secrets in them.
host_kernel_log() {
  log "host kernel log (afsd, afsio, segfault, fuse; last 30):"
  sudo -n dmesg 2>/dev/null | grep -Ei 'afsd|afsio|segfault|fuse' | tail -n 30 | sed 's/^/    | /'
}

# result <mode> <route> <field> <name> <command...>: run, record pass/fail.
result() {
  local mode=$1 route=$2 field=$3 name=$4
  shift 4
  if show "$mode: $route: $name" "$@"; then
    rec "$mode" "$route:$field" pass
    return 0
  fi
  rec "$mode" "$route:$field" fail
  return 1
}

release_after() { # <mode> <route>
  result "$1" "$2" release "vos release root.cell" ex "$1" db release
}

# fs_then_acl <mode> <route> <role>: fs setacl only where fs reaches
# afsd.fuse; otherwise record why not.
fs_then_acl() {
  local mode=$1 route=$2 role=$3
  if result "$mode" "$route" fs "fs reaches afsd.fuse (fs wscell)" ex "$mode" "$role" fs-reach; then
    result "$mode" "$route" acl "fs setacl system:anyuser rl" ex "$mode" "$role" fuse-setacl
  else
    rec "$mode" "$route:acl" "n/a (fs cannot reach afsd.fuse)"
  fi
}

seed_routes() {
  local mode=$1 ip state route anon_fuse=0 off=1 probe_log probe
  rec "$mode" seeding ran
  rec "$mode" fuse_flags "${FUSE_FLAGS[*]}"

  # Server side, before any seeding.
  show "$mode: server view of root.cell" ex "$mode" db server-view
  # The one afsio call kept (round 3: it segfaults in Heimdal's libkrb5 on
  # every call, so its write routes are dropped).
  show "$mode: afsio diagnostic (anonymous read of a missing name)" ex "$mode" client anon-probe
  probe_log="$work/logs/$(tr -c 'A-Za-z0-9_.-' '_' <<<"$mode: afsio diagnostic (anonymous read of a missing name)").log"
  probe=$(sed -n 's/^probe: //p' "$probe_log" | tail -n 1)
  rec "$mode" anon_probe "${probe:-no result}"

  fuse_diag "$mode"

  # Route fuse-token: afsd.fuse with a ticket and (if aklog reaches it) a
  # token; write, and set the ACL if fs reaches afsd.fuse.
  if fuse_container "$mode" fuse "$IP_FUSE"; then
    rec "$mode" fuse-token:mount pass
    show "$mode: admin keytab to the fuse container" send_admin_keytab "$mode" fuse
    show "$mode: fuse-token: probe the mount" ex "$mode" fuse fuse-probe
    if result "$mode" fuse-token krb kinit ex "$mode" fuse kinit-admin; then
      result "$mode" fuse-token token aklog ex "$mode" fuse aklog-token
    else
      rec "$mode" fuse-token:token -
    fi
    result "$mode" fuse-token write "write through the mount" ex "$mode" fuse fuse-write fuse-token
    fs_then_acl "$mode" fuse-token fuse
    if [[ $(get "$mode" fuse-token:write) == pass ]]; then
      release_after "$mode" fuse-token
    else
      rec "$mode" fuse-token:release -
    fi
  else
    rec "$mode" fuse-token:mount fail
    for state in krb token fs write acl release; do rec "$mode" "fuse-token:$state" -; done
  fi
  docker rm -f "$prefix-$mode-fuse" >/dev/null 2>&1

  # Route noauth-fuse: NoAuth on both fileservers (bos setauth off,
  # -localauth); an unauthenticated afsd.fuse writes through /afs/.<cell>/
  # and sets the ACL if fs reaches it; vos release; NoAuth off again.
  rec "$mode" noauth-fuse:krb "- (none held)"
  for ip in "$IP_FS1" "$IP_FS2"; do
    show "$mode: noauth-fuse: setauth $ip off" ex "$mode" db setauth "$ip" off || off=0
  done
  sleep 5
  for state in fs1 fs2; do
    log "$mode: NoAuth file on $state after setauth off: $(ex "$mode" "$state" noauth-file)"
  done
  if ((off)); then
    rec "$mode" noauth-fuse:token "n/a (bos setauth off)"
  else
    rec "$mode" noauth-fuse:token "fail (bos setauth off)"
  fi
  if ((off)) && fuse_container "$mode" fuse-noauth "$IP_FUSE_NOAUTH"; then
    rec "$mode" noauth-fuse:mount pass
    show "$mode: noauth-fuse: probe the mount" ex "$mode" fuse-noauth fuse-probe
    result "$mode" noauth-fuse write "write through the mount" ex "$mode" fuse-noauth fuse-write noauth-fuse
    fs_then_acl "$mode" noauth-fuse fuse-noauth
  else
    rec "$mode" noauth-fuse:mount "$( ((off)) && echo fail || echo -)"
    for state in fs write acl; do rec "$mode" "noauth-fuse:$state" -; done
  fi
  docker rm -f "$prefix-$mode-fuse-noauth" >/dev/null 2>&1
  # The release runs while NoAuth is still on (the route's order).
  if [[ $(get "$mode" noauth-fuse:write) == pass ]]; then
    release_after "$mode" noauth-fuse
  else
    rec "$mode" noauth-fuse:release -
  fi
  # Authorization back on everywhere before any anonymous read; if any
  # fileserver still holds NoAuth, the anonymous reads below prove nothing.
  NOAUTH_LEFT=0
  for ip in "$IP_FS1" "$IP_FS2"; do
    show "$mode: noauth-fuse: setauth $ip on" ex "$mode" db setauth "$ip" on || NOAUTH_LEFT=1
  done
  sleep 5
  for state in fs1 fs2; do
    if [[ $(ex "$mode" "$state" noauth-file) != no ]]; then
      log "$mode: NoAuth file still on $state"
      NOAUTH_LEFT=1
    fi
  done
  rec "$mode" noauth_cleared "$( ((NOAUTH_LEFT)) && echo FAIL || echo pass)"

  # Anonymous reads from a fresh afsd.fuse started after every release, in
  # a container that never held a keytab, a ticket or a token. Tried even
  # where fs could not set the ACL: then the read shows whether a new
  # volume's default ACL lets system:anyuser read.
  if ((NOAUTH_LEFT)); then
    for route in "${SEED_ROUTES[@]}"; do rec "$mode" "$route:anon_fuse" "void (NoAuth left on)"; done
    return
  fi
  fuse_container "$mode" fuse-anon "$IP_FUSE_ANON" && anon_fuse=1
  ((anon_fuse)) && show "$mode: anonymous probe of the mount (lists, default ACL)" ex "$mode" fuse-anon fuse-probe
  for route in "${SEED_ROUTES[@]}"; do
    if [[ $(get "$mode" "$route:write") != pass ]]; then
      rec "$mode" "$route:anon_fuse" -
    elif ((anon_fuse)); then
      result "$mode" "$route" anon_fuse "anonymous fuse read" ex "$mode" fuse-anon fuse-read-anon "$route"
    else
      rec "$mode" "$route:anon_fuse" "fail (no mount)"
    fi
  done
}

run_mode() {
  local mode=$1 start=$SECONDS kvno role ip ok flags_text
  local -a flags
  flags_text=$(mode_flags "$mode")
  read -r -a flags <<<"$flags_text"
  echo "::group::mode $mode (${flags_text:-no added flags})"
  rec "$mode" flags "${flags_text:-none}"

  if ! try "$mode: network" docker network create --subnet "$SUBNET" "$prefix-$mode"; then
    rec "$mode" services fail
    echo "::endgroup::"
    return
  fi
  ok=1
  for role in kdc:$IP_KDC db:$IP_DB fs1:$IP_FS1 fs2:$IP_FS2 client:$IP_CLIENT; do
    try "$mode: start ${role%%:*}" start_container "$mode" "${role%%:*}" "${role#*:}" "${flags[@]}" || ok=0
  done

  # Services: KDC answers, keys installed, bosserver and every instance up.
  ((ok)) && { show "$mode: kdc init (per-run keys)" ex "$mode" kdc kdc-init || ok=0; }
  if ((ok)); then
    kvno=$(ex "$mode" kdc kvno)
    [[ $kvno =~ ^[0-9]+$ ]] || { log "$mode: no key version"; ok=0; }
  fi
  if ((ok)); then
    for role in db fs1 fs2; do
      show "$mode: key and config on $role" send_key "$mode" "$role" "$kvno" || ok=0
    done
    try "$mode: client config" ex "$mode" client client-setup || ok=0
  fi
  if ((ok)); then
    for role in db fs1 fs2; do
      docker exec -d "$prefix-$mode-$role" bash /spike/incell.sh bosserver || ok=0
    done
    show "$mode: db instances" ex "$mode" db db-instances || ok=0
    show "$mode: wait db instances" ex "$mode" db wait-running "$IP_DB" 180 ptserver vlserver || ok=0
    show "$mode: pts users" ex "$mode" db pts-setup || ok=0
  fi
  if ((ok)); then
    for ip in "$IP_FS1" "$IP_FS2"; do
      show "$mode: fs instance on $ip" ex "$mode" db fs-instances "$ip" || ok=0
    done
    for ip in "$IP_FS1" "$IP_FS2"; do
      show "$mode: wait fs on $ip" ex "$mode" db wait-running "$ip" 180 fs || ok=0
    done
  fi
  if ((ok)); then
    rec "$mode" services pass
    rec "$mode" ready_secs $((SECONDS - start))
  else
    rec "$mode" services fail
  fi

  # Volumes with read-only replicas on both fileservers.
  if ((ok)) && show "$mode: volumes and releases" ex "$mode" db volumes; then
    rec "$mode" volumes pass
  else
    if ((ok)); then rec "$mode" volumes fail; else rec "$mode" volumes -; fi
  fi

  # Both fileservers answer, from another container and from the host.
  if ((ok)); then
    local c=pass h=pass
    for ip in "$IP_FS1" "$IP_FS2"; do
      show "$mode: rxdebug $ip from client container" ex "$mode" client rxdebug "$ip" || c=fail
      show "$mode: rxdebug $ip from host network" docker run --rm --network host \
        -v "$here:/spike:ro" "$image_local" bash /spike/incell.sh rxdebug "$ip" || h=fail
    done
    rec "$mode" answer_container "$c"
    rec "$mode" answer_host "$h"
  else
    rec "$mode" answer_container -
    rec "$mode" answer_host -
  fi

  # Seeding: only in SEED_MODES, and only once volumes pass.
  if [[ " ${seed_modes[*]} " == *" $mode "* ]]; then
    if [[ $(get "$mode" volumes) == pass ]]; then
      seed_routes "$mode"
    else
      rec "$mode" seeding "- (volumes did not pass)"
    fi
  else
    rec "$mode" seeding "- (not in SEED_MODES)"
  fi

  teardown_mode "$mode"
  host_kernel_log
  local mods
  mods=$(afs_modules)
  if [[ -z $mods ]]; then rec "$mode" no_afs_module pass; else rec "$mode" no_afs_module "FAIL: $mods"; fi
  rec "$mode" total_secs $((SECONDS - start))
  echo "::endgroup::"
}

step4_passes() { # <mode>
  [[ $(get "$1" services) == pass && $(get "$1" volumes) == pass &&
    $(get "$1" answer_container) == pass && $(get "$1" answer_host) == pass ]]
}

modes_run=()
for m in "${spike_modes[@]}"; do
  if [[ $m == b ]]; then
    for mb in b-sys-resource b-net-admin b-both; do
      run_mode "$mb"
      modes_run+=("$mb")
      if step4_passes "$mb"; then
        log "first passing capability set: $mb; the remaining b sets are skipped"
        break
      fi
    done
  else
    run_mode "$m"
    modes_run+=("$m")
  fi
done

# --- 5. Summary ------------------------------------------------------------

columns=(flags services volumes answer_container answer_host seeding no_afs_module ready_secs total_secs)
route_fields=(mount krb token fs write acl release anon_fuse)
summary="$work/summary.md"
{
  printf '### Cell spike (#47): %s\n\n' "$runner_label"
  printf -- '- kernel %s, %s; docker %s\n' "$(uname -r)" "$arch" \
    "$(docker version --format '{{.Server.Version}}' 2>/dev/null)"
  printf -- '- image debian@sha256:%s; OpenAFS %s; MIT Kerberos %s; snapshot %s; apt transport: %s\n' \
    "$platform_digest" "$OPENAFS_VERSION" "$KRB5_VERSION" "${SNAPSHOT_HTTPS##*/}" "${transport:-unknown}"
  printf -- '- install %ss; AFS modules before: %s, after install: %s\n\n' \
    "$install_secs" "${baseline_modules:-none}" "${after_install:-none}"
  printf '| mode |'
  printf ' %s |' "${columns[@]}"
  printf '\n|---|'
  printf -- '---|%.0s' "${columns[@]}"
  printf '\n'
  for m in "${modes_run[@]}"; do
    printf '| %s |' "$m"
    for c in "${columns[@]}"; do printf ' %s |' "$(get "$m" "$c")"; done
    printf '\n'
  done
  for m in "${modes_run[@]}"; do
    [[ $(get "$m" seeding) == ran ]] || continue
    printf '\n#### Seeding routes, mode %s (fuse containers: %s; NoAuth cleared before the reads: %s)\n\n' \
      "$m" "$(get "$m" fuse_flags)" "$(get "$m" noauth_cleared)"
    printf -- '- afsd.fuse variant alive and serving /afs (20 s foreground runs): %s\n' "$(get "$m" fuse_variant)"
    printf -- '- the one afsio call (anonymous read of a missing name, before seeding): %s\n\n' "$(get "$m" anon_probe)"
    printf '| route |'
    printf ' %s |' "${route_fields[@]}"
    printf '\n|---|'
    printf -- '---|%.0s' "${route_fields[@]}"
    printf '\n'
    for r in "${SEED_ROUTES[@]}"; do
      printf '| %s |' "$r"
      for c in "${route_fields[@]}"; do printf ' %s |' "$(get "$m" "$r:$c")"; done
      printf '\n'
    done
  done
  printf '\nfs: fs wscell reaches afsd.fuse. Where acl is n/a, the anonymous read depends on the ACL a new volume gets.\n'
} >"$summary"

verdict=fail least=""
seed_ok=0 lsmod_ok=1
for m in "${modes_run[@]}"; do
  if [[ -z $least ]] && step4_passes "$m"; then least=$m; fi
  for r in "${SEED_ROUTES[@]}"; do
    if [[ $(get "$m" "$r:write") == pass && $(get "$m" "$r:release") == pass ]] &&
      [[ $(get "$m" "$r:anon_fuse") == pass ]]; then
      seed_ok=1
    fi
  done
  [[ $(get "$m" no_afs_module) == pass ]] || lsmod_ok=0
done
[[ -n $least && $seed_ok == 1 && $lsmod_ok == 1 ]] && verdict=pass
{
  printf '\nLeast-privileged mode passing step 4: %s. A seeding route with an anonymous read: %s. No AFS module: %s.\n' \
    "${least:-none}" "$( ((seed_ok)) && echo yes || echo no)" "$( ((lsmod_ok)) && echo yes || echo no)"
  printf '**Verdict on this runner: %s**\n' "$verdict"
} >>"$summary"

echo
cat "$summary"
[[ -n ${GITHUB_STEP_SUMMARY:-} ]] && cat "$summary" >>"$GITHUB_STEP_SUMMARY"
[[ $verdict == pass ]]
