// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Planted calls to every item coding standards rule 7 bans, one per line,
//! written by `cargo xtask check-bans`, which lints them with the workspace
//! and refuses any other text. Built only with the `planted` feature.

#![cfg(feature = "planted")]

#[rustfmt::skip] #[must_use] pub fn instant_call() -> std::time::Instant { std::time::Instant::now() }
#[rustfmt::skip] #[must_use] pub fn instant_path() -> fn() -> std::time::Instant { std::time::Instant::now }
#[rustfmt::skip] #[must_use] pub fn instant_method(at: std::time::Instant) -> core::time::Duration { at.elapsed() }
#[rustfmt::skip] #[must_use] pub fn system_call() -> std::time::SystemTime { std::time::SystemTime::now() }
#[rustfmt::skip] #[must_use] pub fn system_method(at: std::time::SystemTime) -> bool { at.elapsed().is_ok() }
#[rustfmt::skip] #[must_use] pub fn random_state() -> u64 { use std::hash::BuildHasher; std::hash::RandomState::new().hash_one(1_u8) }
#[rustfmt::skip] #[expect(clippy::default_trait_access)] #[must_use] pub fn random_default() -> std::hash::RandomState { Default::default() }
#[rustfmt::skip] #[must_use] pub fn random_default_call() -> u64 { use std::hash::BuildHasher; std::hash::RandomState::default().hash_one(1_u8) }
#[rustfmt::skip] #[must_use] pub fn hash_map_new() -> bool { let mut map = std::collections::HashMap::new(); map.insert(1_u8, 2_u8); map.is_empty() }
#[rustfmt::skip] #[must_use] pub fn hash_map() -> usize { std::collections::HashMap::<u8, u8>::default().len() }
#[rustfmt::skip] #[must_use] pub fn hash_map_collect() -> usize { [(1_u8, 2_u8)].into_iter().collect::<std::collections::HashMap<_, _>>().len() }
#[rustfmt::skip] #[must_use] pub fn hash_set() -> usize { std::collections::HashSet::<u8>::new().len() }
#[rustfmt::skip] #[must_use] pub fn deadline_boot(now: &ironafs_time::Stamp) -> bool { ironafs_time::Deadline::boot_after(now, now, core::time::Duration::ZERO).is_ok() }
#[rustfmt::skip] #[must_use] pub fn deadline_awake() -> fn(&ironafs_time::Stamp, &ironafs_time::Stamp, core::time::Duration) -> Result<ironafs_time::Deadline, ironafs_time::DeadlineError> { ironafs_time::Deadline::awake_after }
#[rustfmt::skip] #[must_use] pub fn subprocess() -> bool { std::process::Command::new("date").status().is_ok() }
