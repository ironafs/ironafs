// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! An exhaustive sweep: a test run on every input a generator can give,
//! for a domain small enough to enumerate.
//!
//! The sweep drives the same generators as a [`Property`](crate::Property),
//! but instead of drawing choices from a seed it visits every choice
//! sequence, depth first: all zeros first, then the last draw's next
//! choice, carrying into earlier draws like an odometer whose digits each
//! draw's range sets as it is drawn. Draws whose range depends on earlier
//! draws are enumerated as they are drawn. Each sequence gives a distinct
//! input, except where a generator itself maps two choices to one value.
//!
//! A sweep has a bound, the most inputs it runs. A domain larger than the
//! bound is refused once the bound is reached, rather than sampled, so a
//! sweep that passes has run every input, and its notice prints how many
//! and the bound (`IRONAFS_TEST_SEED` does not apply: there is no seed).

use std::fmt;
use std::panic::{self, AssertUnwindSafe};

use crate::Source;
use crate::property::{Verdict, regression_snippet, rerun_loudly, test_failure};

#[cfg(test)]
mod tests;

/// A test run on every input of a small domain, up to a stated bound.
///
/// ```
/// use ironafs_test_support::{Exhaustive, Source};
///
/// Exhaustive::new("every u16 round-trips through big-endian bytes", 1 << 16).check(
///     Source::u16,
///     |value| assert_eq!(u16::from_be_bytes(value.to_be_bytes()), *value),
/// );
/// ```
#[derive(Clone, Debug)]
pub struct Exhaustive {
    name: String,
    bound: u64,
}

impl Exhaustive {
    /// A sweep of at most `bound` inputs.
    ///
    /// # Panics
    ///
    /// Panics if `bound` is zero: a sweep that runs no input would pass
    /// without testing anything (rule 15).
    #[must_use]
    pub fn new(name: impl Into<String>, bound: u64) -> Self {
        assert!(
            bound > 0,
            "an exhaustive sweep needs a bound of at least one input"
        );
        Self {
            name: name.into(),
            bound,
        }
    }

    /// Runs `test` on every input `generate` can give, in choice order,
    /// and returns how many there were. On success it writes to standard
    /// error how many inputs passed and the bound.
    ///
    /// `test` fails as in [`Property::run`](crate::Property::run). A panic
    /// in `generate` is a mistake in the test: it is propagated after the
    /// input's position is written to standard error.
    ///
    /// # Errors
    ///
    /// Returns [`SweepFailure::Failed`] with the first input, in choice
    /// order, on which `test` fails, or [`SweepFailure::TooLarge`] if the
    /// domain has more inputs than the bound. Inputs are not shrunk: the
    /// first failing input is the one whose choices come first in
    /// lexicographic order.
    ///
    /// # Panics
    ///
    /// Panics if `generate` panics.
    pub fn run<T, V: Verdict>(
        &self,
        mut generate: impl FnMut(&mut Source) -> T,
        mut test: impl FnMut(&T) -> V,
    ) -> Result<u64, SweepFailure<T>> {
        self.run_with(&mut generate, &mut test, &mut |notice| {
            eprintln!("{notice}");
        })
    }

    /// Runs the sweep as [`Exhaustive::run`] does, and panics with the
    /// failure's report if it fails. When the test failed by panicking,
    /// the failing input is run once more with its panic shown, as
    /// [`Property::check`](crate::Property::check) does.
    ///
    /// # Panics
    ///
    /// Panics if an input fails, if the domain is larger than the bound,
    /// and as [`Exhaustive::run`] does.
    #[expect(
        clippy::panic,
        reason = "a failed sweep must fail the calling test (rule 15)"
    )]
    pub fn check<T: fmt::Debug, V: Verdict>(
        &self,
        mut generate: impl FnMut(&mut Source) -> T,
        mut test: impl FnMut(&T) -> V,
    ) {
        let Err(failure) = self.run(&mut generate, &mut test) else {
            return;
        };
        if let SweepFailure::Failed(failed) = &failure
            && failed.location.is_some()
        {
            self.rerun_failing(&failed.choices, &mut generate, &mut test, &mut |notice| {
                eprintln!("{notice}");
            });
        }
        panic!("{failure}");
    }

    /// Runs the test once more on the failing input `choices` give, with
    /// its panic shown; `notice` receives the lines that announce it.
    pub(crate) fn rerun_failing<T, V: Verdict>(
        &self,
        choices: &[u64],
        generate: &mut impl FnMut(&mut Source) -> T,
        test: &mut impl FnMut(&T) -> V,
        notice: &mut impl FnMut(&str),
    ) {
        rerun_loudly(
            &format!("exhaustive sweep `{}`", self.name),
            "failing input",
            choices,
            generate,
            test,
            notice,
        );
    }

    /// Runs the sweep; `notice` receives the line that reports success.
    pub(crate) fn run_with<T, V: Verdict>(
        &self,
        generate: &mut impl FnMut(&mut Source) -> T,
        test: &mut impl FnMut(&T) -> V,
        notice: &mut impl FnMut(&str),
    ) -> Result<u64, SweepFailure<T>> {
        let mut prefix = Vec::new();
        let mut index: u64 = 0;
        loop {
            let mut source = Source::enumerating(prefix);
            let value = match panic::catch_unwind(AssertUnwindSafe(|| generate(&mut source))) {
                Ok(value) => value,
                Err(payload) => {
                    eprintln!(
                        "exhaustive sweep `{}`: the generator panicked on input {}",
                        self.name,
                        index.saturating_add(1),
                    );
                    panic::resume_unwind(payload);
                }
            };
            let (choices, limits) = source.into_enumeration();
            if let Some(failure) = test_failure(test, &value) {
                return Err(SweepFailure::Failed(FailedInput {
                    name: self.name.clone(),
                    bound: self.bound,
                    index,
                    value,
                    choices: without_trailing_zeros(choices),
                    message: failure.message,
                    location: failure.location,
                }));
            }
            index = index.saturating_add(1);
            let Some(next) = next_choices(&choices, &limits) else {
                notice(&format!(
                    "exhaustive sweep `{}`: all {index} inputs passed (bound {})",
                    self.name, self.bound
                ));
                return Ok(index);
            };
            if index >= self.bound {
                return Err(SweepFailure::TooLarge {
                    name: self.name.clone(),
                    bound: self.bound,
                });
            }
            prefix = next;
        }
    }
}

/// The choices of the next input after the one that drew `choices`, each
/// at most its limit, or `None` after the last input: the last choice that
/// can still grow grows by one, and the draws after it start again from
/// zero.
fn next_choices(choices: &[u64], limits: &[u64]) -> Option<Vec<u64>> {
    let position = choices
        .iter()
        .zip(limits)
        .rposition(|(choice, limit)| choice < limit)?;
    let mut next: Vec<u64> = choices.iter().take(position).copied().collect();
    next.push(choices.get(position)?.saturating_add(1));
    Some(next)
}

/// `choices` without the zeros at its end, which a replay reads anyway.
fn without_trailing_zeros(mut choices: Vec<u64>) -> Vec<u64> {
    while choices.last() == Some(&0) {
        choices.pop();
    }
    choices
}

/// Why an exhaustive sweep failed.
#[derive(Debug)]
pub enum SweepFailure<T> {
    /// The test failed on an input.
    Failed(FailedInput<T>),
    /// The domain has more inputs than the sweep's bound. The inputs up
    /// to the bound passed.
    TooLarge {
        /// The sweep's name.
        name: String,
        /// The sweep's bound.
        bound: u64,
    },
}

impl<T: fmt::Debug> fmt::Display for SweepFailure<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Failed(failed) => {
                let location = failed
                    .location
                    .as_ref()
                    .map_or_else(String::new, |location| format!("panicked at: {location}\n"));
                write!(
                    formatter,
                    "exhaustive sweep `{}` failed on input {} of at most {}\n\
                     input: {:?}\n\
                     failure: {}\n\
                     {location}\
                     choices: {:?}\n\
                     {}",
                    failed.name,
                    failed.index.saturating_add(1),
                    failed.bound,
                    failed.value,
                    failed.message,
                    failed.choices,
                    regression_snippet(&failed.name, &failed.choices),
                )
            }
            Self::TooLarge { name, bound } => write!(
                formatter,
                "exhaustive sweep `{name}` has more than {bound} inputs; \
                 raise its bound, or test the domain as a Property"
            ),
        }
    }
}

/// The first input on which an exhaustive sweep's test failed.
#[derive(Debug)]
pub struct FailedInput<T> {
    name: String,
    bound: u64,
    index: u64,
    value: T,
    choices: Vec<u64>,
    message: String,
    location: Option<String>,
}

impl<T> FailedInput<T> {
    /// The failing input.
    #[must_use]
    pub fn value(&self) -> &T {
        &self.value
    }

    /// The zero-based position of the input in the sweep's order.
    #[must_use]
    pub fn index(&self) -> u64 {
        self.index
    }

    /// The input's choices, without trailing zeros.
    /// [`Source::from_choices`] and
    /// [`Property::regression`](crate::Property::regression) replay them.
    #[must_use]
    pub fn choices(&self) -> &[u64] {
        &self.choices
    }

    /// The test's failure: its panic message, or its error's `Display`
    /// text.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Where the test panicked, as `file:line:column`, or `None` if it
    /// returned an error.
    #[must_use]
    pub fn location(&self) -> Option<&str> {
        self.location.as_deref()
    }
}
