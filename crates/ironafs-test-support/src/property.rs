// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Running a property over seeded inputs and reporting its counterexample.
//!
//! A property is a generator, which builds an input from a [`Source`], and
//! a test, which checks one input. The runner draws each case from a
//! generator seeded by the run's seed. On the first failure it shrinks the
//! input's choices and reports the simplest failing input with the seed, so
//! that `IRONAFS_TEST_SEED=<seed>` replays the same run (coding standards
//! rule 15: random inputs come from a seed printed on failure).
//!
//! The seed is fixed unless asked otherwise: without `IRONAFS_TEST_SEED` or
//! [`Property::seed`], it is derived from the property's name, so every run
//! of a test draws the same inputs. Tests stay deterministic and are never
//! retried (rule 15); a search with other seeds is a deliberate run with
//! the variable set.

use std::env;
use std::fmt;
use std::panic::{self, AssertUnwindSafe};

use crate::capture::catch_quietly;
use crate::shrink::shrink;
use crate::{Source, SplitMix64};

#[cfg(test)]
mod tests;

/// The environment variable that sets the seed of every property run.
const SEED_VARIABLE: &str = "IRONAFS_TEST_SEED";

/// A property test: how many cases to run, from which seed, and how hard
/// to shrink a failure.
///
/// ```
/// use ironafs_test_support::{Property, Source};
///
/// Property::new("u32 round-trips through big-endian bytes").check(
///     Source::u32,
///     |value| assert_eq!(u32::from_be_bytes(value.to_be_bytes()), *value),
/// );
/// ```
#[derive(Clone, Debug)]
pub struct Property {
    name: String,
    cases: u32,
    seed: Option<u64>,
    shrink_budget: u32,
}

impl Property {
    /// A property with 256 cases and a shrink budget of 10,000 runs, seeded
    /// from `name` unless `IRONAFS_TEST_SEED` is set.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            cases: 256,
            seed: None,
            shrink_budget: 10_000,
        }
    }

    /// Sets how many inputs are generated and tested.
    #[must_use]
    pub fn cases(self, cases: u32) -> Self {
        Self { cases, ..self }
    }

    /// Fixes the seed in code. `IRONAFS_TEST_SEED` still overrides it, so a
    /// printed seed always replays.
    #[must_use]
    pub fn seed(self, seed: u64) -> Self {
        Self {
            seed: Some(seed),
            ..self
        }
    }

    /// Sets the most times the test is run while shrinking a failure.
    #[must_use]
    pub fn shrink_budget(self, runs: u32) -> Self {
        Self {
            shrink_budget: runs,
            ..self
        }
    }

    /// Runs the property and returns the shrunk counterexample of the first
    /// failing case.
    ///
    /// `test` fails by panicking (with `assert!` and its relatives) or by
    /// returning `Err`. A panic in `generate` while drawing a case is a
    /// mistake in the test: it is propagated unchanged after the seed is
    /// written to standard error. While shrinking, a candidate on which
    /// `generate` panics is treated as passing.
    ///
    /// # Errors
    ///
    /// Returns the [`Counterexample`] if any case fails.
    ///
    /// # Panics
    ///
    /// Panics if `IRONAFS_TEST_SEED` is set but is not a decimal or
    /// `0x`-prefixed hexadecimal `u64`, or if `generate` panics.
    #[expect(
        clippy::panic,
        reason = "rule 1 covers untrusted input; a malformed seed variable is a mistake in the test run"
    )]
    pub fn run<T, V: Verdict>(
        &self,
        mut generate: impl FnMut(&mut Source) -> T,
        mut test: impl FnMut(&T) -> V,
    ) -> Result<(), Counterexample<T>> {
        let from_env = env::var(SEED_VARIABLE).ok();
        let seed = match resolve_seed(from_env.as_deref(), self.seed, &self.name) {
            Ok(seed) => seed,
            Err(message) => panic!("{message}"),
        };
        let mut case_seeds = SplitMix64::new(seed);
        for case in 0..self.cases {
            let mut source = Source::fresh(SplitMix64::new(case_seeds.next_u64()));
            let value = match panic::catch_unwind(AssertUnwindSafe(|| generate(&mut source))) {
                Ok(value) => value,
                Err(payload) => {
                    eprintln!(
                        "property `{}`: the generator panicked on case {} (seed {})",
                        self.name,
                        u64::from(case) + 1,
                        SeedText(seed),
                    );
                    panic::resume_unwind(payload);
                }
            };
            let Some(message) = test_failure(&mut test, &value) else {
                continue;
            };
            let shrunk = shrink(
                source.into_record(),
                (value, message),
                self.shrink_budget,
                |choices| {
                    let mut replay = Source::replay(choices.to_vec());
                    let value = catch_quietly(|| generate(&mut replay)).ok()?;
                    let message = test_failure(&mut test, &value)?;
                    Some((replay.into_record(), (value, message)))
                },
            );
            let (value, message) = shrunk.payload;
            return Err(Counterexample {
                name: self.name.clone(),
                seed,
                case,
                cases: self.cases,
                value,
                message,
                shrink_runs: shrunk.runs,
                shrink_budget_exhausted: shrunk.budget_exhausted,
            });
        }
        Ok(())
    }

    /// Runs the property and panics with the counterexample's report, which
    /// names the seed, if any case fails.
    ///
    /// # Panics
    ///
    /// Panics if a case fails, and as [`Property::run`] does.
    #[expect(
        clippy::panic,
        reason = "a failed property must fail the calling test (rule 15)"
    )]
    pub fn check<T: fmt::Debug, V: Verdict>(
        &self,
        generate: impl FnMut(&mut Source) -> T,
        test: impl FnMut(&T) -> V,
    ) {
        if let Err(counterexample) = self.run(generate, test) {
            panic!("{counterexample}");
        }
    }
}

/// What a property's test returns: `()` passes unless it panicked, and
/// `Result` fails with its error.
pub trait Verdict {
    /// The failure's description, or `None` if the test passed.
    fn failure(self) -> Option<String>;
}

impl Verdict for () {
    fn failure(self) -> Option<String> {
        None
    }
}

impl<E: fmt::Debug> Verdict for Result<(), E> {
    fn failure(self) -> Option<String> {
        self.err().map(|error| format!("{error:?}"))
    }
}

/// Runs `test` on `value` and returns its failure, if any.
fn test_failure<T, V: Verdict>(test: &mut impl FnMut(&T) -> V, value: &T) -> Option<String> {
    match catch_quietly(|| test(value)) {
        Ok(verdict) => verdict.failure(),
        Err(message) => Some(message),
    }
}

/// The simplest failing input found for a property, and how to replay it.
///
/// Its `Display` is the report [`Property::check`] fails with.
#[derive(Debug)]
pub struct Counterexample<T> {
    name: String,
    seed: u64,
    case: u32,
    cases: u32,
    value: T,
    message: String,
    shrink_runs: u32,
    shrink_budget_exhausted: bool,
}

impl<T> Counterexample<T> {
    /// The run's seed; `IRONAFS_TEST_SEED` set to it replays the run.
    #[must_use]
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// The zero-based index of the first failing case.
    #[must_use]
    pub fn case(&self) -> u32 {
        self.case
    }

    /// The shrunk input.
    #[must_use]
    pub fn value(&self) -> &T {
        &self.value
    }

    /// The test's failure on the shrunk input: its panic message, or its
    /// error's `Debug` text.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// How many times the test was run while shrinking.
    #[must_use]
    pub fn shrink_runs(&self) -> u32 {
        self.shrink_runs
    }

    /// Whether shrinking stopped at its budget, so a simpler failing input
    /// may exist.
    #[must_use]
    pub fn shrink_budget_exhausted(&self) -> bool {
        self.shrink_budget_exhausted
    }
}

impl<T: fmt::Debug> fmt::Display for Counterexample<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            formatter,
            "property `{}` failed on case {} of {} (seed {})",
            self.name,
            u64::from(self.case) + 1,
            self.cases,
            SeedText(self.seed),
        )?;
        writeln!(formatter, "minimal input: {:?}", self.value)?;
        writeln!(formatter, "failure: {}", self.message)?;
        write!(formatter, "shrunk in {} runs", self.shrink_runs)?;
        if self.shrink_budget_exhausted {
            write!(
                formatter,
                "; the shrink budget ran out, so the input may not be minimal"
            )?;
        }
        write!(
            formatter,
            "\nreplay with {SEED_VARIABLE}={}",
            SeedText(self.seed)
        )
    }
}

/// A seed as printed: fixed-width hexadecimal, which [`parse_seed`] reads.
struct SeedText(u64);

impl fmt::Display for SeedText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:#018x}", self.0)
    }
}

/// Reads a seed written in decimal or as `0x`-prefixed hexadecimal,
/// ignoring surrounding whitespace.
fn parse_seed(text: &str) -> Option<u64> {
    let text = text.trim();
    let (digits, radix) = match text.strip_prefix("0x").or_else(|| text.strip_prefix("0X")) {
        Some(hex) => (hex, 16),
        None => (text, 10),
    };
    // from_str_radix accepts a leading sign; a seed never has one.
    if digits.starts_with(['+', '-']) {
        return None;
    }
    u64::from_str_radix(digits, radix).ok()
}

/// A seed derived from the property's name, so that each property draws
/// its own inputs and every run draws the same ones.
fn seed_from_name(name: &str) -> u64 {
    let length = u64::try_from(name.len()).unwrap_or(u64::MAX);
    name.bytes()
        .fold(SplitMix64::new(length).next_u64(), |hash, byte| {
            SplitMix64::new(hash ^ u64::from(byte)).next_u64()
        })
}

/// Chooses the run's seed: the environment's, else the one fixed in code,
/// else the one derived from the name.
fn resolve_seed(from_env: Option<&str>, explicit: Option<u64>, name: &str) -> Result<u64, String> {
    match (from_env, explicit) {
        (Some(text), _) => parse_seed(text).ok_or_else(|| {
            format!("{SEED_VARIABLE} is not a decimal or 0x-prefixed hexadecimal u64: {text:?}")
        }),
        (None, Some(seed)) => Ok(seed),
        (None, None) => Ok(seed_from_name(name)),
    }
}
