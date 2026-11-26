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
//! suffix), a shebang line, and a frontmatter fence (`---` as the first
//! token), since the compiler skips both unread.

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
    /// The text between the quotes of a plain or raw string literal, as
    /// written (escapes are not decoded). Only `tokens_with_strings` keeps
    /// strings; `tokens` drops every literal.
    Str(String),
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

/// A path named in the source.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct SourcePath {
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
/// a suffix, a shebang line, or a frontmatter fence.
pub(crate) fn tokens(source: &str) -> Result<Vec<Token>, LexError> {
    lex(source, false)
}

/// Splits Rust source into tokens as `tokens` does, but keeps the text of
/// each plain and raw string literal as a `TokenKind::Str`, for checks that
/// read an attribute's string argument. Byte, C and character literals are
/// still dropped.
///
/// # Errors
///
/// As `tokens`.
pub(crate) fn tokens_with_strings(source: &str) -> Result<Vec<Token>, LexError> {
    lex(source, true)
}

fn lex(source: &str, strings: bool) -> Result<Vec<Token>, LexError> {
    let mut lexer = Lexer {
        chars: source.chars().collect(),
        position: 0,
        line: 1,
        tokens: Vec::new(),
        strings,
    };
    // The compiler drops a byte order mark at the start of a file.
    if lexer.at(0) == Some('\u{feff}') {
        lexer.position = 1;
    }
    lexer.shebang()?;
    lexer.frontmatter()?;
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
    /// Whether plain and raw string literals are kept as tokens.
    strings: bool,
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

    /// Refuses a frontmatter fence: `---` as the first thing in the file
    /// after whitespace. A compiler that reads frontmatter skips its
    /// content unread, so a quote in it would start a literal here and not
    /// there. No item starts with `-`, so nothing valid is refused.
    fn frontmatter(&self) -> Result<(), LexError> {
        let mut offset = 0;
        let mut line = self.line;
        while let Some(next) = self.at(offset).filter(|next| next.is_whitespace()) {
            if next == '\n' {
                line += 1;
            }
            offset += 1;
        }
        if (offset..offset + 3).all(|index| self.at(index) == Some('-')) {
            Err(LexError {
                line,
                reason: "a frontmatter fence, whose content the compiler skips unread",
            })
        } else {
            Ok(())
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
                    let start = self.position;
                    self.quoted('"', "unterminated string", line)?;
                    self.keep_string(start, self.position.saturating_sub(1), line);
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

    /// Keeps the characters from `start` to `end` as a string token, if
    /// this lexer keeps strings.
    fn keep_string(&mut self, start: usize, end: usize, line: usize) {
        if self.strings {
            let text: String = self
                .chars
                .get(start..end)
                .unwrap_or_default()
                .iter()
                .collect();
            self.push(TokenKind::Str(text), line);
        }
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
                let (start, end) = self.raw_string(line)?;
                if word == "r" {
                    self.keep_string(start, end, line);
                }
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

    /// Skips a raw string whose prefix is consumed, and returns where its
    /// text starts and ends.
    fn raw_string(&mut self, line: usize) -> Result<(usize, usize), LexError> {
        let mut hashes = 0;
        while self.at(0) == Some('#') {
            self.position += 1;
            hashes += 1;
        }
        self.position += 1;
        let start = self.position;
        loop {
            match self.advance() {
                Some('"') => {
                    if (0..hashes).all(|offset| self.at(offset) == Some('#')) {
                        let end = self.position - 1;
                        self.position += hashes;
                        return Ok((start, end));
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
pub(crate) fn std_paths(tokens: &[Token]) -> Vec<SourcePath> {
    named_paths(tokens, "std")
}

/// Lists imports and qualified paths starting at `root`. A bare root
/// is included because a macro can use it to build a path.
pub(crate) fn named_paths(tokens: &[Token], root: &str) -> Vec<SourcePath> {
    paths(tokens, Some(root))
        .into_iter()
        .filter(|(found, _)| found.path.split("::").next() == Some(root))
        .map(|(found, _)| found)
        .collect()
}

/// Lists every path a `use` or `extern crate` item names, whatever crate it
/// starts at, with use trees expanded as in `std_paths`.
pub(crate) fn use_paths(tokens: &[Token]) -> Vec<SourcePath> {
    paths(tokens, None)
        .into_iter()
        .filter_map(|(found, imported)| imported.then_some(found))
        .collect()
}

/// One `use` item, read with its context, for a reader that must refuse
/// what it cannot resolve: the paths its tree imports (as segments, each
/// ending in the name it binds), the item's attributes (each the tokens
/// between `#[` and `]`, written out), how deep in braces it stands (0 at
/// file scope), and whether its tree holds a glob or a rename or anything
/// a plain tree of paths, groups and commas does not.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct UseItem {
    pub(crate) line: usize,
    pub(crate) paths: Vec<Vec<String>>,
    pub(crate) attributes: Vec<String>,
    pub(crate) depth: usize,
    pub(crate) glob: bool,
    pub(crate) renamed: bool,
    pub(crate) plain: bool,
}

/// Every `use` item among `tokens` (read with or without their strings),
/// with its context. The tree is expanded by the same reader as
/// `use_paths`.
pub(crate) fn use_items(tokens: &[Token]) -> Vec<UseItem> {
    let mut items = Vec::new();
    let mut depth = 0_usize;
    for (at, token) in tokens.iter().enumerate() {
        match punct(Some(token)) {
            Some('{') => depth += 1,
            Some('}') => depth = depth.saturating_sub(1),
            _ => {}
        }
        if ident(Some(token)) != Some("use") || token.raw {
            continue;
        }
        let end = tokens
            .get(at..)
            .unwrap_or_default()
            .iter()
            .position(|token| punct(Some(token)) == Some(';'))
            .map_or(tokens.len(), |offset| at + offset);
        let tree = tokens.get(at + 1..end).unwrap_or_default();
        let mut found = Vec::new();
        let consumed = use_tree(tree, 0, String::new(), &mut found);
        let plain = consumed == tree.len()
            && end < tokens.len()
            && tree.iter().all(|token| match &token.kind {
                TokenKind::Ident(word) => word != "as" || token.raw,
                TokenKind::PathSep => true,
                TokenKind::Punct(next) => matches!(next, '{' | '}' | ','),
                _ => false,
            });
        items.push(UseItem {
            line: token.line,
            paths: found
                .iter()
                .map(|path| path.split("::").map(str::to_owned).collect())
                .collect(),
            attributes: attributes_before(tokens, at),
            depth,
            glob: tree.iter().any(|token| punct(Some(token)) == Some('*')),
            renamed: tree
                .iter()
                .any(|token| ident(Some(token)) == Some("as") && !token.raw),
            plain,
        });
    }
    items
}

/// The attributes of the item whose keyword is at `at`, after any
/// visibility (`pub`, `pub(...)`), each written out without its `#[` and
/// `]`, in order.
fn attributes_before(tokens: &[Token], at: usize) -> Vec<String> {
    let mut start = at;
    // A visibility: `pub`, or `pub(...)`, its parenthesis matched with
    // nesting counted, as `os::scan::allowlist_range` matches its bracket.
    if start > 0 && punct(tokens.get(start - 1)) == Some(')') {
        let close = start - 1;
        let open = (0..close)
            .rev()
            .find(|&open| matching(tokens, open, '(', ')') == Some(close));
        if let Some(open) = open
            && open > 0
            && ident(tokens.get(open - 1)) == Some("pub")
        {
            start = open - 1;
        }
    } else if start > 0 && ident(tokens.get(start - 1)) == Some("pub") {
        start -= 1;
    }
    let mut found = Vec::new();
    while start > 0 && punct(tokens.get(start - 1)) == Some(']') {
        // The `[` this `]` closes, with nested brackets counted (as in
        // `#[doc = stringify!([x])]`), never the nearest `[`.
        let close = start - 1;
        let Some(open) = (0..close)
            .rev()
            .find(|&open| matching(tokens, open, '[', ']') == Some(close))
        else {
            break;
        };
        if open == 0 || punct(tokens.get(open - 1)) != Some('#') {
            break;
        }
        let text: Vec<String> = tokens
            .get(open + 1..start - 1)
            .unwrap_or_default()
            .iter()
            .map(|token| match &token.kind {
                TokenKind::Ident(word) => word.clone(),
                TokenKind::PathSep => "::".to_owned(),
                TokenKind::Punct(next) => next.to_string(),
                TokenKind::Str(text) => format!("{text:?}"),
                TokenKind::Lifetime => "'_".to_owned(),
            })
            .collect();
        found.push(text.concat());
        start = open - 1;
    }
    found.reverse();
    found
}

/// Every path of `use` and `extern crate` items, and every expression path
/// that starts at the requested root, each marked with whether an item
/// imported it.
fn paths(tokens: &[Token], root: Option<&str>) -> Vec<(SourcePath, bool)> {
    let mut paths = Vec::new();
    let mut index = 0;
    while let Some(token) = tokens.get(index) {
        let line = token.line;
        let mut found = Vec::new();
        let mut imported = true;
        index = match &token.kind {
            TokenKind::Ident(word) if word == "use" && !token.raw => {
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
                    && !token.raw
                    && matches!(tokens.get(index + 1), Some(Token { kind: TokenKind::Ident(next), .. }) if next == "crate") =>
            {
                use_tree(tokens, index + 2, String::new(), &mut found)
            }
            TokenKind::Ident(word)
                if root == Some(word.as_str()) && starts_a_path(tokens, index) =>
            {
                imported = false;
                if is(tokens, index + 1, &TokenKind::PathSep) {
                    expression_path(tokens, index, &mut found)
                } else {
                    // A bare root, as a macro argument or before a macro
                    // fragment, can build any path out of sight.
                    found.push(word.clone());
                    index + 1
                }
            }
            _ => index + 1,
        };
        paths.extend(
            found
                .into_iter()
                .map(|path| (SourcePath { path, line }, imported)),
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

/// The index of the bracket that closes the `opening` bracket at `open`,
/// counting nested pairs of the same kind; `None` if it is not closed or
/// `open` holds no `opening` bracket.
pub(crate) fn matching(
    tokens: &[Token],
    open: usize,
    opening: char,
    closing: char,
) -> Option<usize> {
    if punct(tokens.get(open)) != Some(opening) {
        return None;
    }
    matching_bracket(tokens.iter().enumerate().skip(open), opening, closing)
}

/// Follows one bracket pair in the order of the iterator, starting at the
/// first bracket. Reversing the iterator also reverses the pair.
fn matching_bracket<'a>(
    tokens: impl Iterator<Item = (usize, &'a Token)>,
    opening: char,
    closing: char,
) -> Option<usize> {
    let mut depth = 0_usize;
    for (index, token) in tokens {
        match punct(Some(token)) {
            Some(next) if next == opening => depth += 1,
            Some(next) if next == closing => {
                depth = depth.checked_sub(1)?;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
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
        TokenKind::PathSep => {
            let Some(at) = index.checked_sub(2) else {
                return true;
            };
            let stem = tokens.get(at);
            match stem.map(|token| &token.kind) {
                // These keywords can precede an absolute path, as in
                // `return ::std::...`. A raw keyword can be a path segment.
                Some(TokenKind::Ident(word)) => {
                    stem.is_some_and(|token| !token.raw)
                        && matches!(
                            word.as_str(),
                            "as" | "break"
                                | "const"
                                | "dyn"
                                | "for"
                                | "if"
                                | "impl"
                                | "in"
                                | "let"
                                | "match"
                                | "mut"
                                | "return"
                                | "where"
                                | "while"
                                | "yield"
                        )
                }
                Some(TokenKind::Punct('>')) => !closes_angle_path(tokens, at),
                _ => true,
            }
        }
        _ => true,
    }
}

/// Looks for the opening angle of a path ending in `>::`. The search skips
/// bracketed groups and stops at a statement, logical operator or enclosing
/// bracket. An arrow or a `for<...>` binder can precede an absolute path.
/// A comma at the candidate's depth can separate expressions, so only an
/// explicit turbofish proves that such a comma belongs to the candidate's
/// arguments. Commas inside a qualified type's nested arguments do not
/// require a turbofish.
fn closes_angle_path(tokens: &[Token], close: usize) -> bool {
    if ends_arrow(tokens, close) {
        return false;
    }
    let mut depth = 1;
    let mut crossed_comma = false;
    let mut cursor = close;
    while let Some(at) = cursor.checked_sub(1) {
        cursor = at;
        match punct(tokens.get(at)) {
            Some('>') if !ends_arrow(tokens, at) => depth += 1,
            Some('<') => {
                depth -= 1;
                if depth == 0 {
                    let before = at.checked_sub(1).and_then(|before| tokens.get(before));
                    return match before {
                        Some(Token {
                            kind: TokenKind::Ident(word),
                            raw: false,
                            ..
                        }) if word == "for" => false,
                        Some(Token {
                            kind: TokenKind::PathSep,
                            ..
                        }) => true,
                        _ => !crossed_comma,
                    };
                }
            }
            Some(',') if depth == 1 => crossed_comma = true,
            Some(end @ (')' | ']' | '}')) => {
                let start = match end {
                    ')' => '(',
                    ']' => '[',
                    _ => '{',
                };
                let before = tokens.iter().take(at + 1).enumerate().rev();
                let Some(open) = matching_bracket(before, end, start) else {
                    return false;
                };
                cursor = open;
            }
            Some('(' | '[' | '{' | ';') => return false,
            Some('|' | '&') if ends_logical_operator(tokens, at) => return false,
            _ => {}
        }
    }
    false
}

/// Whether `at` ends an arrow that can precede an absolute path or occur
/// within an angle-bracketed type.
fn ends_arrow(tokens: &[Token], at: usize) -> bool {
    matches!(
        at.checked_sub(1)
            .and_then(|before| punct(tokens.get(before))),
        Some('-' | '=')
    )
}

/// Logical operators separate comparisons, as in `a < b && c > ::name`.
/// Two ampersands can also start a reference type within angle brackets.
fn ends_logical_operator(tokens: &[Token], at: usize) -> bool {
    let Some(previous) = at.checked_sub(1) else {
        return false;
    };
    match (punct(tokens.get(previous)), punct(tokens.get(at))) {
        (Some('|'), Some('|')) => true,
        (Some('&'), Some('&')) => {
            let Some(left) = previous.checked_sub(1) else {
                return false;
            };
            match tokens.get(left) {
                Some(Token {
                    kind: TokenKind::Ident(word),
                    raw,
                    ..
                }) => *raw || !matches!(word.as_str(), "as" | "const" | "mut"),
                Some(Token {
                    kind: TokenKind::Punct('>'),
                    ..
                }) => !ends_arrow(tokens, left),
                Some(Token {
                    kind: TokenKind::Punct(')' | ']' | '}'),
                    ..
                }) => true,
                _ => false,
            }
        }
        _ => false,
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
            Some(TokenKind::Punct('$')) if !prefix.is_empty() => {
                // A macro fragment can select any item under the prefix.
                found.push(prefix);
                return index + 1;
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

/// The lines of every name `core` among `tokens` that can stand for the
/// first segment of a `core::...` path in its scope (or, through a glob
/// import, in another file): a module, a type (struct, enum, union, trait
/// or alias), a crate (`extern crate`), an import, a type parameter in any
/// position of a generic list, or a macro. Raw identifiers included. Names
/// of the value namespace (a binding, a parameter, a field, a function, a
/// constant or a static) cannot, and are not reported. With none reported
/// in a crate, every `core::...` path in it resolves through the extern
/// prelude to the core crate.
pub(crate) fn core_shadows(tokens: &[Token]) -> Vec<usize> {
    const ITEMS: &[&str] = &["mod", "struct", "enum", "union", "trait", "type"];
    let generic_lists = generic_lists(tokens);
    let mut in_import = false;
    let mut found = Vec::new();
    for (at, token) in tokens.iter().enumerate() {
        let word = ident(Some(token));
        if (word == Some("use") || (word == Some("crate") && previous_is(tokens, at, "extern")))
            && !token.raw
        {
            in_import = true;
        }
        if punct(Some(token)) == Some(';') {
            in_import = false;
        }
        if word != Some("core") || is(tokens, at + 1, &TokenKind::PathSep) {
            continue;
        }
        let before = at.checked_sub(1).and_then(|before| tokens.get(before));
        let item = ident(before).is_some_and(|word| ITEMS.contains(&word));
        let macro_name = punct(before) == Some('!') && previous_is(tokens, at - 1, "macro_rules");
        let parameter = generic_lists.iter().any(|(open, close)| {
            *open < at
                && at < *close
                && matches!(punct(before), Some('<' | ','))
                && matches!(
                    tokens.get(at + 1).and_then(|next| punct(Some(next))),
                    Some(':' | ',' | '>' | '=')
                )
        });
        if item || macro_name || parameter || in_import {
            found.push(token.line);
        }
    }
    found
}

/// Whether the token before `at` is the identifier `word`.
fn previous_is(tokens: &[Token], at: usize, word: &str) -> bool {
    at.checked_sub(1)
        .and_then(|before| tokens.get(before))
        .is_some_and(|token| ident(Some(token)) == Some(word))
}

/// The generic parameter lists among `tokens`, as the indices of their
/// `<` and `>`: a `<` right after `impl`, or after the name of a function,
/// struct, enum, union, trait or type alias. A `>` after `-` (an arrow in
/// a bound) does not close one.
fn generic_lists(tokens: &[Token]) -> Vec<(usize, usize)> {
    const NAMED: &[&str] = &["fn", "struct", "enum", "union", "trait", "type"];
    let mut lists = Vec::new();
    for (open, token) in tokens.iter().enumerate() {
        if punct(Some(token)) != Some('<') {
            continue;
        }
        let after_impl = previous_is(tokens, open, "impl");
        let after_name = open >= 2
            && ident(open.checked_sub(1).and_then(|before| tokens.get(before))).is_some()
            && open
                .checked_sub(2)
                .and_then(|before| tokens.get(before))
                .and_then(|token| ident(Some(token)))
                .is_some_and(|word| NAMED.contains(&word));
        if !(after_impl || after_name) {
            continue;
        }
        let mut depth = 0_usize;
        for (at, inner) in tokens.iter().enumerate().skip(open) {
            match punct(Some(inner)) {
                Some('<') => depth += 1,
                Some('>')
                    if at
                        .checked_sub(1)
                        .and_then(|before| punct(tokens.get(before)))
                        != Some('-') =>
                {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        lists.push((open, at));
                        break;
                    }
                }
                _ => {}
            }
        }
    }
    lists
}
