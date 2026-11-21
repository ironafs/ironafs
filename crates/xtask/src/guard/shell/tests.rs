// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! Tests for word splitting, quoting, redirections, here-documents and
//! substitutions.

use super::{Item, ParseError, SimpleCommand, Stdin, Word, parse};

fn lit(text: &str) -> Word {
    Word::Literal(text.to_owned())
}

fn dynamic(prefix: &str) -> Word {
    Word::Dynamic(prefix.to_owned())
}

fn command(words: &[Word]) -> Item {
    Item::Command(SimpleCommand {
        words: words.to_vec(),
        stdin: Stdin::Terminal,
    })
}

fn with_stdin(words: &[Word], stdin: Stdin) -> Item {
    Item::Command(SimpleCommand {
        words: words.to_vec(),
        stdin,
    })
}

#[test]
fn parse_splits_words_and_removes_quotes() {
    assert_eq!(
        parse(r#"git commit -m 'a b' "c \"d\" \$e" f\ g"#).unwrap(),
        [command(&[
            lit("git"),
            lit("commit"),
            lit("-m"),
            lit("a b"),
            lit("c \"d\" $e"),
            lit("f g"),
        ])]
    );
}

#[test]
fn parse_removes_backslashes_inside_a_command_name() {
    assert_eq!(
        parse(r"\g\it push").unwrap(),
        [command(&[lit("git"), lit("push")])]
    );
    assert_eq!(
        parse("g''it pu\"sh\"").unwrap(),
        [command(&[lit("git"), lit("push")])]
    );
}

#[test]
fn parse_decodes_ansi_c_quotes_and_marks_numeric_escapes_dynamic() {
    assert_eq!(
        parse(r"echo $'a\nb\'c'").unwrap(),
        [command(&[lit("echo"), lit("a\nb'c")])]
    );
    assert_eq!(
        parse(r"$'\x67it' push").unwrap(),
        [command(&[dynamic(""), lit("push")])]
    );
}

#[test]
fn parse_splits_commands_at_every_separator() {
    assert_eq!(
        parse("a; b && c || d & e\nf").unwrap(),
        ["a", "b", "c", "d", "e", "f"].map(|name| command(&[lit(name)]))
    );
}

#[test]
fn parse_marks_piped_input() {
    assert_eq!(
        parse("cat msg | git commit -F -").unwrap(),
        [
            command(&[lit("cat"), lit("msg")]),
            with_stdin(
                &[lit("git"), lit("commit"), lit("-F"), lit("-")],
                Stdin::Pipe
            ),
        ]
    );
}

#[test]
fn parse_skips_comments() {
    assert_eq!(
        parse("# git push\nls # git push").unwrap(),
        [command(&[lit("ls")])]
    );
    assert_eq!(
        parse("echo a#b").unwrap(),
        [command(&[lit("echo"), lit("a#b")])]
    );
}

#[test]
fn parse_removes_redirections_from_words() {
    assert_eq!(
        parse("git log >out 2>&1 &>>all").unwrap(),
        [command(&[lit("git"), lit("log")])]
    );
    assert_eq!(
        parse("git commit -F - < msg.txt").unwrap(),
        [with_stdin(
            &[lit("git"), lit("commit"), lit("-F"), lit("-")],
            Stdin::File(Some("msg.txt".to_owned()))
        )]
    );
}

#[test]
fn parse_reads_here_documents_into_stdin() {
    let script = "git commit -F - <<'EOF'\nsubject\n\nAssisted-by: none\nEOF\necho done";
    assert_eq!(
        parse(script).unwrap(),
        [
            with_stdin(
                &[lit("git"), lit("commit"), lit("-F"), lit("-")],
                Stdin::Text(Some("subject\n\nAssisted-by: none\n".to_owned()))
            ),
            command(&[lit("echo"), lit("done")]),
        ]
    );
}

#[test]
fn parse_strips_tabs_and_marks_expanding_here_documents() {
    assert_eq!(
        parse("cat <<-END\n\tline\n\tEND").unwrap(),
        [with_stdin(
            &[lit("cat")],
            Stdin::Text(Some("line\n".to_owned()))
        )]
    );
    assert_eq!(
        parse("cat <<END\nplain \\$ text\nEND").unwrap(),
        [with_stdin(
            &[lit("cat")],
            Stdin::Text(Some("plain $ text\n".to_owned()))
        )]
    );
    assert_eq!(
        parse("cat <<END\n$HOME\nEND").unwrap(),
        [with_stdin(&[lit("cat")], Stdin::Text(None))]
    );
}

#[test]
fn parse_reads_here_strings() {
    assert_eq!(
        parse("gh pr comment 1 -F - <<< 'text'").unwrap(),
        [with_stdin(
            &[
                lit("gh"),
                lit("pr"),
                lit("comment"),
                lit("1"),
                lit("-F"),
                lit("-")
            ],
            Stdin::Text(Some("text\n".to_owned()))
        )]
    );
}

#[test]
fn parse_reads_a_cat_here_document_substitution_as_literal_text() {
    let script = "git commit -m \"$(cat <<'EOF'\nsubject\n\nAssisted-by: none\nEOF\n)\"";
    let items = parse(script).unwrap();
    assert_eq!(
        items.last(),
        Some(&command(&[
            lit("git"),
            lit("commit"),
            lit("-m"),
            lit("subject\n\nAssisted-by: none"),
        ]))
    );
}

#[test]
fn parse_emits_substituted_commands_in_a_subshell_before_their_user() {
    assert_eq!(
        parse("echo \"$(git push origin main)\" `git status` $HOME ${X}").unwrap(),
        [
            Item::Enter,
            command(&[lit("git"), lit("push"), lit("origin"), lit("main")]),
            Item::Leave,
            Item::Enter,
            command(&[lit("git"), lit("status")]),
            Item::Leave,
            command(&[
                lit("echo"),
                dynamic(""),
                dynamic(""),
                dynamic(""),
                dynamic("")
            ]),
        ]
    );
}

#[test]
fn parse_keeps_the_literal_prefix_of_a_dynamic_word() {
    assert_eq!(
        parse("SHA=$(git rev-parse HEAD)").unwrap(),
        [
            Item::Enter,
            command(&[lit("git"), lit("rev-parse"), lit("HEAD")]),
            Item::Leave,
            command(&[dynamic("SHA=")]),
        ]
    );
}

#[test]
fn parse_marks_subshells() {
    assert_eq!(
        parse("(cd a && git push); ls").unwrap(),
        [
            Item::Enter,
            command(&[lit("cd"), lit("a")]),
            command(&[lit("git"), lit("push")]),
            Item::Leave,
            command(&[lit("ls")]),
        ]
    );
}

#[test]
fn parse_reads_function_bodies_as_commands() {
    assert_eq!(
        parse("g() { git \"$@\"; }; g push").unwrap(),
        [
            command(&[lit("{"), lit("git"), dynamic("")]),
            command(&[lit("}")]),
            command(&[lit("g"), lit("push")]),
        ]
    );
}

#[test]
fn parse_rejects_unterminated_input() {
    for script in [
        "echo 'a",
        "echo \"a",
        "echo $(ls",
        "echo `ls",
        "cat <<EOF\nno end",
        "echo \\",
        "(ls",
        "ls )",
        "echo ${a",
    ] {
        assert!(
            matches!(parse(script), Err(ParseError(_))),
            "{script:?} parsed"
        );
    }
}

#[test]
fn parse_rejects_nesting_past_its_depth_cap() {
    let script = format!("{}ls{}", "(".repeat(100), ")".repeat(100));
    assert_eq!(parse(&script), Err(ParseError("nesting too deep")));
}
