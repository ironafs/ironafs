# SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
# SPDX-License-Identifier: GPL-3.0-or-later
#
# Shared values for the #47 cell spike (throwaway; never merged). Sourced by
# run.sh on the runner and by incell.sh inside the containers. Every name is
# synthetic (RFC 2606 .invalid); no real cell's names or keys.
# shellcheck shell=bash disable=SC2034

CELL=test.ironafs.invalid
REALM=TEST.IRONAFS.INVALID

SUBNET=172.30.47.0/24
IP_KDC=172.30.47.10
IP_DB=172.30.47.11
IP_FS1=172.30.47.21
IP_FS2=172.30.47.22
IP_CLIENT=172.30.47.30
IP_FUSE=172.30.47.31

# debian:trixie, pinned by digest (plan R4, read 2026-11-21).
IMAGE_INDEX=9c6e381421d0f48d1b8133e573f33c185ee16cbf505eda9c4a8a4791b25ea8a6
IMAGE_AMD64=bfc36aecdece0387418759dba474aa856c026ca8087da3606b41b90ab0509c99
IMAGE_ARM64=e529717ccc8118a0b9d38525f2b4e78cf150dbe470ad60d733c7c0023cabc3b0

# The dated snapshot (plan R3). The base image has no CA bundle, so the runner
# mounts its own for HTTPS; plain HTTP is the fallback for transport failures
# only. Either way apt checks the signed InRelease and the package hashes.
SNAPSHOT_HTTPS=https://snapshot.debian.org/archive/debian/20261120T023438Z
SNAPSHOT_HTTP=http://snapshot.debian.org/archive/debian/20261120T023438Z
SNAPSHOT_SUITE=trixie
SNAPSHOT_COMPONENT=main

OPENAFS_VERSION=1.8.13.2-1
KRB5_VERSION=1.21.3-5+deb13u1

PACKAGES=(
  "openafs-fileserver=$OPENAFS_VERSION"
  "openafs-dbserver=$OPENAFS_VERSION"
  "openafs-client=$OPENAFS_VERSION"
  "openafs-krb5=$OPENAFS_VERSION"
  "openafs-fuse=$OPENAFS_VERSION"
  "krb5-kdc=$KRB5_VERSION"
  "krb5-admin-server=$KRB5_VERSION"
  "krb5-user=$KRB5_VERSION"
)

# SHA-256 of the .deb files, from the snapshot's signed Packages (plan R3).
# A value with "..." is the prefix and suffix the plan recorded; the rest are
# checked only through apt's signed metadata.
HASH_PINS='
amd64 openafs-fileserver 990c2ac0d1657bfc5e0e8f182bae5b78dae295222d7a20a5b4b12315097cceaa
amd64 krb5-kdc 5533f05f8d20cee3e5441b5934f131a3b5b0487c7c1818680f175cbe8c7ea7c1
amd64 openafs-dbserver 71e52905...061abc
amd64 openafs-client 214b7849...64246e0bc
amd64 openafs-krb5 d50f8305...c478e972
amd64 krb5-admin-server 40a6668b...77ffd1d6
arm64 openafs-fileserver 9f018f74a63bbf27b803e7915d652e7f737c144fe5c2cdc2903a72dfbc4a0799
arm64 krb5-kdc 43329914d5743bb4a9ebb7e427a05e813c4515a59d18feaa7299e14aa10f2b2c
'

# The synthetic file the seeding routes write and the anonymous read checks.
SEED_TEXT='ironafs test cell'
