// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! A reader for the subset of POSIX shell syntax that decides which simple
//! commands a Bash tool call runs.

use std::fmt;

#[cfg(test)]
mod tests;

/// One word of a simple command after quote removal.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Word {
    Literal(String),
    /// Text that depends on an expansion done at run time. It keeps literal
    /// text that starts with the text before the first expansion, enough to
    /// recognise an assignment such as `NAME=$(…)`.
    Dynamic(String),
}

impl Word {
    pub(super) fn literal(&self) -> Option<&str> {
        match self {
            Self::Literal(text) => Some(text),
            Self::Dynamic(_) => None,
        }
    }
}

/// Where a simple command's standard input comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Stdin {
    Terminal,
    /// A here-document or here-string; `None` when its text expands at run time.
    Text(Option<String>),
    /// A file redirected with `<`; `None` when its name expands at run time.
    File(Option<String>),
    Pipe,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SimpleCommand {
    pub(super) words: Vec<Word>,
    pub(super) stdin: Stdin,
}

/// A simple command, or the start or end of a subshell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Item {
    Command(SimpleCommand),
    Enter,
    Leave,
}

#[derive(Debug, PartialEq, Eq)]
pub(super) struct ParseError(&'static str);

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.0)
    }
}

/// Deepest nesting of subshells and substitutions accepted.
const MAX_DEPTH: usize = 32;

/// Splits a script into the simple commands it runs, in order.
///
/// Commands inside a substitution come before the command that uses its
/// output, between [`Item::Enter`] and [`Item::Leave`]. A substitution of
/// exactly `cat` reading a here-document is replaced by that text, as the
/// shell would; any other expansion makes its word [`Word::Dynamic`].
///
/// # Errors
///
/// Returns an error for unterminated quotes, substitutions, subshells or
/// here-documents, a missing redirection target, or nesting past
/// [`MAX_DEPTH`].
pub(super) fn parse(script: &str) -> Result<Vec<Item>, ParseError> {
    let mut parser = Parser {
        chars: script.chars().collect(),
        pos: 0,
    };
    let mut items = Vec::new();
    parser.list(false, 0, &mut items)?;
    Ok(items)
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

/// A here-document whose body starts after the current line.
struct Pending {
    delimiter: String,
    strip_tabs: bool,
    quoted: bool,
    /// The index in the item list of the command it feeds, once known.
    target: Option<usize>,
}

#[derive(Default)]
struct Current {
    words: Vec<Word>,
    stdin: Option<Stdin>,
    piped: bool,
    heredocs: Vec<usize>,
}

impl Current {
    fn finish(&mut self, items: &mut Vec<Item>, pending: &mut [Pending], piped_next: bool) {
        let current = std::mem::take(self);
        self.piped = piped_next;
        if current.words.is_empty() {
            return;
        }
        let stdin = current.stdin.unwrap_or(if current.piped {
            Stdin::Pipe
        } else {
            Stdin::Terminal
        });
        items.push(Item::Command(SimpleCommand {
            words: current.words,
            stdin,
        }));
        let index = items.len() - 1;
        for heredoc in current.heredocs {
            if let Some(pending) = pending.get_mut(heredoc) {
                pending.target = Some(index);
            }
        }
    }
}

fn ends_word(ch: char) -> bool {
    matches!(
        ch,
        ' ' | '\t' | '\n' | ';' | '&' | '|' | '(' | ')' | '<' | '>'
    )
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<char> {
        self.chars.get(self.pos + offset).copied()
    }

    fn next(&mut self) -> Option<char> {
        let ch = self.peek();
        if ch.is_some() {
            self.pos += 1;
        }
        ch
    }

    fn skip_blanks(&mut self) {
        loop {
            match (self.peek(), self.peek_at(1)) {
                (Some(' ' | '\t'), _) => self.pos += 1,
                (Some('\\'), Some('\n')) => self.pos += 2,
                _ => return,
            }
        }
    }

    /// Parses commands up to the end of input, or up to `)` when `close`.
    fn list(&mut self, close: bool, depth: usize, items: &mut Vec<Item>) -> Result<(), ParseError> {
        if depth > MAX_DEPTH {
            return Err(ParseError("nesting too deep"));
        }
        let mut pending = Vec::new();
        let mut current = Current::default();
        loop {
            self.skip_blanks();
            let Some(ch) = self.peek() else {
                if close {
                    return Err(ParseError("unterminated subshell or substitution"));
                }
                current.finish(items, &mut pending, false);
                return finished(&pending);
            };
            match ch {
                ')' => {
                    if !close {
                        return Err(ParseError("unmatched )"));
                    }
                    self.pos += 1;
                    current.finish(items, &mut pending, false);
                    return finished(&pending);
                }
                '\n' => {
                    self.pos += 1;
                    current.finish(items, &mut pending, false);
                    self.read_heredocs(&mut pending, items)?;
                }
                '&' if self.peek_at(1) == Some('>') => {
                    self.redirection(&mut current, &mut pending, items, depth)?;
                }
                ';' | '&' | '|' => {
                    self.pos += 1;
                    let mut piped = ch == '|';
                    match self.peek() {
                        Some('|') if ch == '|' => {
                            self.pos += 1;
                            piped = false;
                        }
                        Some('&') if ch == '|' || ch == '&' => self.pos += 1,
                        Some(';') if ch == ';' => self.pos += 1,
                        _ => (),
                    }
                    current.finish(items, &mut pending, piped);
                }
                '(' if current.words.is_empty() => {
                    self.pos += 1;
                    items.push(Item::Enter);
                    self.list(true, depth + 1, items)?;
                    items.push(Item::Leave);
                }
                '(' if self.peek_at(1) == Some(')') => {
                    // A function definition: its name is not a command, and
                    // its body is read as ordinary commands.
                    self.pos += 2;
                    current = Current::default();
                }
                '(' => return Err(ParseError("unexpected (")),
                '#' => {
                    while self.peek().is_some_and(|ch| ch != '\n') {
                        self.pos += 1;
                    }
                }
                '<' | '>' if self.peek_at(1) == Some('(') => {
                    self.process_substitution(items, depth)?;
                    current.words.push(Word::Dynamic(String::new()));
                }
                '<' | '>' => self.redirection(&mut current, &mut pending, items, depth)?,
                _ => {
                    let word = self.word(items, depth)?;
                    let descriptor = word.literal().is_some_and(|text| {
                        !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit())
                    });
                    // `2>file`: the digits name a descriptor, not an argument.
                    if !(descriptor && matches!(self.peek(), Some('<' | '>'))) {
                        current.words.push(word);
                    }
                }
            }
        }
    }

    fn process_substitution(
        &mut self,
        items: &mut Vec<Item>,
        depth: usize,
    ) -> Result<(), ParseError> {
        self.pos += 2;
        items.push(Item::Enter);
        self.list(true, depth + 1, items)?;
        items.push(Item::Leave);
        Ok(())
    }

    fn redirection(
        &mut self,
        current: &mut Current,
        pending: &mut Vec<Pending>,
        items: &mut Vec<Item>,
        depth: usize,
    ) -> Result<(), ParseError> {
        let operator: String = self.chars.iter().skip(self.pos).take(3).collect();
        if operator.starts_with("<<<") {
            self.pos += 3;
            let text = self.target(items, depth)?;
            current.stdin = Some(Stdin::Text(text.literal().map(|text| format!("{text}\n"))));
        } else if operator.starts_with("<<") {
            self.pos += 2;
            let strip_tabs = self.peek() == Some('-');
            if strip_tabs {
                self.pos += 1;
            }
            self.skip_blanks();
            let (delimiter, quoted) = self.delimiter()?;
            pending.push(Pending {
                delimiter,
                strip_tabs,
                quoted,
                target: None,
            });
            current.heredocs.push(pending.len() - 1);
        } else if operator.starts_with("<&") || operator.starts_with("<>") {
            self.pos += 2;
            self.target(items, depth)?;
        } else if operator.starts_with('<') {
            self.pos += 1;
            let file = self.target(items, depth)?;
            current.stdin = Some(Stdin::File(file.literal().map(str::to_owned)));
        } else {
            // `>`, `>>`, `>|`, `>&`, `&>` and `&>>` all name an output.
            self.pos += 1;
            if matches!(self.peek(), Some('>' | '|' | '&')) {
                self.pos += 1;
            }
            if operator.starts_with("&>>") {
                self.pos += 1;
            }
            self.target(items, depth)?;
        }
        Ok(())
    }

    fn target(&mut self, items: &mut Vec<Item>, depth: usize) -> Result<Word, ParseError> {
        self.skip_blanks();
        match (self.peek(), self.peek_at(1)) {
            (Some('<' | '>'), Some('(')) => {
                self.process_substitution(items, depth)?;
                Ok(Word::Dynamic(String::new()))
            }
            (Some(ch), _) if !ends_word(ch) => self.word(items, depth),
            _ => Err(ParseError("missing redirection target")),
        }
    }

    /// Reads a here-document delimiter after quote removal, and whether any
    /// part of it was quoted.
    fn delimiter(&mut self) -> Result<(String, bool), ParseError> {
        let mut text = String::new();
        let mut quoted = false;
        while let Some(ch) = self.peek().filter(|ch| !ends_word(*ch)) {
            self.pos += 1;
            match ch {
                '\'' | '"' => {
                    quoted = true;
                    loop {
                        match self.next() {
                            Some(end) if end == ch => break,
                            Some(inner) => text.push(inner),
                            None => return Err(ParseError("unterminated quote")),
                        }
                    }
                }
                '\\' => {
                    quoted = true;
                    text.push(self.next().ok_or(ParseError("trailing backslash"))?);
                }
                _ => text.push(ch),
            }
        }
        if text.is_empty() {
            return Err(ParseError("missing here-document delimiter"));
        }
        Ok((text, quoted))
    }

    fn read_heredocs(
        &mut self,
        pending: &mut Vec<Pending>,
        items: &mut [Item],
    ) -> Result<(), ParseError> {
        for heredoc in pending.drain(..) {
            let mut body = String::new();
            loop {
                if self.peek().is_none() {
                    return Err(ParseError("unterminated here-document"));
                }
                let mut line = String::new();
                while let Some(ch) = self.next() {
                    if ch == '\n' {
                        break;
                    }
                    line.push(ch);
                }
                let line = if heredoc.strip_tabs {
                    line.trim_start_matches('\t')
                } else {
                    &line
                };
                if line == heredoc.delimiter {
                    break;
                }
                body.push_str(line);
                body.push('\n');
            }
            let text = if heredoc.quoted {
                Some(body)
            } else {
                unescape_heredoc(&body)
            };
            if let Some(Item::Command(command)) =
                heredoc.target.and_then(|index| items.get_mut(index))
            {
                command.stdin = Stdin::Text(text);
            }
        }
        Ok(())
    }

    fn word(&mut self, items: &mut Vec<Item>, depth: usize) -> Result<Word, ParseError> {
        let start = self.pos;
        let mut text = String::new();
        let mut dynamic = false;
        let mut prefix = None;
        while let Some(ch) = self.peek().filter(|ch| !ends_word(*ch)) {
            if dynamic && prefix.is_none() {
                prefix = Some(text.clone());
            }
            match ch {
                '\\' => {
                    self.pos += 1;
                    match self.next() {
                        Some('\n') => (),
                        Some(escaped) => text.push(escaped),
                        None => return Err(ParseError("trailing backslash")),
                    }
                }
                '\'' => {
                    self.pos += 1;
                    loop {
                        match self.next() {
                            Some('\'') => break,
                            Some(inner) => text.push(inner),
                            None => return Err(ParseError("unterminated quote")),
                        }
                    }
                }
                '"' => {
                    self.pos += 1;
                    self.double_quoted(&mut text, &mut dynamic, items, depth)?;
                }
                '$' => self.dollar(&mut text, &mut dynamic, false, items, depth)?,
                '`' => {
                    self.backtick(items, depth)?;
                    dynamic = true;
                }
                '~' if self.pos == start => {
                    self.pos += 1;
                    dynamic = true;
                }
                _ => {
                    self.pos += 1;
                    text.push(ch);
                }
            }
        }
        Ok(if dynamic {
            Word::Dynamic(prefix.unwrap_or(text))
        } else {
            Word::Literal(text)
        })
    }

    fn double_quoted(
        &mut self,
        text: &mut String,
        dynamic: &mut bool,
        items: &mut Vec<Item>,
        depth: usize,
    ) -> Result<(), ParseError> {
        loop {
            match self.peek() {
                None => return Err(ParseError("unterminated quote")),
                Some('"') => {
                    self.pos += 1;
                    return Ok(());
                }
                Some('\\') => {
                    self.pos += 1;
                    match self.next() {
                        Some(escaped @ ('$' | '`' | '"' | '\\')) => text.push(escaped),
                        Some('\n') => (),
                        Some(other) => {
                            text.push('\\');
                            text.push(other);
                        }
                        None => return Err(ParseError("unterminated quote")),
                    }
                }
                Some('$') => self.dollar(text, dynamic, true, items, depth)?,
                Some('`') => {
                    self.backtick(items, depth)?;
                    *dynamic = true;
                }
                Some(ch) => {
                    self.pos += 1;
                    text.push(ch);
                }
            }
        }
    }

    fn dollar(
        &mut self,
        text: &mut String,
        dynamic: &mut bool,
        in_quotes: bool,
        items: &mut Vec<Item>,
        depth: usize,
    ) -> Result<(), ParseError> {
        match self.peek_at(1) {
            Some('\'') if !in_quotes => {
                self.pos += 2;
                match self.ansi_c()? {
                    Some(decoded) => text.push_str(&decoded),
                    None => *dynamic = true,
                }
            }
            Some('"') if !in_quotes => {
                self.pos += 2;
                self.double_quoted(text, dynamic, items, depth)?;
            }
            Some('(') if self.peek_at(2) == Some('(') => {
                self.pos += 3;
                let mut open = 2_usize;
                while open > 0 {
                    match self.next() {
                        Some('(') => open += 1,
                        Some(')') => open -= 1,
                        Some(_) => (),
                        None => return Err(ParseError("unterminated arithmetic expansion")),
                    }
                }
                *dynamic = true;
            }
            Some('(') => {
                self.pos += 2;
                let mut inner = Vec::new();
                self.list(true, depth + 1, &mut inner)?;
                match here_document_text(&inner) {
                    Some(value) => text.push_str(&value),
                    None => *dynamic = true,
                }
                items.push(Item::Enter);
                items.extend(inner);
                items.push(Item::Leave);
            }
            Some('{') => {
                self.pos += 2;
                let mut open = 1_usize;
                while open > 0 {
                    match self.next() {
                        Some('{') => open += 1,
                        Some('}') => open -= 1,
                        Some(_) => (),
                        None => return Err(ParseError("unterminated parameter expansion")),
                    }
                }
                *dynamic = true;
            }
            Some(ch) if ch.is_ascii_alphabetic() || ch == '_' => {
                self.pos += 2;
                while self
                    .peek()
                    .is_some_and(|ch| ch.is_ascii_alphanumeric() || ch == '_')
                {
                    self.pos += 1;
                }
                *dynamic = true;
            }
            Some('@' | '*' | '#' | '?' | '$' | '!' | '-' | '0'..='9') => {
                self.pos += 2;
                *dynamic = true;
            }
            _ => {
                self.pos += 1;
                text.push('$');
            }
        }
        Ok(())
    }

    /// Decodes `$'…'`. Numeric and control escapes, which could spell any
    /// text, give `None`.
    fn ansi_c(&mut self) -> Result<Option<String>, ParseError> {
        let mut text = String::new();
        let mut known = true;
        loop {
            match self.next() {
                None => return Err(ParseError("unterminated quote")),
                Some('\'') => return Ok(known.then_some(text)),
                Some('\\') => match self.next() {
                    None => return Err(ParseError("unterminated quote")),
                    Some('n') => text.push('\n'),
                    Some('t') => text.push('\t'),
                    Some('r') => text.push('\r'),
                    Some(escaped @ ('\\' | '\'' | '"' | '?')) => text.push(escaped),
                    Some(_) => known = false,
                },
                Some(ch) => text.push(ch),
            }
        }
    }

    fn backtick(&mut self, items: &mut Vec<Item>, depth: usize) -> Result<(), ParseError> {
        self.pos += 1;
        let mut script = String::new();
        loop {
            match self.next() {
                None => return Err(ParseError("unterminated command substitution")),
                Some('`') => break,
                Some('\\') => match self.next() {
                    Some(escaped @ ('`' | '$' | '\\')) => script.push(escaped),
                    Some(other) => {
                        script.push('\\');
                        script.push(other);
                    }
                    None => return Err(ParseError("unterminated command substitution")),
                },
                Some(ch) => script.push(ch),
            }
        }
        let mut inner = Parser {
            chars: script.chars().collect(),
            pos: 0,
        };
        items.push(Item::Enter);
        inner.list(false, depth + 1, items)?;
        items.push(Item::Leave);
        Ok(())
    }
}

fn finished(pending: &[Pending]) -> Result<(), ParseError> {
    if pending.is_empty() {
        Ok(())
    } else {
        Err(ParseError("unterminated here-document"))
    }
}

/// The text of `$(cat <<EOF … EOF)`: the here-document without its final
/// newlines, as command substitution removes them.
fn here_document_text(inner: &[Item]) -> Option<String> {
    match inner {
        [
            Item::Command(SimpleCommand {
                words,
                stdin: Stdin::Text(Some(body)),
            }),
        ] if words.as_slice() == [Word::Literal("cat".to_owned())] => {
            Some(body.trim_end_matches('\n').to_owned())
        }
        _ => None,
    }
}

/// Applies the escapes of an unquoted here-document, or gives `None` if it
/// expands anything at run time.
fn unescape_heredoc(body: &str) -> Option<String> {
    let mut text = String::new();
    let mut chars = body.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '\\' => match chars.peek() {
                Some(&escaped @ ('$' | '`' | '\\')) => {
                    chars.next();
                    text.push(escaped);
                }
                Some('\n') => {
                    chars.next();
                }
                _ => text.push('\\'),
            },
            '`' => return None,
            '$' => match chars.peek() {
                Some(&next) if next.is_ascii_alphanumeric() || "_({@*#?$!-".contains(next) => {
                    return None;
                }
                _ => text.push('$'),
            },
            _ => text.push(ch),
        }
    }
    Some(text)
}
