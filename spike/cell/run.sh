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
# 4. For each mode (a: unprivileged; b: minimal added capabilities, first
#    passing set wins; c: --privileged), fresh containers on a fresh network:
#    KDC, db server, two fileservers, a client. Per-run keys are made inside
#    the KDC container and piped to the servers, never printed or stored on
#    the runner. Record: services up, volumes with RO replicas on both
#    fileservers, both fileservers answering from another container and from
#    the host's network namespace, seeding via afsio then afsd.fuse, an
#    anonymous read of the seeded file, and no AFS kernel module on the host.
# 5. Print a summary table (stdout and the job summary). Exit 0 only if the
#    brief's pass rule holds on this runner.

set -uo pipefail

here=$(cd "$(dirname "$0")" && pwd)
# shellcheck source=spike/cell/env.sh
. "$here/env.sh"

runner_label=${RUNNER_LABEL:-local}
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
  ca_mount=(-v /etc/ssl/certs/ca-certificates.crt:/etc/ssl/certs/ca-certificates.crt:ro)
try "builder start" docker run -d --init --name "$builder" -v "$here:/spike:ro" \
  "${ca_mount[@]}" "$image" sleep infinity || exit 1
show "install pinned packages" docker exec "$builder" bash /spike/incell.sh install "$arch" || exit 1
try "commit image" docker commit "$builder" "$image_local" || exit 1
docker rm -f "$builder" >/dev/null
install_secs=$((SECONDS - t0))
log "install took ${install_secs}s"
after_install=$(afs_modules)
log "AFS modules after install: ${after_install:-none}"
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

fuse_attempt() { # <mode> <variant> <flags...>
  local mode=$1 variant=$2 how
  shift 2
  for how in with-mountpoint from-cacheinfo; do
    docker rm -f "$prefix-$mode-fuse" >/dev/null 2>&1
    try "$mode: fuse container ($variant)" start_container "$mode" fuse "$IP_FUSE" "$@" || return 1
    show "$mode: fuse prepare" ex "$mode" fuse fuse-prepare || return 1
    docker exec -d "$prefix-$mode-fuse" bash /spike/incell.sh fuse-run "$how"
    show "$mode: fuse mount ($variant; mount point $how)" ex "$mode" fuse fuse-wait && return 0
  done
  return 1
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

  # Seeding route (i): afsio, then an anonymous read from the keyless client.
  if [[ $(get "$mode" volumes) == pass ]]; then
    if show "$mode: afsio seed" ex "$mode" db afsio-seed; then
      rec "$mode" seed_afsio pass
    else
      rec "$mode" seed_afsio fail
    fi
    if show "$mode: anonymous afsio read" ex "$mode" client afsio-read-anon; then
      rec "$mode" anon_read_afsio pass
    else
      rec "$mode" anon_read_afsio fail
    fi

    # Route (ii): afsd.fuse with /dev/fuse.
    local fuse=fail
    if fuse_attempt "$mode" "SYS_ADMIN, /dev/fuse" "${flags[@]}" --device /dev/fuse --cap-add SYS_ADMIN; then
      fuse="pass"
    elif fuse_attempt "$mode" "SYS_ADMIN, /dev/fuse, apparmor=unconfined" "${flags[@]}" \
      --device /dev/fuse --cap-add SYS_ADMIN --security-opt apparmor=unconfined; then
      fuse="pass (apparmor=unconfined)"
    fi
    rec "$mode" fuse_mount "$fuse"
    if [[ $fuse == pass* ]]; then
      if show "$mode: fs reaches afsd.fuse" ex "$mode" fuse fuse-fs-reach; then
        rec "$mode" fuse_fs_cmd pass
      else
        rec "$mode" fuse_fs_cmd fail
      fi
      if show "$mode: fuse seed (no token)" ex "$mode" fuse fuse-seed; then
        rec "$mode" seed_fuse pass
        try "$mode: release after fuse seed" \
          docker exec "$prefix-$mode-db" vos release root.cell -localauth
      else
        rec "$mode" seed_fuse fail
      fi
      if show "$mode: anonymous fuse read" ex "$mode" fuse fuse-read-anon; then
        rec "$mode" anon_read_fuse pass
      else
        rec "$mode" anon_read_fuse fail
      fi
    else
      rec "$mode" fuse_fs_cmd -
      rec "$mode" seed_fuse -
      rec "$mode" anon_read_fuse -
    fi
  else
    rec "$mode" seed_afsio -
    rec "$mode" fuse_mount -
    rec "$mode" fuse_fs_cmd -
    rec "$mode" seed_fuse -
    rec "$mode" anon_read_afsio -
    rec "$mode" anon_read_fuse -
  fi

  teardown_mode "$mode"
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
run_mode a
modes_run+=(a)
for m in b-sys-resource b-net-admin b-both; do
  run_mode "$m"
  modes_run+=("$m")
  if step4_passes "$m"; then
    log "first passing capability set: $m; the remaining b sets are skipped"
    break
  fi
done
run_mode c
modes_run+=(c)

# --- 5. Summary ------------------------------------------------------------

columns=(flags services volumes answer_container answer_host seed_afsio anon_read_afsio fuse_mount fuse_fs_cmd seed_fuse anon_read_fuse no_afs_module ready_secs total_secs)
summary="$work/summary.md"
{
  printf '### Cell spike (#47): %s\n\n' "$runner_label"
  printf -- '- kernel %s, %s; docker %s\n' "$(uname -r)" "$arch" \
    "$(docker version --format '{{.Server.Version}}' 2>/dev/null)"
  printf -- '- image debian@sha256:%s; OpenAFS %s; MIT Kerberos %s; snapshot %s\n' \
    "$platform_digest" "$OPENAFS_VERSION" "$KRB5_VERSION" "${SNAPSHOT_HTTPS##*/}"
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
} >"$summary"

verdict=fail least=""
seed_ok=0 lsmod_ok=1
for m in "${modes_run[@]}"; do
  if [[ -z $least ]] && step4_passes "$m"; then least=$m; fi
  if [[ $(get "$m" seed_afsio) == pass && $(get "$m" anon_read_afsio) == pass ]] ||
    [[ $(get "$m" fuse_mount) == pass* && $(get "$m" anon_read_fuse) == pass ]]; then
    seed_ok=1
  fi
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
