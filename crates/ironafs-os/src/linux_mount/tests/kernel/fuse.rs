// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The two FUSE exchanges for the mount test. Layouts of the native-endian
//! headers, INIT and GETATTR are from `fuse(4)` and the Linux FUSE UAPI
//! (`<linux/fuse.h>`). This fixture negotiates protocol 7.9 and answers one
//! root GETATTR. The namespace launcher bounds blocking device I/O.

use std::fs::File;
use std::io::{Read, Write};

const IN_HEADER: usize = 40;
const OUT_HEADER: usize = 16;
const INIT: u32 = 26;
const GETATTR: u32 = 3;

/// Serves INIT and one root GETATTR on the mounted device.
pub(super) fn serve_getattr(mut device: &File, uid: u32, gid: u32) {
    // The UAPI requires at least 8 KiB. Leave room for a 64 KiB page
    // and the headers on platforms with larger pages.
    let mut request = vec![0_u8; 128 * 1024];
    let unique = read_request(&mut device, &mut request, INIT, 16);
    assert_eq!(u32::from_ne_bytes(request[40..44].try_into().unwrap()), 7);
    assert!(u32::from_ne_bytes(request[44..48].try_into().unwrap()) >= 9);
    // The INIT reply through 7.22 has 24 bytes, including max_write.
    let mut init = [0_u8; 24];
    init[0..4].copy_from_slice(&7_u32.to_ne_bytes());
    init[4..8].copy_from_slice(&9_u32.to_ne_bytes());
    init[20..24].copy_from_slice(&4096_u32.to_ne_bytes());
    reply(&mut device, unique, &init);

    let unique = read_request(&mut device, &mut request, GETATTR, 16);
    assert_eq!(u64::from_ne_bytes(request[16..24].try_into().unwrap()), 1);
    // Zero cache lifetime followed by the root directory's fuse_attr.
    let mut attr_out = [0_u8; 104];
    let attr = &mut attr_out[16..];
    attr[0..8].copy_from_slice(&1_u64.to_ne_bytes()); // ino
    attr[60..64].copy_from_slice(&0x41e9_u32.to_ne_bytes()); // mode
    attr[64..68].copy_from_slice(&2_u32.to_ne_bytes()); // nlink
    attr[68..72].copy_from_slice(&uid.to_ne_bytes());
    attr[72..76].copy_from_slice(&gid.to_ne_bytes());
    attr[80..84].copy_from_slice(&4096_u32.to_ne_bytes()); // blksize
    reply(&mut device, unique, &attr_out);
}

/// Reads one request in one device read, returning its reply identifier.
fn read_request(device: &mut &File, buffer: &mut [u8], opcode: u32, payload_min: usize) -> [u8; 8] {
    let count = device.read(buffer).unwrap();
    assert!(
        count >= IN_HEADER + payload_min,
        "short FUSE request: {count}"
    );
    assert_eq!(
        u32::from_ne_bytes(buffer[..4].try_into().unwrap()) as usize,
        count
    );
    assert_eq!(u32::from_ne_bytes(buffer[4..8].try_into().unwrap()), opcode);
    buffer[8..16].try_into().unwrap()
}

/// Writes one complete successful reply in one device write.
fn reply(device: &mut &File, unique: [u8; 8], payload: &[u8]) {
    let mut reply = Vec::with_capacity(OUT_HEADER + payload.len());
    reply.extend_from_slice(
        &u32::try_from(OUT_HEADER + payload.len())
            .unwrap()
            .to_ne_bytes(),
    );
    reply.extend_from_slice(&0_i32.to_ne_bytes());
    reply.extend_from_slice(&unique);
    reply.extend_from_slice(payload);
    assert_eq!(device.write(&reply).unwrap(), reply.len());
}
