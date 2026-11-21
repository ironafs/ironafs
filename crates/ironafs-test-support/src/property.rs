// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Running a property over seeded inputs and reporting its counterexample.
//!
//! A property is a generator, which builds an input from a [`Source`], and
//! a test, which checks one input. The runner draws each case from a
//! generator seeded by the run's seed. On the first failure it writes the
//! seed to standard error at once, then shrinks the input's choices within
//! a run budget and a time limit, and reports the simplest failing input
//! with the seed, so that `IRONAFS_TEST_SEED=<seed>` replays the same run
//! (coding standards rule 15: random inputs come from a seed printed on
//! failure). A test killed by the runner while shrinking has still printed
//! its seed. The report also prints a regression test that replays the
//! minimal input, to paste into the crate's `tests/it/regressions.rs`
//! ([`Property::regression`]).
//!
//! The seed is fixed unless asked otherwise: without `IRONAFS_TEST_SEED` or
//! [`Property::seed`], it is derived from the property's name, so every run
//! of a test draws the same inputs. Tests stay deterministic and are never
//! retried (rule 15); a search with other seeds is a deliberate run with
//! the variable set. An empty variable counts as unset.

use std::env::{self, VarError};
use std::fmt;
use std::panic::{self, AssertUnwindSafe};
use std::time::{Duration, Instant};

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
    shrink_time_limit: Duration,
}

impl Property {
    /// A property with 256 cases, a shrink budget of 10,000 runs and a
    /// shrink time limit of 20 seconds, seeded from `name` unless
    /// `IRONAFS_TEST_SEED` is set.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            cases: 256,
            seed: None,
            shrink_budget: 10_000,
            shrink_time_limit: Duration::from_secs(20),
        }
    }

    /// Sets how many inputs are generated and tested.
    ///
    /// # Panics
    ///
    /// Panics if `cases` is zero: a property that runs no case would pass
    /// without testing anything (rule 15).
    #[must_use]
    pub fn cases(self, cases: u32) -> Self {
        assert!(cases > 0, "a property needs at least one case");
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

    /// Sets the longest time spent shrinking a failure, measured on the
    /// wall clock. When it runs out, the simplest failing input found so
    /// far is reported, and the report says that it may not be minimal.
    ///
    /// The default of 20 seconds leaves room, under the test runner's
    /// 60-second limit per test (`.config/nextest.toml`), for the failing
    /// case itself and the report. The seed is printed before shrinking
    /// starts, so a test stopped by the runner still names it.
    #[must_use]
    pub fn shrink_time_limit(self, limit: Duration) -> Self {
        Self {
            shrink_time_limit: limit,
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
    /// Panics are caught on the calling thread only. A panic in a thread
    /// that `test` spawns is printed as usual, and fails the case only if
    /// `test` joins that thread and propagates the panic.
    ///
    /// # Errors
    ///
    /// Returns the [`Counterexample`] if any case fails.
    ///
    /// # Panics
    ///
    /// Panics if `IRONAFS_TEST_SEED` is set but is not valid Unicode or not
    /// a decimal or `0x`-prefixed hexadecimal `u64`, or if `generate`
    /// panics.
    pub fn run<T, V: Verdict>(
        &self,
        mut generate: impl FnMut(&mut Source) -> T,
        mut test: impl FnMut(&T) -> V,
    ) -> Result<(), Counterexample<T>> {
        self.run_seeded(self.resolved_seed(), &mut generate, &mut test)
    }

    /// Runs the property and panics with the counterexample's report, which
    /// names the seed, if any case fails.
    ///
    /// When the test failed by panicking, the minimal input is run once
    /// more without silencing the panic, so that it prints as any other
    /// panic does, with a backtrace when `RUST_BACKTRACE` asks for one.
    ///
    /// # Panics
    ///
    /// Panics if a case fails, and as [`Property::run`] does.
    pub fn check<T: fmt::Debug, V: Verdict>(
        &self,
        generate: impl FnMut(&mut Source) -> T,
        test: impl FnMut(&T) -> V,
    ) {
        self.check_described(generate, test, |value| format!("{value:?}"));
    }

    /// Runs the property as [`Property::check`] does, describing the
    /// minimal input in the report with `describe` instead of its `Debug`
    /// text.
    ///
    /// ```
    /// use ironafs_test_support::{Property, Source};
    ///
    /// Property::new("a byte string is as long as its length").check_described(
    ///     |source| source.bytes(0..=64),
    ///     |bytes| assert!(bytes.len() <= 64),
    ///     |bytes| format!("{} bytes", bytes.len()),
    /// );
    /// ```
    ///
    /// # Panics
    ///
    /// Panics if a case fails, and as [`Property::run`] does.
    #[expect(
        clippy::panic,
        reason = "a failed property must fail the calling test (rule 15)"
    )]
    pub fn check_described<T, V: Verdict>(
        &self,
        mut generate: impl FnMut(&mut Source) -> T,
        mut test: impl FnMut(&T) -> V,
        describe: impl FnOnce(&T) -> String,
    ) {
        let Err(counterexample) = self.run_seeded(self.resolved_seed(), &mut generate, &mut test)
        else {
            return;
        };
        if counterexample.location.is_some() {
            self.rerun_loudly(&counterexample.choices, &mut generate, &mut test);
        }
        panic!("{}", counterexample.report(describe));
    }

    /// Runs the test once on the input that `choices` give, such as a
    /// counterexample's minimal choices, and fails as [`Property::check`]
    /// does if it fails. This is how a failure is kept as a regression.
    ///
    /// Regressions are Rust tests in the owning crate's
    /// `tests/it/regressions.rs`, one `#[test]` each, which the failure
    /// report prints ready to paste. They are source files with the
    /// licence header like any other, tracked and run by every test run;
    /// the runner never writes regression data files of its own. To be
    /// callable from there, a property's generator and test are named
    /// functions, not closures.
    ///
    /// Choices keep their meaning when the way fresh runs draw them
    /// changes (the boundary odds, the string tiers, the weights), so a
    /// pinned regression stays valid; a change to what a choice means,
    /// which the golden tests catch, is the one thing that moves it.
    ///
    /// ```
    /// use ironafs_test_support::{Property, Source};
    ///
    /// fn generate(source: &mut Source) -> Vec<u8> {
    ///     source.vec_of(0..=8, Source::u8)
    /// }
    ///
    /// fn test(bytes: &Vec<u8>) {
    ///     assert!(bytes.len() <= 8);
    /// }
    ///
    /// Property::new("a vector stays within its length cap").check(generate, test);
    /// // Pinned from an earlier failure: one element, 255.
    /// Property::new("a vector stays within its length cap").regression(&[1, 255], generate, test);
    /// ```
    ///
    /// # Panics
    ///
    /// Panics with a report naming the property, the choices and the input
    /// if `test` returns an error, and propagates its panic (printed as
    /// any other panic, then followed by the same report on standard error)
    /// if it panics. A panic in `generate` is propagated unchanged.
    #[expect(
        clippy::panic,
        reason = "a failed regression must fail the calling test (rule 15)"
    )]
    pub fn regression<T: fmt::Debug, V: Verdict>(
        &self,
        choices: &[u64],
        generate: impl FnOnce(&mut Source) -> T,
        test: impl FnOnce(&T) -> V,
    ) {
        let value = generate(&mut Source::from_choices(choices.to_vec()));
        let report = |message: &str| {
            format!(
                "regression of property `{}` failed on choices {choices:?}\n\
                 input: {value:?}\n\
                 failure: {message}",
                self.name
            )
        };
        match panic::catch_unwind(AssertUnwindSafe(|| test(&value))) {
            Ok(verdict) => {
                if let Some(message) = verdict.failure() {
                    panic!("{}", report(&message));
                }
            }
            Err(payload) => {
                eprintln!("{}", report("the panic above"));
                panic::resume_unwind(payload);
            }
        }
    }

    /// The run's seed: `IRONAFS_TEST_SEED`'s, else the one fixed in code,
    /// else the one derived from the name.
    #[expect(
        clippy::panic,
        reason = "rule 1 covers untrusted input; a malformed seed variable is a mistake in the test run"
    )]
    fn resolved_seed(&self) -> u64 {
        let seed = seed_text(env::var(SEED_VARIABLE))
            .and_then(|text| resolve_seed(text.as_deref(), self.seed, &self.name));
        match seed {
            Ok(seed) => seed,
            Err(message) => panic!("{message}"),
        }
    }

    /// Runs the property from `seed`, without reading the environment,
    /// writing the first failure's notice to standard error and timing
    /// the shrink on the wall clock.
    pub(crate) fn run_seeded<T, V: Verdict>(
        &self,
        seed: u64,
        generate: &mut impl FnMut(&mut Source) -> T,
        test: &mut impl FnMut(&T) -> V,
    ) -> Result<(), Counterexample<T>> {
        // The shrink time limit guards against the test runner's own
        // wall-clock kill, so it reads the real clock (a departure from
        // rule 7, which covers the code under test, not the runner).
        let origin = Instant::now();
        self.run_with(
            seed,
            generate,
            test,
            &mut |notice| eprintln!("{notice}"),
            &mut || origin.elapsed(),
        )
    }

    /// Runs the property from `seed`. `notice` receives the line that
    /// announces the first failure, before shrinking starts; `now` reads a
    /// monotonic clock, against which the shrink time limit is measured.
    pub(crate) fn run_with<T, V: Verdict>(
        &self,
        seed: u64,
        generate: &mut impl FnMut(&mut Source) -> T,
        test: &mut impl FnMut(&T) -> V,
        notice: &mut impl FnMut(&str),
        now: &mut impl FnMut() -> Duration,
    ) -> Result<(), Counterexample<T>> {
        let mut case_seeds = SplitMix64::new(seed);
        for case in 0..self.cases {
            let mut source = Source::from_seed(case_seeds.next_u64());
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
            let Some(failure) = test_failure(test, &value) else {
                continue;
            };
            notice(&format!(
                "property `{}` failed on case {} of {} (seed {}); shrinking it now; \
                 replay with {SEED_VARIABLE}={}",
                self.name,
                u64::from(case) + 1,
                self.cases,
                SeedText(seed),
                SeedText(seed),
            ));
            let started = now();
            let limit = self.shrink_time_limit;
            let shrunk = shrink(
                source.into_recording(),
                (value, failure),
                self.shrink_budget,
                || now().saturating_sub(started) >= limit,
                |choices| {
                    let mut replay = Source::from_choices(choices.to_vec());
                    let value = catch_quietly(|| generate(&mut replay)).ok()?;
                    let failure = test_failure(test, &value)?;
                    Some((replay.into_recording(), (value, failure)))
                },
            );
            let (value, failure) = shrunk.payload;
            return Err(Counterexample {
                name: self.name.clone(),
                seed,
                case,
                cases: self.cases,
                value,
                message: failure.message,
                location: failure.location,
                choices: shrunk.choices,
                shrink_runs: shrunk.runs,
                shrink_budget_exhausted: shrunk.budget_exhausted,
                shrink_timed_out: shrunk.timed_out,
            });
        }
        Ok(())
    }

    /// Runs the test once more on the input `choices` give, with its panic
    /// printed by the process's panic hook.
    fn rerun_loudly<T, V: Verdict>(
        &self,
        choices: &[u64],
        generate: &mut impl FnMut(&mut Source) -> T,
        test: &mut impl FnMut(&T) -> V,
    ) {
        eprintln!(
            "property `{}`: running the minimal input once more, with its panic shown",
            self.name
        );
        let rerun = panic::catch_unwind(AssertUnwindSafe(|| {
            let value = generate(&mut Source::from_choices(choices.to_vec()));
            test(&value).failure()
        }));
        if matches!(rerun, Ok(None)) {
            eprintln!(
                "property `{}`: the minimal input passed when run again, so the test is not deterministic",
                self.name
            );
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

impl<E: fmt::Display> Verdict for Result<(), E> {
    fn failure(self) -> Option<String> {
        self.err().map(|error| error.to_string())
    }
}

/// How a test failed on one input.
struct Failure {
    /// The panic message or the error's text.
    message: String,
    /// Where the panic was raised; `None` for an error.
    location: Option<String>,
}

/// Runs `test` on `value` and returns its failure, if any.
fn test_failure<T, V: Verdict>(test: &mut impl FnMut(&T) -> V, value: &T) -> Option<Failure> {
    match catch_quietly(|| test(value)) {
        Ok(verdict) => verdict.failure().map(|message| Failure {
            message,
            location: None,
        }),
        Err(panic) => Some(Failure {
            message: panic.message,
            location: panic.location,
        }),
    }
}

/// The simplest failing input found for a property, and how to replay it.
///
/// The input is a local minimum: no deletion, zeroing or lowering of its
/// choices that the shrinker tries still fails. A simpler failing input
/// can exist that is reachable only through passing inputs: a property
/// that fails on 10 and on 1000 or more, but passes in between, shrinks a
/// large failing integer to 1000. The report does not say "may not be
/// minimal" in that case, because the shrinker cannot know.
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
    location: Option<String>,
    choices: Vec<u64>,
    shrink_runs: u32,
    shrink_budget_exhausted: bool,
    shrink_timed_out: bool,
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
    /// error's `Display` text.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Where the test panicked on the shrunk input, as `file:line:column`,
    /// or `None` if it returned an error.
    #[must_use]
    pub fn location(&self) -> Option<&str> {
        self.location.as_deref()
    }

    /// The shrunk input's choices. [`Source::from_choices`] replays them,
    /// so a regression test can pin this input without the seed.
    #[must_use]
    pub fn choices(&self) -> &[u64] {
        &self.choices
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

    /// Whether shrinking stopped at its time limit, so a simpler failing
    /// input may exist.
    #[must_use]
    pub fn shrink_timed_out(&self) -> bool {
        self.shrink_timed_out
    }
}

impl<T> Counterexample<T> {
    /// The failure report, describing the shrunk input with `describe`.
    #[must_use]
    pub fn report(&self, describe: impl FnOnce(&T) -> String) -> String {
        let location = self
            .location
            .as_ref()
            .map_or_else(String::new, |location| format!("panicked at: {location}\n"));
        let budget = if self.shrink_budget_exhausted {
            "; the shrink budget ran out, so the input may not be minimal"
        } else if self.shrink_timed_out {
            "; the shrink time limit ran out, so the input may not be minimal"
        } else {
            ""
        };
        format!(
            "property `{}` failed on case {} of {} (seed {})\n\
             minimal input: {}\n\
             failure: {}\n\
             {location}\
             minimal choices: {:?}\n\
             shrunk in {} runs{budget}\n\
             replay with {SEED_VARIABLE}={}\n\
             pin it with a regression test in tests/it/regressions.rs, \
             calling this property's generator and test:\n\
             #[test]\n\
             fn {}() {{\n    \
             Property::new({:?}).regression(&{:?}, generate, test);\n\
             }}",
            self.name,
            u64::from(self.case) + 1,
            self.cases,
            SeedText(self.seed),
            describe(&self.value),
            self.message,
            self.choices,
            self.shrink_runs,
            SeedText(self.seed),
            regression_test_name(&self.name),
            self.name,
            self.choices,
        )
    }
}

impl<T: fmt::Debug> fmt::Display for Counterexample<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.report(|value| format!("{value:?}")))
    }
}

/// A seed as printed: fixed-width hexadecimal, which [`parse_seed`] reads.
struct SeedText(u64);

impl fmt::Display for SeedText {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:#018x}", self.0)
    }
}

/// The name of the regression test the report prints: the property's
/// name in lowercase ASCII letters and digits joined by underscores, which
/// a Rust function name can hold.
fn regression_test_name(name: &str) -> String {
    let words: Vec<String> = name
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_ascii_lowercase)
        .collect();
    let starts_with_letter = words
        .first()
        .and_then(|word| word.chars().next())
        .is_some_and(|character| character.is_ascii_alphabetic());
    let prefix = if starts_with_letter { "" } else { "property_" };
    let joined = words.join("_");
    let separator = if joined.is_empty() { "" } else { "_" };
    format!("{prefix}{joined}{separator}holds_on_pinned_input")
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

/// Reads the seed variable: its text, `None` if it is unset, or an error
/// if it is not valid Unicode.
fn seed_text(variable: Result<String, VarError>) -> Result<Option<String>, String> {
    match variable {
        Ok(text) => Ok(Some(text)),
        Err(VarError::NotPresent) => Ok(None),
        Err(VarError::NotUnicode(text)) => Err(format!(
            "{SEED_VARIABLE} is not valid Unicode: \"{}\"",
            text.display()
        )),
    }
}

/// Chooses the run's seed: the environment's, else the one fixed in code,
/// else the one derived from the name. Empty or blank environment text
/// counts as unset.
fn resolve_seed(from_env: Option<&str>, explicit: Option<u64>, name: &str) -> Result<u64, String> {
    let from_env = from_env.filter(|text| !text.trim().is_empty());
    match (from_env, explicit) {
        (Some(text), _) => parse_seed(text).ok_or_else(|| {
            format!("{SEED_VARIABLE} is not a decimal or 0x-prefixed hexadecimal u64: {text:?}")
        }),
        (None, Some(seed)) => Ok(seed),
        (None, None) => Ok(seed_from_name(name)),
    }
}
