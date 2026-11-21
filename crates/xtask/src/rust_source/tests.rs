// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the Rust token reader and the `std` paths it finds.

use super::{LexError, Token, TokenKind, std_paths, tokens, use_paths};

fn idents(source: &str) -> Vec<String> {
    tokens(source)
        .unwrap()
        .into_iter()
        .filter_map(|token| match token.kind {
            TokenKind::Ident(name) => Some(name),
            _ => None,
        })
        .collect()
}

fn paths(source: &str) -> Vec<(String, usize)> {
    std_paths(&tokens(source).unwrap())
        .into_iter()
        .map(|path| (path.path, path.line))
        .collect()
}

#[test]
fn tokens_skip_comments_strings_and_character_literals() {
    let source = r##"
        // std::net in a line comment
        /* std::fs /* nested std::thread */ still comment */
        /// std::net in a doc comment
        let a = "std::net \" escaped";
        let b = r#"std::fs "quoted" inside"#;
        let c = br"std::thread";
        let d = b"std::net";
        let e = c"std::fs";
        let f = '"';
        let g = '\'';
        let h = b'x';
        let i = '\u{1F600}';
        fn life<'a>(x: &'a str) -> &'static str { x }
        let r#type = 1;
    "##;

    assert_eq!(
        idents(source),
        [
            "let", "a", "let", "b", "let", "c", "let", "d", "let", "e", "let", "f", "let", "g",
            "let", "h", "let", "i", "fn", "life", "a", "x", "a", "str", "static", "str", "x",
            "let", "type", "1",
        ]
    );
}

#[test]
fn tokens_count_lines_through_multi_line_comments_and_strings() {
    let lexed = tokens("a\n/* one\ntwo */ b \"x\ny\" c\n::").unwrap();

    assert_eq!(
        lexed,
        [
            Token {
                kind: TokenKind::Ident("a".into()),
                line: 1,
                raw: false
            },
            Token {
                kind: TokenKind::Ident("b".into()),
                line: 3,
                raw: false
            },
            Token {
                kind: TokenKind::Ident("c".into()),
                line: 4,
                raw: false
            },
            Token {
                kind: TokenKind::PathSep,
                line: 5,
                raw: false
            },
        ]
    );
}

#[test]
fn tokens_reject_unterminated_comments_and_strings() {
    for (source, line, reason) in [
        ("a /* b", 1, "unterminated block comment"),
        ("\n\"abc", 2, "unterminated string"),
        ("r#\"abc\"", 1, "unterminated string"),
        ("'\\", 1, "unterminated character literal"),
    ] {
        assert_eq!(
            tokens(source),
            Err(LexError { line, reason }),
            "source {source:?}"
        );
    }
}

#[test]
fn std_paths_expand_use_trees() {
    let source = "use std::{fs, net::{self, UdpSocket as U}, time::*};\n\
                  pub(crate) use ::std::thread::spawn;\n\
                  use {std::io, other::thing};\n\
                  use std as s;\n\
                  extern crate std as t;\n";

    assert_eq!(
        paths(source),
        [
            ("std::fs".into(), 1),
            ("std::net".into(), 1),
            ("std::net::UdpSocket".into(), 1),
            ("std::time::*".into(), 1),
            ("std::thread::spawn".into(), 2),
            ("std::io".into(), 3),
            ("std".into(), 4),
            ("std".into(), 5),
        ]
    );
}

#[test]
fn std_paths_find_expression_paths_and_ignore_other_crates() {
    let source = "let s = std::net::UdpSocket::bind(a);\n\
                  let t: Vec<std::thread::JoinHandle<()>> = x;\n\
                  let u = ::std::time::Instant::now();\n\
                  let v = other::std::net::Thing;\n\
                  let w = value.std;\n\
                  let x = core::net::Ipv4Addr::LOCALHOST;\n";

    assert_eq!(
        paths(source),
        [
            ("std::net::UdpSocket::bind".into(), 1),
            ("std::thread::JoinHandle".into(), 2),
            ("std::time::Instant::now".into(), 3),
        ]
    );
}

#[test]
fn std_paths_see_through_raw_identifiers_and_list_a_bare_std() {
    let source = "let a = r#std::r#fs::read(p);\n\
                  use r#std::{r#net};\n\
                  v!(std);\n\
                  macro_rules! w { ($($t:tt)*) => { std $($t)* } }\n\
                  let b = value.std;\n";

    assert_eq!(
        paths(source),
        [
            ("std::fs::read".into(), 1),
            ("std::net".into(), 2),
            ("std".into(), 3),
            ("std".into(), 4),
        ]
    );
}

#[test]
fn use_paths_list_every_imported_path_from_any_crate_and_no_expression_path() {
    let source = "use std::include as pull;\n\
                  extern crate core as c;\n\
                  use c::{println as say, fmt::Write};\n\
                  let x = std::fs::read(p);\n\
                  use crate::local::print;\n\
                  use self::inner::asm;\n";

    let found: Vec<(String, usize)> = use_paths(&tokens(source).unwrap())
        .into_iter()
        .map(|found| (found.path, found.line))
        .collect();
    assert_eq!(
        found,
        [
            ("std::include".into(), 1),
            ("core".into(), 2),
            ("c::println".into(), 3),
            ("c::fmt::Write".into(), 3),
            ("crate::local::print".into(), 5),
            ("self::inner::asm".into(), 6),
        ]
    );
}

#[test]
fn std_paths_end_on_malformed_use_trees() {
    assert_eq!(
        paths("use std::{fs; net};\nuse std::{thread"),
        [
            ("std::fs".into(), 1),
            ("std::net".into(), 1),
            ("std::thread".into(), 2),
        ]
    );
}
