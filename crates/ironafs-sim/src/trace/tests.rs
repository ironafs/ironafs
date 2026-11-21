// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The trace encoding: decision and operation lines read back as written,
//! the vocabulary refuses free text, and identities ignore metadata.

use super::{
    Decision, Entry, ErrorCode, EventKind, How, Op, TaskId, check_vocabulary, chosen_identities,
    decisions, is_word, ops, terminal,
};

fn decision() -> Decision {
    Decision {
        number: 12,
        enabled: vec![
            Entry::Task(TaskId(0)),
            Entry::Task(TaskId(3)),
            Entry::Event {
                kind: EventKind::Delivery,
                creation: 4,
            },
        ],
        pick: 2,
        how: How::Drawn,
        draws: 9,
        at: 1_500,
    }
}

#[test]
fn decision_renders_every_entry_and_parses_back() {
    let line = decision().render();
    assert_eq!(
        line,
        "d12 sched decide [t0,t3,delivery#4] pick=2 drawn draws=9 at=1500"
    );
    assert_eq!(Decision::parse(&line), Some(decision()));
    assert_eq!(
        decision().chosen(),
        Some(Entry::Event {
            kind: EventKind::Delivery,
            creation: 4
        })
    );
}

#[test]
fn decision_parse_refuses_altered_lines() {
    for line in [
        "d12 sched decide [t0,t3,delivery#4] pick=2 drawn draws=9",
        "d12 sched decide [t0,t3,loss#4] pick=2 drawn draws=9 at=1500",
        "d12 sched decide [t0,t03] pick=1 drawn draws=9 at=1500",
        "d12 sched decide [t0] pick=0 guessed draws=9 at=1500",
        "d12 sched decide [t0] pick=0 drawn draws=9 at=1500 extra",
        "d-1 sched decide [t0] pick=0 drawn draws=9 at=1500",
    ] {
        assert_eq!(Decision::parse(line), None, "{line}");
    }
}

#[test]
fn op_lines_parse_with_and_without_value() {
    let trace = "d3 t1 op get file:f 7\nd4 t2 op set file:f\nd4 world reply c1 00ff\n";
    assert_eq!(
        ops(trace),
        vec![
            Op {
                decision: 3,
                task: TaskId(1),
                label: "get".to_owned(),
                object: "file:f".to_owned(),
                value: Some(7),
            },
            Op {
                decision: 4,
                task: TaskId(2),
                label: "set".to_owned(),
                object: "file:f".to_owned(),
                value: None,
            },
        ]
    );
}

#[test]
fn words_are_a_closed_alphabet() {
    assert!(is_word("file:f.v2_x-1"));
    for word in ["", "File", "a b", "a\nb", "é", "0x7ff/3"] {
        assert!(!is_word(word), "{word:?}");
    }
}

#[test]
fn vocabulary_accepts_every_code_and_refuses_free_text() {
    let mut clean = String::new();
    for code in ErrorCode::ALL {
        clean.push_str("d1 t0 error ");
        clean.push_str(code.name());
        clean.push('\n');
    }
    assert_eq!(check_vocabulary(&clean), Ok(()));
    let planted = format!("{clean}d2 t0 error No such file or directory (os error 2)\n");
    let refused = check_vocabulary(&planted).unwrap_err();
    assert_eq!(refused.line, ErrorCode::ALL.len() + 1);
    for code in ErrorCode::ALL {
        assert_eq!(ErrorCode::parse(code.name()), Some(code));
    }
}

#[test]
fn identities_ignore_metadata() {
    let first = "ironafs-sim trace 2\nd0 sched run sample seed=1\n\
                 d0 sched decide [t0,t1] pick=1 drawn draws=1 at=5\n";
    let second = "ironafs-sim trace 2\nd0 sched run sample seed=2\n\
                  d0 sched decide [t0,t1] pick=1 drawn draws=1 at=9\n";
    assert_ne!(first, second);
    assert_eq!(chosen_identities(first), chosen_identities(second));
    assert_eq!(chosen_identities(first), vec![Entry::Task(TaskId(1))]);
    assert_eq!(decisions(first).len(), 1);
}

#[test]
fn terminal_reads_only_a_last_end_line() {
    assert_eq!(terminal("x\nd4 sched end ok\n"), Some("end ok"));
    assert_eq!(terminal("d4 sched end ok\nd5 t0 op a b\n"), None);
    assert_eq!(terminal(""), None);
    assert_eq!(
        terminal("d4 sched decide [t0] pick=0 drawn draws=1 at=0\n"),
        None
    );
}
