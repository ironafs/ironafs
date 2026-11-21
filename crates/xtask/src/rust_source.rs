// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! A token reader for Rust source, enough to find the paths a file names.
//!
//! Comments, string and character literals are dropped, so text inside them
//! never counts as a use. Everything else becomes identifiers, lifetimes,
//! path separators and single punctuation characters.
//!
//! The reader follows the lexical rules of the 2024 edition, which
//! `check-deps` requires of every crate. Where it could part from the
//! compiler about where a literal starts or ends, it refuses the source
//! instead: a literal directly followed by an identifier character (a
//! suffix), and a shebang line, which the compiler skips unread.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::Error;

#[cfg(test)]
mod tests;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum TokenKind {
    /// An identifier, keyword or number. A raw identifier loses its `r#`:
    /// `r#std` names the same crate as `std`.
    Ident(String),
    /// A lifetime or a loop label, its name included: the name is never
    /// read as an identifier, so it cannot start a literal of its own.
    Lifetime,
    /// `::`
    PathSep,
    Punct(char),
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Token {
    pub(crate) kind: TokenKind,
    pub(crate) line: usize,
    /// Whether the token is a raw identifier, written with `r#`: then a
    /// keyword's spelling is only a name.
    pub(crate) raw: bool,
}

/// Where and why the source could not be read as tokens.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct LexError {
    line: usize,
    reason: &'static str,
}

impl fmt::Display for LexError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "line {}: {}", self.line, self.reason)
    }
}

/// A path that starts at the standard library, as written in the source.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct StdPath {
    pub(crate) path: String,
    pub(crate) line: usize,
}

/// Collects the `.rs` files under `dir` in path order. A directory or file
/// whose path `skip` accepts is left out with everything under it.
///
/// # Errors
///
/// Returns an error if a directory cannot be listed.
pub(crate) fn files(
    dir: &Path,
    skip: &impl Fn(&Path) -> bool,
    found: &mut Vec<PathBuf>,
) -> Result<(), Error> {
    let mut entries = fs::read_dir(dir)
        .and_then(|entries| {
            entries
                .map(|entry| entry.map(|entry| entry.path()))
                .collect::<Result<Vec<_>, _>>()
        })
        .map_err(Error::path_io("list source directory", dir))?;
    entries.sort();
    for path in entries {
        if skip(&path) {
            continue;
        }
        if path.is_dir() {
            files(&path, skip, found)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
    Ok(())
}

/// Reads a file as Rust tokens. The inner error says why the file is not
/// readable as Rust source: not UTF-8, or an unterminated comment or literal.
///
/// # Errors
///
/// Returns an error if the file cannot be read at all.
pub(crate) fn read(file: &Path) -> Result<Result<Vec<Token>, String>, Error> {
    match fs::read_to_string(file) {
        Ok(source) => Ok(tokens(&source).map_err(|error| error.to_string())),
        Err(error) if error.kind() == io::ErrorKind::InvalidData => Ok(Err("not UTF-8".into())),
        Err(error) => Err(Error::path_io("read source file", file)(error)),
    }
}

/// Splits Rust source into tokens.
///
/// # Errors
///
/// Returns the line of an unterminated comment or literal, a literal with
/// a suffix, or a shebang line.
pub(crate) fn tokens(source: &str) -> Result<Vec<Token>, LexError> {
    let mut lexer = Lexer {
        chars: source.chars().collect(),
        position: 0,
        line: 1,
        tokens: Vec::new(),
    };
    // The compiler drops a byte order mark at the start of a file.
    if lexer.at(0) == Some('\u{feff}') {
        lexer.position = 1;
    }
    lexer.shebang()?;
    lexer.run()?;
    Ok(lexer.tokens)
}

/// Whether `next` could be part of an identifier: `_`, an ASCII letter or
/// digit, or any character outside ASCII but whitespace. Wider than the
/// compiler's rule, so the reader fails closed.
fn identifier_like(next: char) -> bool {
    next == '_' || next.is_ascii_alphanumeric() || (!next.is_ascii() && !next.is_whitespace())
}

struct Lexer {
    chars: Vec<char>,
    position: usize,
    line: usize,
    tokens: Vec<Token>,
}

impl Lexer {
    fn at(&self, offset: usize) -> Option<char> {
        self.chars.get(self.position + offset).copied()
    }

    fn advance(&mut self) -> Option<char> {
        let next = self.at(0)?;
        self.position += 1;
        if next == '\n' {
            self.line += 1;
        }
        Some(next)
    }

    fn push(&mut self, kind: TokenKind, line: usize) {
        self.tokens.push(Token {
            kind,
            line,
            raw: false,
        });
    }

    /// Refuses a shebang line: `#!` at the start of the file not followed,
    /// after whitespace, by `[`. The compiler skips such a line unread, so
    /// a quote in it would start a literal here and not there. An inner
    /// attribute with a comment before its `[` is refused too.
    fn shebang(&self) -> Result<(), LexError> {
        if self.at(0) != Some('#') || self.at(1) != Some('!') {
            return Ok(());
        }
        let mut offset = 2;
        while self.at(offset).is_some_and(char::is_whitespace) {
            offset += 1;
        }
        if self.at(offset) == Some('[') {
            Ok(())
        } else {
            Err(LexError {
                line: self.line,
                reason: "a shebang line, which the compiler skips unread",
            })
        }
    }

    /// Refuses an identifier character directly after a literal that has
    /// just closed. The compiler reads it as the literal's suffix; this
    /// reader could read it as the prefix of another literal.
    fn literal_end(&self, line: usize) -> Result<(), LexError> {
        if self.at(0).is_some_and(identifier_like) {
            Err(LexError {
                line,
                reason: "a literal followed by an identifier character, which the compiler reads \
                         as a suffix",
            })
        } else {
            Ok(())
        }
    }

    fn run(&mut self) -> Result<(), LexError> {
        while let Some(next) = self.at(0) {
            let line = self.line;
            match next {
                '/' if self.at(1) == Some('/') => {
                    while self.at(0).is_some_and(|next| next != '\n') {
                        self.advance();
                    }
                }
                '/' if self.at(1) == Some('*') => self.block_comment()?,
                '"' => {
                    self.advance();
                    self.quoted('"', "unterminated string", line)?;
                    self.literal_end(line)?;
                }
                '\'' => self.quote_or_lifetime(line)?,
                ':' if self.at(1) == Some(':') => {
                    self.position += 2;
                    self.push(TokenKind::PathSep, line);
                }
                next if next == '_' || next.is_alphanumeric() => self.word(line)?,
                next => {
                    self.advance();
                    if !next.is_whitespace() {
                        self.push(TokenKind::Punct(next), line);
                    }
                }
            }
        }
        Ok(())
    }

    fn block_comment(&mut self) -> Result<(), LexError> {
        let line = self.line;
        let mut depth = 0_usize;
        loop {
            match (self.at(0), self.at(1)) {
                (Some('/'), Some('*')) => {
                    self.position += 2;
                    depth += 1;
                }
                (Some('*'), Some('/')) => {
                    self.position += 2;
                    depth -= 1;
                    if depth == 0 {
                        return Ok(());
                    }
                }
                (Some(_), _) => {
                    self.advance();
                }
                (None, _) => {
                    return Err(LexError {
                        line,
                        reason: "unterminated block comment",
                    });
                }
            }
        }
    }

    /// Skips the rest of a literal whose opening quote is consumed. The
    /// caller checks what follows it.
    fn quoted(&mut self, quote: char, reason: &'static str, line: usize) -> Result<(), LexError> {
        loop {
            match self.advance() {
                Some('\\') => {
                    self.advance();
                }
                Some(next) if next == quote => return Ok(()),
                Some(_) => {}
                None => return Err(LexError { line, reason }),
            }
        }
    }

    fn quote_or_lifetime(&mut self, line: usize) -> Result<(), LexError> {
        if self.at(1) == Some('\\') || self.at(2) == Some('\'') {
            self.advance();
            self.quoted('\'', "unterminated character literal", line)?;
            self.literal_end(line)
        } else {
            // A lifetime or a loop label, raw (`'r#name`) or not. Its name
            // is part of the token, so `'r"` is never a raw string.
            self.advance();
            if self.at(0) == Some('r')
                && self.at(1) == Some('#')
                && self.at(2).is_some_and(identifier_like)
            {
                self.position += 2;
            }
            while self.at(0).is_some_and(identifier_like) {
                self.position += 1;
            }
            self.push(TokenKind::Lifetime, line);
            Ok(())
        }
    }

    fn word(&mut self, line: usize) -> Result<(), LexError> {
        let start = self.position;
        while self
            .at(0)
            .is_some_and(|next| next == '_' || next.is_alphanumeric())
        {
            self.position += 1;
        }
        let word: String = self
            .chars
            .get(start..self.position)
            .unwrap_or_default()
            .iter()
            .collect();
        match (word.as_str(), self.at(0), self.at(1)) {
            ("r" | "br" | "cr", Some('"' | '#'), _) if self.raw_string_follows() => {
                self.raw_string(line)?;
                self.literal_end(line)
            }
            ("r", Some('#'), Some(next)) if next == '_' || next.is_alphanumeric() => {
                self.position += 1;
                let name_start = self.position;
                while self
                    .at(0)
                    .is_some_and(|next| next == '_' || next.is_alphanumeric())
                {
                    self.position += 1;
                }
                let name: String = self
                    .chars
                    .get(name_start..self.position)
                    .unwrap_or_default()
                    .iter()
                    .collect();
                self.tokens.push(Token {
                    kind: TokenKind::Ident(name),
                    line,
                    raw: true,
                });
                Ok(())
            }
            ("b" | "c", Some('"'), _) => {
                self.advance();
                self.quoted('"', "unterminated string", line)?;
                self.literal_end(line)
            }
            ("b", Some('\''), _) => {
                self.advance();
                self.quoted('\'', "unterminated character literal", line)?;
                self.literal_end(line)
            }
            _ => {
                self.push(TokenKind::Ident(word), line);
                Ok(())
            }
        }
    }

    fn raw_string_follows(&self) -> bool {
        let mut offset = 0;
        while self.at(offset) == Some('#') {
            offset += 1;
        }
        self.at(offset) == Some('"')
    }

    fn raw_string(&mut self, line: usize) -> Result<(), LexError> {
        let mut hashes = 0;
        while self.at(0) == Some('#') {
            self.position += 1;
            hashes += 1;
        }
        self.position += 1;
        loop {
            match self.advance() {
                Some('"') => {
                    if (0..hashes).all(|offset| self.at(offset) == Some('#')) {
                        self.position += hashes;
                        return Ok(());
                    }
                }
                Some(_) => {}
                None => {
                    return Err(LexError {
                        line,
                        reason: "unterminated string",
                    });
                }
            }
        }
    }
}

/// Lists every path that names the standard library, with use trees
/// expanded: `use std::{fs, net::*}` gives `std::fs` and `std::net::*`, and
/// `self` gives the path it ends. A `std` that no `::` follows is listed as
/// `std`.
pub(crate) fn std_paths(tokens: &[Token]) -> Vec<StdPath> {
    paths(tokens)
        .into_iter()
        .filter(|(found, _)| found.path == "std" || found.path.starts_with("std::"))
        .map(|(found, _)| found)
        .collect()
}

/// Lists every path a `use` or `extern crate` item names, whatever crate it
/// starts at, with use trees expanded as in `std_paths`.
pub(crate) fn use_paths(tokens: &[Token]) -> Vec<StdPath> {
    paths(tokens)
        .into_iter()
        .filter_map(|(found, imported)| imported.then_some(found))
        .collect()
}

/// Every path of `use` and `extern crate` items, and every expression path
/// that starts at `std`, each marked with whether an item imported it.
fn paths(tokens: &[Token]) -> Vec<(StdPath, bool)> {
    let mut paths = Vec::new();
    let mut index = 0;
    while let Some(token) = tokens.get(index) {
        let line = token.line;
        let mut found = Vec::new();
        let mut imported = true;
        index = match &token.kind {
            TokenKind::Ident(word) if word == "use" => {
                let start = index + 1;
                let start = if is(tokens, start, &TokenKind::PathSep) {
                    start + 1
                } else {
                    start
                };
                use_tree(tokens, start, String::new(), &mut found)
            }
            TokenKind::Ident(word)
                if word == "extern"
                    && matches!(tokens.get(index + 1), Some(Token { kind: TokenKind::Ident(next), .. }) if next == "crate") =>
            {
                use_tree(tokens, index + 2, String::new(), &mut found)
            }
            TokenKind::Ident(word) if word == "std" && starts_a_path(tokens, index) => {
                imported = false;
                if is(tokens, index + 1, &TokenKind::PathSep) {
                    expression_path(tokens, index, &mut found)
                } else {
                    // A bare `std`, as a macro argument or before a macro
                    // fragment, can build any path out of sight.
                    found.push("std".to_owned());
                    index + 1
                }
            }
            _ => index + 1,
        };
        paths.extend(
            found
                .into_iter()
                .map(|path| (StdPath { path, line }, imported)),
        );
    }
    paths
}

/// The name of an identifier token, raw or not.
pub(crate) fn ident(token: Option<&Token>) -> Option<&str> {
    match token.map(|token| &token.kind) {
        Some(TokenKind::Ident(word)) => Some(word),
        _ => None,
    }
}

/// The character of a punctuation token.
pub(crate) fn punct(token: Option<&Token>) -> Option<char> {
    match token.map(|token| &token.kind) {
        Some(TokenKind::Punct(next)) => Some(*next),
        _ => None,
    }
}

fn is(tokens: &[Token], index: usize, kind: &TokenKind) -> bool {
    tokens.get(index).is_some_and(|token| &token.kind == kind)
}

/// Whether the identifier at `index` begins a path rather than continuing
/// one (`other::std`) or naming a field (`value.std`).
fn starts_a_path(tokens: &[Token], index: usize) -> bool {
    let Some(before) = index.checked_sub(1).and_then(|before| tokens.get(before)) else {
        return true;
    };
    match &before.kind {
        TokenKind::Punct('.') => false,
        TokenKind::PathSep => !index
            .checked_sub(2)
            .and_then(|before| tokens.get(before))
            .is_some_and(|token| matches!(token.kind, TokenKind::Ident(_) | TokenKind::Punct('>'))),
        _ => true,
    }
}

fn join(prefix: &str, segment: &str) -> String {
    if prefix.is_empty() {
        segment.to_owned()
    } else {
        format!("{prefix}::{segment}")
    }
}

fn expression_path(tokens: &[Token], mut index: usize, found: &mut Vec<String>) -> usize {
    let mut path = String::new();
    while let Some(Token {
        kind: TokenKind::Ident(segment),
        ..
    }) = tokens.get(index)
    {
        path = join(&path, segment);
        index += 1;
        if !is(tokens, index, &TokenKind::PathSep) {
            break;
        }
        index += 1;
    }
    found.push(path);
    index
}

/// Expands one use tree starting at `index` and returns the index after it.
fn use_tree(
    tokens: &[Token],
    mut index: usize,
    mut prefix: String,
    found: &mut Vec<String>,
) -> usize {
    let mut named = false;
    loop {
        match tokens.get(index).map(|token| &token.kind) {
            // A `self` that starts a path names the current module; one
            // inside a path names the path so far.
            Some(TokenKind::Ident(segment)) if segment == "self" => {
                if prefix.is_empty() {
                    "self".clone_into(&mut prefix);
                }
                index += 1;
                named = true;
            }
            Some(TokenKind::Ident(segment)) => {
                prefix = join(&prefix, segment);
                index += 1;
                named = true;
            }
            Some(TokenKind::Punct('*')) => {
                found.push(join(&prefix, "*"));
                return index + 1;
            }
            Some(TokenKind::Punct('{')) => {
                index += 1;
                loop {
                    match tokens.get(index).map(|token| &token.kind) {
                        Some(TokenKind::Punct('}')) => return index + 1,
                        Some(TokenKind::Punct(',')) => index += 1,
                        Some(_) => {
                            let after = use_tree(tokens, index, prefix.clone(), found);
                            // A token no tree can start with is skipped.
                            index = after.max(index + 1);
                        }
                        None => return index,
                    }
                }
            }
            _ => {
                if named {
                    found.push(prefix);
                }
                return index;
            }
        }
        if is(tokens, index, &TokenKind::PathSep) {
            index += 1;
        } else {
            found.push(prefix);
            if matches!(tokens.get(index), Some(Token { kind: TokenKind::Ident(word), .. }) if word == "as")
            {
                index += 2;
            }
            return index;
        }
    }
}
