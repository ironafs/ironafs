// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The trace: one line per event, in a fixed text encoding (seams and
//! scheduling specification, 5.6, point 11, (c)).
//!
//! Every line is ASCII and ends in a newline. After the version line and
//! the header, each line starts with the number of the decision whose
//! step wrote it, then who acted: a task (`t3`), the world (`world`) or
//! the scheduler (`sched`). Decision lines carry the whole enabled list,
//! each entry by its identity, the chosen index, whether it was drawn,
//! the schedule stream's draw count and the virtual boot time, so a leak
//! that changed a task id, a draw or a time shows in the bytes.
//!
//! A trace holds no pointer, thread id, thread name or host value, and no
//! error text: an error is a code from [`ErrorCode`]'s closed list, and a
//! trace naming anything else is refused by [`check_vocabulary`].

use std::fmt;
use std::fmt::Write as _;

#[cfg(test)]
mod tests;

/// The first line of every trace: the trace format's version. A change to
/// what a trace says, or how, moves it, and the golden fixtures are
/// regenerated in the same commit.
pub const VERSION_LINE: &str = "ironafs-sim trace 2";

/// The most bytes a golden fixture may hold (specification, 5.6, point
/// 11; section 13, Q15).
pub const FIXTURE_CAP: usize = 256 * 1024;

/// A task's identity, assigned by the scheduler from a counter when it
/// decides to create the task (5.6, point 4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TaskId(pub u64);

impl fmt::Display for TaskId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "t{}", self.0)
    }
}

/// The kinds of world event, in the enabled list's order (5.6, point 5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventKind {
    /// A server model's reply delivered to a waiting call.
    Delivery,
}

impl EventKind {
    const ALL: [Self; 1] = [Self::Delivery];

    /// The kind's name in a trace.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Delivery => "delivery",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.name() == text)
    }
}

/// One entry of the enabled list: a task, or a world event by its kind
/// and creation number.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Entry {
    /// A task whose wake condition holds, or one not yet started.
    Task(TaskId),
    /// A world event due now.
    Event {
        /// What the event does.
        kind: EventKind,
        /// The event's number, from the scheduler's counter.
        creation: u64,
    },
}

impl fmt::Display for Entry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Task(task) => task.fmt(formatter),
            Self::Event { kind, creation } => write!(formatter, "{}#{creation}", kind.name()),
        }
    }
}

impl Entry {
    fn parse(text: &str) -> Option<Self> {
        if let Some(number) = text.strip_prefix('t') {
            return parse_u64(number).map(|id| Self::Task(TaskId(id)));
        }
        let (kind, creation) = text.split_once('#')?;
        Some(Self::Event {
            kind: EventKind::parse(kind)?,
            creation: parse_u64(creation)?,
        })
    }
}

/// How a decision chose its entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum How {
    /// Drawn from the schedule stream.
    Drawn,
    /// The overdue entry of greatest age, run without a draw (5.6, point
    /// 6, fairness).
    Overdue,
}

impl How {
    const fn name(self) -> &'static str {
        match self {
            Self::Drawn => "drawn",
            Self::Overdue => "overdue",
        }
    }
}

/// A decision line, parsed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decision {
    /// The decision's number, from 0.
    pub number: u64,
    /// The whole enabled list, in order.
    pub enabled: Vec<Entry>,
    /// The index of the chosen entry.
    pub pick: usize,
    /// Whether the index was drawn.
    pub how: How,
    /// The schedule stream's draw count after this decision.
    pub draws: u64,
    /// The virtual boot time of the decision, in nanoseconds.
    pub at: u64,
}

impl Decision {
    /// The chosen entry, if the index is inside the list.
    #[must_use]
    pub fn chosen(&self) -> Option<Entry> {
        self.enabled.get(self.pick).copied()
    }

    /// The decision's line.
    #[must_use]
    pub fn render(&self) -> String {
        let mut line = format!("d{} sched decide [", self.number);
        for (index, entry) in self.enabled.iter().enumerate() {
            if index > 0 {
                line.push(',');
            }
            let _ = write!(line, "{entry}");
        }
        let _ = write!(
            line,
            "] pick={} {} draws={} at={}",
            self.pick,
            self.how.name(),
            self.draws,
            self.at
        );
        line
    }

    /// Parses a decision line; `None` if `line` is not one.
    #[must_use]
    pub fn parse(line: &str) -> Option<Self> {
        let rest = line.strip_prefix('d')?;
        let (number, rest) = rest.split_once(" sched decide [")?;
        let (list, rest) = rest.split_once("] pick=")?;
        let mut words = rest.split(' ');
        let pick = words.next().and_then(parse_u64)?;
        let how = match words.next()? {
            "drawn" => How::Drawn,
            "overdue" => How::Overdue,
            _ => return None,
        };
        let draws = words.next()?.strip_prefix("draws=").and_then(parse_u64)?;
        let at = words.next()?.strip_prefix("at=").and_then(parse_u64)?;
        if words.next().is_some() {
            return None;
        }
        let enabled = if list.is_empty() {
            Vec::new()
        } else {
            list.split(',')
                .map(Entry::parse)
                .collect::<Option<Vec<_>>>()?
        };
        Some(Self {
            number: parse_u64(number)?,
            enabled,
            pick: usize::try_from(pick).ok()?,
            how,
            draws,
            at,
        })
    }
}

/// An operation event: a named step of the code under test on one object,
/// such as a race scenario's meeting event (5.6, point 6).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Op {
    /// The decision whose step wrote it.
    pub decision: u64,
    /// The task that wrote it.
    pub task: TaskId,
    /// What happened.
    pub label: String,
    /// The object it happened to: call, request, file and version.
    pub object: String,
    /// A value it carried, if any.
    pub value: Option<u64>,
}

impl Op {
    /// Parses an operation line; `None` if `line` is not one.
    #[must_use]
    pub fn parse(line: &str) -> Option<Self> {
        let rest = line.strip_prefix('d')?;
        let mut words = rest.split(' ');
        let decision = words.next().and_then(parse_u64)?;
        let task = words.next()?.strip_prefix('t').and_then(parse_u64)?;
        if words.next()? != "op" {
            return None;
        }
        let label = words.next()?.to_owned();
        let object = words.next()?.to_owned();
        let value = match words.next() {
            None => None,
            Some(value) => Some(parse_u64(value)?),
        };
        if words.next().is_some() {
            return None;
        }
        Some(Self {
            decision,
            task: TaskId(task),
            label,
            object,
            value,
        })
    }
}

/// Whether `word` may appear as a label or object name in a trace: one or
/// more of `a-z`, `0-9`, `-`, `_`, `:` and `.`, so a word never holds a
/// space, a newline or text from outside the code under test's own names.
#[must_use]
pub fn is_word(word: &str) -> bool {
    !word.is_empty()
        && word
            .bytes()
            .all(|byte| matches!(byte, b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b':' | b'.'))
}

/// The closed vocabulary of errors a trace may name (5.6, point 11, (c)):
/// the seams' error kinds, never an operating system's message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ErrorCode {
    /// The boot clock could not be read.
    ClockBoot,
    /// The awake clock could not be read.
    ClockAwake,
    /// Wall time could not be read.
    ClockWall,
    /// The random source could not supply bytes.
    Rng,
    /// A wait's deadline passed.
    DeadlinePassed,
    /// A wait's cancel token fired.
    Cancelled,
    /// A deadline could not be built: its arithmetic overflowed.
    DeadlineOverflow,
}

impl ErrorCode {
    /// Every code, in a fixed order.
    pub const ALL: [Self; 7] = [
        Self::ClockBoot,
        Self::ClockAwake,
        Self::ClockWall,
        Self::Rng,
        Self::DeadlinePassed,
        Self::Cancelled,
        Self::DeadlineOverflow,
    ];

    /// The code's name in a trace.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ClockBoot => "clock-boot",
            Self::ClockAwake => "clock-awake",
            Self::ClockWall => "clock-wall",
            Self::Rng => "rng",
            Self::DeadlinePassed => "deadline-passed",
            Self::Cancelled => "cancelled",
            Self::DeadlineOverflow => "deadline-overflow",
        }
    }

    /// The code named `text`, if the vocabulary has one.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|code| code.name() == text)
    }
}

/// A trace line names an error outside the vocabulary: a simulator error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OutsideVocabulary {
    /// The line's number in the trace, from 1.
    pub line: usize,
}

impl fmt::Display for OutsideVocabulary {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "trace line {} names an error outside the simulator's vocabulary",
            self.line
        )
    }
}

impl std::error::Error for OutsideVocabulary {}

/// Checks that every error line of `trace` names one code of
/// [`ErrorCode`] and nothing else.
///
/// # Errors
///
/// Returns the first line that names anything else.
pub fn check_vocabulary(trace: &str) -> Result<(), OutsideVocabulary> {
    for (index, line) in trace.lines().enumerate() {
        let Some((_, error)) = line.split_once(" error ") else {
            continue;
        };
        if ErrorCode::parse(error).is_none() {
            return Err(OutsideVocabulary {
                line: index.saturating_add(1),
            });
        }
    }
    Ok(())
}

/// Every decision line of `trace`, in order.
#[must_use]
pub fn decisions(trace: &str) -> Vec<Decision> {
    trace.lines().filter_map(Decision::parse).collect()
}

/// Every operation line of `trace`, in order.
#[must_use]
pub fn ops(trace: &str) -> Vec<Op> {
    trace.lines().filter_map(Op::parse).collect()
}

/// The chosen entry of every decision of `trace`: what two seeds are
/// compared on, never the seed or other metadata a trace prints
/// (specification, 10.5, point 5).
#[must_use]
pub fn chosen_identities(trace: &str) -> Vec<Entry> {
    decisions(trace)
        .iter()
        .filter_map(Decision::chosen)
        .collect()
}

/// The trace's terminal line, if its last line is one: `end ok`, or `end`
/// with a failure.
#[must_use]
pub fn terminal(trace: &str) -> Option<&str> {
    trace
        .lines()
        .last()
        .and_then(|line| line.split_once(" sched "))
        .map(|(_, end)| end)
        .filter(|end| end.starts_with("end "))
}

fn parse_u64(text: &str) -> Option<u64> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    if text.len() > 1 && text.starts_with('0') {
        return None;
    }
    text.parse().ok()
}
