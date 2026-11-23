// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the reading of the pinned toolchain.

use std::ffi::OsStr;
use std::path::PathBuf;

use super::{Pinned, channel, mismatch, pin_cargo, release};
use crate::Command;

#[test]
fn channel_reads_the_toolchain_file_s_pin() {
    let text = "# A comment.\n[toolchain]\nchannel = \"1.100.0\" # exact\ncomponents = []\n";
    assert_eq!(channel(text), Some("1.100.0"));
    assert_eq!(channel("[toolchain]\ncomponents = []\n"), None);
}

#[test]
fn release_reads_the_verbose_version() {
    let rustc = "rustc 1.100.0 (0ef1bc386 2026-11-09)\nbinary: rustc\nhost: aarch64-apple-darwin\n\
                 release: 1.100.0\nLLVM version: 21.1.0\n";
    assert_eq!(release(rustc), Some("1.100.0"));
    assert_eq!(
        release("cargo 1.100.0 (3acc79c0c 2026-11-05)\nrelease: 1.100.0\n"),
        Some("1.100.0")
    );
    assert_eq!(release("rustc 1.99.0\n"), None);
}

#[test]
fn mismatch_names_a_tool_that_is_not_the_pin() {
    assert_eq!(mismatch("rustc", Some("1.100.0"), "1.100.0"), None);
    let found = mismatch("cargo", Some("1.101.0-nightly"), "1.100.0").unwrap();
    assert!(
        found.contains("cargo") && found.contains("1.101.0-nightly") && found.contains("1.100.0")
    );
    assert!(mismatch("rustc", None, "1.100.0").is_some());
}

#[test]
fn pin_cargo_names_the_pinned_compiler_and_no_wrapper() {
    let pinned = Pinned {
        rustc: PathBuf::from("/pinned/bin/rustc"),
    };
    let mut command = Command::new("cargo");
    command.env("RUSTFLAGS", "-Cplanted");
    pin_cargo(&mut command, &pinned);
    let envs: Vec<(&OsStr, Option<&OsStr>)> = command.get_envs().collect();
    let value = |name: &str| {
        envs.iter()
            .find(|(key, _)| *key == OsStr::new(name))
            .map(|(_, value)| *value)
    };
    assert_eq!(value("RUSTC"), Some(Some(OsStr::new("/pinned/bin/rustc"))));
    assert_eq!(value("RUSTC_WRAPPER"), Some(Some(OsStr::new(""))));
    assert_eq!(value("RUSTC_WORKSPACE_WRAPPER"), Some(Some(OsStr::new(""))));
    assert_eq!(value("RUSTFLAGS"), Some(None));
}

#[test]
fn has_target_looks_for_the_target_s_library_in_the_pinned_sysroot() {
    let sysroot = std::env::temp_dir().join(format!("ironafs-xtask-pin-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&sysroot);
    std::fs::create_dir_all(sysroot.join("bin")).unwrap();
    std::fs::create_dir_all(sysroot.join("lib/rustlib/x86_64-unknown-linux-musl/lib")).unwrap();
    let pinned = Pinned {
        rustc: sysroot.join("bin/rustc"),
    };
    assert!(pinned.has_target("x86_64-unknown-linux-musl"));
    assert!(!pinned.has_target("aarch64-unknown-linux-musl"));
    std::fs::remove_dir_all(&sysroot).unwrap();
}
