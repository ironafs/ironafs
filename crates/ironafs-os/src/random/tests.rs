// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the fill loop every random source shares, with a scripted
//! stand-in for the system call (ADR 0005, sheet 8: short counts, `EINTR`,
//! `ENOSYS`, zero and an over-long count).

use std::collections::VecDeque;
use std::io;

use super::{MAX_PIECE, Piece, RandomError, fill_with};

/// A stand-in that answers each call from `script` in turn, writing
/// `0xA5` into the bytes it reports filled, and records each request's
/// length.
struct Script {
    answers: VecDeque<Answer>,
    requests: Vec<usize>,
}

#[derive(Clone, Copy)]
enum Answer {
    /// Fill the whole request.
    Full,
    /// Report this many bytes filled.
    Count(usize),
    Interrupted,
    Unavailable(i32),
    Failed(i32),
}

impl Script {
    fn new(answers: &[Answer]) -> Self {
        Self {
            answers: answers.iter().copied().collect(),
            requests: Vec::new(),
        }
    }

    fn call(&mut self, piece: &mut [u8]) -> Piece {
        self.requests.push(piece.len());
        match self.answers.pop_front().unwrap_or(Answer::Full) {
            Answer::Full => {
                piece.fill(0xA5);
                Piece::Filled(piece.len())
            }
            Answer::Count(count) => {
                for byte in piece.iter_mut().take(count) {
                    *byte = 0xA5;
                }
                Piece::Filled(count)
            }
            Answer::Interrupted => Piece::Interrupted,
            Answer::Unavailable(errno) => Piece::Unavailable(io::Error::from_raw_os_error(errno)),
            Answer::Failed(errno) => Piece::Failed(io::Error::from_raw_os_error(errno)),
        }
    }
}

fn fill(length: usize, answers: &[Answer]) -> (Result<(), RandomError>, Vec<u8>, Vec<usize>) {
    let mut buffer = vec![0_u8; length];
    let mut script = Script::new(answers);
    let result = fill_with(&mut buffer, |piece| script.call(piece));
    (result, buffer, script.requests)
}

#[test]
fn fill_with_makes_no_call_for_an_empty_buffer() {
    let (result, _, requests) = fill(0, &[Answer::Failed(5)]);
    assert!(matches!(result, Ok(())));
    assert_eq!(requests, [] as [usize; 0]);
}

#[test]
fn fill_with_asks_at_most_256_bytes_per_call_until_the_buffer_is_full() {
    assert_eq!(MAX_PIECE, 256);
    for (length, expected) in [
        (1, vec![1]),
        (256, vec![256]),
        (257, vec![256, 1]),
        (4096, vec![256; 16]),
    ] {
        let (result, buffer, requests) = fill(length, &[]);
        assert!(matches!(result, Ok(())), "{length}");
        assert_eq!(requests, expected, "{length}");
        assert!(buffer.iter().all(|byte| *byte == 0xA5), "{length}");
    }
}

#[test]
fn fill_with_continues_after_a_short_count_from_where_it_stopped() {
    let (result, buffer, requests) = fill(300, &[Answer::Count(10), Answer::Count(1)]);
    assert!(matches!(result, Ok(())));
    assert_eq!(requests, [256, 256, 256, 33]);
    assert!(buffer.iter().all(|byte| *byte == 0xA5));
}

#[test]
fn fill_with_retries_an_interrupted_call() {
    let (result, buffer, requests) = fill(8, &[Answer::Interrupted, Answer::Interrupted]);
    assert!(matches!(result, Ok(())));
    assert_eq!(requests, [8, 8, 8]);
    assert!(buffer.iter().all(|byte| *byte == 0xA5));
}

#[test]
fn fill_with_reports_an_absent_or_filtered_source_with_its_errno() {
    for errno in [38, 1] {
        let (result, _, requests) = fill(300, &[Answer::Full, Answer::Unavailable(errno)]);
        match result {
            Err(RandomError::Unavailable(error)) => assert_eq!(error.raw_os_error(), Some(errno)),
            other => panic!("{errno}: {other:?}"),
        }
        assert_eq!(requests, [256, 44], "{errno}: no further call, no fallback");
    }
}

#[test]
fn fill_with_reports_any_other_failure_without_retrying() {
    let (result, _, requests) = fill(8, &[Answer::Failed(14)]);
    match result {
        Err(RandomError::Failed(error)) => assert_eq!(error.raw_os_error(), Some(14)),
        other => panic!("{other:?}"),
    }
    assert_eq!(requests, [8]);
}

#[test]
fn fill_with_refuses_a_call_that_returns_no_bytes() {
    let (result, _, requests) = fill(8, &[Answer::Count(0)]);
    assert!(matches!(result, Err(RandomError::NoBytes)), "{result:?}");
    assert_eq!(requests, [8]);
}

#[test]
fn fill_with_refuses_a_call_that_returns_more_than_was_asked() {
    let (result, _, _) = fill(8, &[Answer::Count(9)]);
    assert!(
        matches!(
            result,
            Err(RandomError::OverLong {
                asked: 8,
                returned: 9
            })
        ),
        "{result:?}"
    );
}

#[test]
fn random_errors_display_lowercase_and_name_the_errno() {
    let unavailable = RandomError::Unavailable(io::Error::from_raw_os_error(38)).to_string();
    assert!(
        unavailable.starts_with("random source unavailable or filtered: "),
        "{unavailable}"
    );
    assert!(unavailable.contains("os error 38"), "{unavailable}");
    assert_eq!(
        RandomError::OverLong {
            asked: 8,
            returned: 9
        }
        .to_string(),
        "random source returned 9 bytes for a request of 8"
    );
}
