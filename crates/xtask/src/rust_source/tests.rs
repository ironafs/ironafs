// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for the Rust token reader and the `std` paths it finds.

use super::{
    LexError, Token, TokenKind, core_shadows, std_paths, tokens, tokens_with_strings, use_items,
    use_paths,
};

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
fn tokens_skip_comments_literals_and_lifetime_names() {
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
            "let", "h", "let", "i", "fn", "life", "x", "str", "str", "x", "let", "type", "1",
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
fn std_paths_find_absolute_roots_after_operators_and_where() {
    let source = "let _ = n > ::std::mem::size_of::<u8>();\n\
                  let _ = n >> ::std::mem::size_of::<u16>();\n\
                  let _ = 0 < n && n > ::std::mem::size_of::<u32>();\n\
                  fn bounded() where ::std::sync::Arc<u8>: Clone {}\n\
                  take(0 < n, n > ::std::mem::size_of::<u8>());\n";

    assert_eq!(
        paths(source),
        [
            ("std::mem::size_of".into(), 1),
            ("std::mem::size_of".into(), 2),
            ("std::mem::size_of".into(), 3),
            ("std::sync::Arc".into(), 4),
            ("std::mem::size_of".into(), 5),
        ]
    );
}

#[test]
fn std_paths_ignore_names_after_angle_bracket_path_tails() {
    let source = "let _ = Local::<Option<u8>>::std;\n\
                  let _ = ConstLocal::<{ 8 >> 1 }>::std;\n\
                  let _ = <fn() -> u8 as LocalTrait>::std;\n\
                  let _ = <*const &&u8 as LocalTrait>::std;\n\
                  let _ = Local::<u8, u16>::std;\n\
                  let _ = <Local<u8, u16> as LocalTrait>::std;\n";

    assert_eq!(paths(source), []);
}

#[test]
fn std_paths_ignore_qualified_function_pointer_tails() {
    let source = r"trait LocalTrait {
    #[expect(non_upper_case_globals)]
    const std: () = ();
}
impl<T> LocalTrait for T {}
fn local_items() {
    let _ = <fn(&u8) -> &&u8 as LocalTrait>::std;
    let _ = <for<'a> fn(&'a u8) -> &'a &'a u8 as LocalTrait>::std;
    let _ = <u8 as LocalTrait>::std;
}";

    // The bare declaration on line 3 is collected.
    assert_eq!(paths(source), [("std".into(), 3)]);
}

#[test]
fn std_paths_find_absolute_roots_after_higher_ranked_binders() {
    let source = r"fn higher_ranked_trait<F>()
where
    F: for<'a> ::std::ops::Fn(&'a u8),
{
}
fn higher_ranked_type()
where
    for<'a> ::std::slice::Iter<'a, u8>: Clone,
{
}
fn ordinary()
where
    ::std::sync::Arc<u8>: Clone,
{
}
fn logical(n: usize) {
    let _ = 0 < n && n > ::std::mem::size_of::<u8>();
}";

    assert_eq!(
        paths(source),
        [
            ("std::ops::Fn".into(), 3),
            ("std::slice::Iter".into(), 8),
            ("std::sync::Arc".into(), 13),
            ("std::mem::size_of".into(), 17),
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

#[test]
fn tokens_refuse_an_identifier_character_directly_after_a_literal() {
    for source in [
        "\"\"r\"\\\" \"",
        "b\"x\"s",
        "br\"x\"_s",
        "r#\"x\"#y",
        "c\"x\"z",
        "cr\"x\"z",
        "'a'b",
        "b'a'b",
        "'\\''s",
        "\"x\"\u{e9}",
    ] {
        assert_eq!(
            tokens(source),
            Err(LexError {
                line: 1,
                reason: "a literal followed by an identifier character, which the compiler reads \
                         as a suffix"
            }),
            "source {source:?}"
        );
    }
}

#[test]
fn tokens_accept_a_literal_followed_by_punctuation_or_space() {
    for source in [
        "f(\"x\", 'a')",
        "\"x\".len()",
        "r\"x\"#",
        "b'a'..b'z'",
        "[\"x\"]",
    ] {
        assert!(tokens(source).is_ok(), "source {source:?}");
    }
}

#[test]
fn tokens_read_a_lifetime_with_its_name_so_no_name_can_start_a_literal() {
    let lexed = tokens("drop_all!('r\"\\\" \");\ninclude!(\"x\"); // \"").unwrap();
    let kinds: Vec<TokenKind> = lexed.into_iter().map(|token| token.kind).collect();

    assert_eq!(
        kinds,
        [
            TokenKind::Ident("drop_all".into()),
            TokenKind::Punct('!'),
            TokenKind::Punct('('),
            TokenKind::Lifetime,
            TokenKind::Punct(')'),
            TokenKind::Punct(';'),
            TokenKind::Ident("include".into()),
            TokenKind::Punct('!'),
            TokenKind::Punct('('),
            TokenKind::Punct(')'),
            TokenKind::Punct(';'),
        ]
    );
}

#[test]
fn tokens_read_raw_and_non_ascii_lifetime_names_whole() {
    let kinds = |source: &str| -> Vec<TokenKind> {
        tokens(source)
            .unwrap()
            .into_iter()
            .map(|token| token.kind)
            .collect()
    };

    assert_eq!(kinds("'r#b \"x\""), [TokenKind::Lifetime]);
    assert_eq!(kinds("'a\u{b7}r\"\\\" \""), [TokenKind::Lifetime]);
    assert_eq!(
        kinds("'outer: loop {}"),
        [
            TokenKind::Lifetime,
            TokenKind::Punct(':'),
            TokenKind::Ident("loop".into()),
            TokenKind::Punct('{'),
            TokenKind::Punct('}'),
        ]
    );
}

#[test]
fn tokens_refuse_a_shebang_line() {
    for source in [
        "#!/usr/bin/env run\nfn main() {}",
        "#! \"\nfn main() {}",
        "\u{feff}#!/bin/x \"\n",
        "#!/*c*/[no_std]",
    ] {
        assert_eq!(
            tokens(source),
            Err(LexError {
                line: 1,
                reason: "a shebang line, which the compiler skips unread"
            }),
            "source {source:?}"
        );
    }
}

#[test]
fn tokens_refuse_a_frontmatter_fence() {
    for (source, line) in [
        ("---\nkey = \"\n---\nfn main() {}", 1),
        ("---cargo\n---\n", 1),
        ("\u{feff}---\n---\n", 1),
        ("\n\n  \t---\n'\n---\n", 3),
        ("\u{feff}\n-----\n-----\n", 2),
    ] {
        assert_eq!(
            tokens(source),
            Err(LexError {
                line,
                reason: "a frontmatter fence, whose content the compiler skips unread"
            }),
            "source {source:?}"
        );
    }
}

#[test]
fn tokens_read_dashes_after_the_first_token() {
    for source in ["fn f() -> i32 { --1 }\n---\n", "// c\n---\n", "--\n", "-\n"] {
        assert!(
            tokens(source).is_ok_and(|lexed| lexed
                .iter()
                .any(|token| token.kind == TokenKind::Punct('-'))),
            "source {source:?}"
        );
    }
}

#[test]
fn tokens_read_an_inner_attribute_at_the_start_and_a_leading_byte_order_mark() {
    for source in [
        "#![no_std]",
        "#!  [no_std]",
        "\u{feff}#![no_std]",
        "\u{feff}fn f() {}",
    ] {
        let lexed = tokens(source).unwrap();
        assert!(
            lexed
                .iter()
                .all(|token| token.kind != TokenKind::Punct('\u{feff}')),
            "source {source:?}"
        );
    }
    assert_eq!(
        tokens("#![no_std]")
            .unwrap()
            .first()
            .map(|token| &token.kind),
        Some(&TokenKind::Punct('#'))
    );
}

#[test]
fn tokens_with_strings_keep_the_text_of_plain_and_raw_strings_only() {
    let source = r#"#[expect(unsafe_code, reason = "linux-clock")] let a = r"x \y"; let b = b"no"; let c = c"no"; let d = "a\"b";"#;
    let strings: Vec<String> = tokens_with_strings(source)
        .unwrap()
        .into_iter()
        .filter_map(|token| match token.kind {
            TokenKind::Str(text) => Some(text),
            _ => None,
        })
        .collect();
    assert_eq!(strings, ["linux-clock", "x \\y", "a\\\"b"]);
    assert!(
        tokens(source)
            .unwrap()
            .iter()
            .all(|token| !matches!(token.kind, TokenKind::Str(_)))
    );
}

#[test]
fn core_shadows_finds_every_item_or_import_named_core() {
    for source in [
        "mod core { pub mod ffi {} }\n",
        "mod core;\n",
        "pub(crate) struct core;\n",
        "enum core {}\n",
        "type core = u8;\n",
        "trait core {}\n",
        "union core { a: u8 }\n",
        "extern crate std as core;\n",
        "use other::thing as core;\n",
        "use crate::shadow::core;\n",
        "use crate::shadow::{core, other};\n",
        "macro_rules! core { () => {} }\n",
        "fn f<core>() {}\n",
        "fn f<T, core>() {}\n",
        "impl<'a, T, core: Copy> S<'a, T> {}\n",
        "struct S<const N: usize, core>;\n",
        "fn f<F: Fn() -> u8, core>() {}\n",
        "trait T<core = u8> {}\n",
        "extern crate core;\n",
        "mod r#core {}\n",
    ] {
        let found = core_shadows(&tokens(source).unwrap());
        assert_eq!(found, [1], "{source}");
    }
    for source in [
        "use core::ffi::{c_int, c_long};\n",
        "use core::ptr;\nfn f() { core::ptr::null::<u8>(); }\n",
        "let x = ::core::mem::size_of::<u8>();\n",
        "// mod core\n",
        // The value namespace: none of these can stand for the first
        // segment of a `core::` path.
        "fn f() { let core = 1; let _ = core; }\n",
        "struct S { core: u8 }\nfn f(s: S) -> u8 { s.core }\n",
        "fn f(core: u8) -> u8 { core }\n",
        "fn g() -> S { S { core: 1 } }\n",
        "fn core() {}\n",
        "const core: u8 = 0;\n",
        "static core: u8 = 0;\n",
        "fn f(x: u8) -> bool { x < 1 && core::convert::identity(true) }\n",
    ] {
        assert_eq!(
            core_shadows(&tokens(source).unwrap()),
            Vec::<usize>::new(),
            "{source}"
        );
    }
}

#[test]
fn use_items_give_each_import_s_paths_attributes_depth_and_shape() {
    let source = "use core::{ffi::c_int, cell::{self, Cell}};\n\
                  #[cfg(target_env = \"gnu\")]\npub(super) use gnu::msghdr;\n\
                  fn f() { use super::*; }\nuse a::b as c;\nuse a::{b\n";
    let items = use_items(&tokens_with_strings(source).unwrap());
    let shape: Vec<_> = items
        .iter()
        .map(|item| {
            (
                item.paths
                    .iter()
                    .map(|path| path.join("::"))
                    .collect::<Vec<_>>(),
                item.attributes.clone(),
                item.depth,
                item.glob,
                item.renamed,
                item.plain,
            )
        })
        .collect();
    let paths = |list: &[&str]| {
        list.iter()
            .map(|path| (*path).to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        shape,
        [
            (
                paths(&["core::ffi::c_int", "core::cell", "core::cell::Cell"]),
                vec![],
                0,
                false,
                false,
                true
            ),
            (
                paths(&["gnu::msghdr"]),
                vec!["cfg(target_env=\"gnu\")".to_owned()],
                0,
                false,
                false,
                true
            ),
            (paths(&["super::*"]), vec![], 1, true, false, false),
            (paths(&["a::b"]), vec![], 0, false, true, false),
            (paths(&["a::b"]), vec![], 0, false, false, false),
        ]
    );
}

#[test]
fn use_items_match_each_attribute_s_brackets_with_nesting_counted() {
    let source = "#[cfg(miri)]\n#[doc = stringify!([x])]\npub(super) use core::ffi::c_int;\n\
                  #[doc = stringify!([x])]\nuse core::ffi::c_long;\n\
                  #[cfg(miri)]\npub(in crate::a) use b::c;\n\
                  #[cfg(miri)]\npub(crate) use d::{e, f};\n";
    let items = use_items(&tokens_with_strings(source).unwrap());
    let attributes: Vec<Vec<String>> = items.iter().map(|item| item.attributes.clone()).collect();
    assert_eq!(
        attributes,
        [
            vec!["cfg(miri)".to_owned(), "doc=stringify!([x])".to_owned()],
            vec!["doc=stringify!([x])".to_owned()],
            vec!["cfg(miri)".to_owned()],
            vec!["cfg(miri)".to_owned()],
        ]
    );
}
