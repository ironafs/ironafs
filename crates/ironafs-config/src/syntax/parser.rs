// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! The grammar of the subset (`docs/specs/config-syntax.md`, sections 5.1,
//! 5.2, 5.4 and 5.5): the whole-file checks, then the lines in order.
//!
//! The parser reads one character at a time through `Cursor` and never
//! recurses: an array holds only scalars, so it is read in one loop. Every
//! refused TOML construct is recognised at its first character and
//! refused there, with its own error.

use alloc::string::String;
use alloc::vec::Vec;
use core::mem;

use super::document::{Builder, Segment};
use super::{
    Document, Element, Expected, Feature, MAX_ARRAY_ELEMENTS, MAX_DEPTH, MAX_FILE_BYTES,
    MAX_KEY_BYTES, MAX_LINE_BYTES, MAX_STRING_BYTES, Position, Scalar, SyntaxError,
    SyntaxErrorKind, Value,
};

const BYTE_ORDER_MARK: &[u8] = b"\xEF\xBB\xBF";

/// Parses one file: the whole-file checks, then the lines.
pub(super) fn parse(input: &[u8]) -> Result<Document, SyntaxError> {
    let text = check_file(input)?;
    Parser {
        cursor: Cursor::new(text),
        builder: Builder::new(),
    }
    .document()
}

/// The checks of section 5.4, point 1 that come before any line is read:
/// size, byte order mark, UTF-8, then line ends, control characters and
/// line length, from the start of the file.
fn check_file(input: &[u8]) -> Result<&str, SyntaxError> {
    if input.len() > MAX_FILE_BYTES {
        return Err(SyntaxError::file_too_large());
    }
    if input.starts_with(BYTE_ORDER_MARK) {
        return Err(SyntaxError::at(
            Position { line: 1, column: 1 },
            SyntaxErrorKind::Unsupported(Feature::ByteOrderMark),
        ));
    }
    let text = match core::str::from_utf8(input) {
        Ok(text) => text,
        Err(error) => {
            let valid = input
                .get(..error.valid_up_to())
                .and_then(|prefix| core::str::from_utf8(prefix).ok())
                .unwrap_or_default();
            return Err(SyntaxError::at(
                position_after(valid),
                SyntaxErrorKind::InvalidUtf8,
            ));
        }
    };
    check_lines(text)?;
    Ok(text)
}

/// The position just after `prefix`.
fn position_after(prefix: &str) -> Position {
    let mut position = Position { line: 1, column: 1 };
    for character in prefix.chars() {
        advance(&mut position, character);
    }
    position
}

/// Moves `position` past `character`. Lines and columns are bounded by the
/// file's size, which is capped, so the counts cannot overflow.
fn advance(position: &mut Position, character: char) {
    if character == '\n' {
        position.line += 1;
        position.column = 1;
    } else {
        position.column += 1;
    }
}

/// Refuses a carriage return without a line feed, a control character
/// other than tab, a bidirectional control character, and a line longer than [`MAX_LINE_BYTES`] without its
/// line end, whichever comes first.
fn check_lines(text: &str) -> Result<(), SyntaxError> {
    let mut position = Position { line: 1, column: 1 };
    let mut line_bytes: usize = 0;
    let mut characters = text.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '\n' => line_bytes = 0,
            '\r' => {
                if characters.peek() != Some(&'\n') {
                    return Err(SyntaxError::at(
                        position,
                        SyntaxErrorKind::BareCarriageReturn,
                    ));
                }
            }
            '\t' => line_bytes += 1,
            other if is_bidi_control(other) => {
                return Err(SyntaxError::at(
                    position,
                    SyntaxErrorKind::BidiControl(other),
                ));
            }
            other if other.is_control() => {
                return Err(SyntaxError::at(
                    position,
                    SyntaxErrorKind::ControlCharacter(other),
                ));
            }
            other => line_bytes += other.len_utf8(),
        }
        if line_bytes > MAX_LINE_BYTES {
            return Err(SyntaxError::at(
                Position {
                    line: position.line,
                    column: 1,
                },
                SyntaxErrorKind::LineTooLong,
            ));
        }
        advance(&mut position, character);
    }
    Ok(())
}

/// Unicode's `Bidi_Control` set (section 5.1, point 3): characters that
/// change the order text is displayed in, so a file could read
/// differently on screen than it parses.
fn is_bidi_control(character: char) -> bool {
    matches!(
        character,
        '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'
    )
}

/// A bare key's characters (section 5.1, point 8).
fn is_bare(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_' || character == '-'
}

/// The characters of a token that is not a string or an array: wide
/// enough to take in every float, date and integer form TOML has, so each
/// is recognised whole and refused by name.
fn is_token(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '+' | '.' | ':')
}

/// A position in the checked text.
struct Cursor<'a> {
    text: &'a str,
    offset: usize,
    position: Position,
}

impl<'a> Cursor<'a> {
    fn new(text: &'a str) -> Self {
        Self {
            text,
            offset: 0,
            position: Position { line: 1, column: 1 },
        }
    }

    fn rest(&self) -> &'a str {
        self.text.get(self.offset..).unwrap_or_default()
    }

    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn bump(&mut self) {
        if let Some(character) = self.peek() {
            self.offset += character.len_utf8();
            advance(&mut self.position, character);
        }
    }

    fn position(&self) -> Position {
        self.position
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t')) {
            self.bump();
        }
    }

    /// Skips a comment up to its line end, which it leaves.
    fn skip_comment(&mut self) {
        while self
            .peek()
            .is_some_and(|character| !matches!(character, '\n' | '\r'))
        {
            self.bump();
        }
    }

    /// An error for what is here, where `expected` was. A line end is
    /// reported as `\n` whether it is LF or CR LF.
    fn unexpected(&self, expected: Expected) -> SyntaxError {
        let found = self.peek().map(|character| match character {
            '\r' => '\n',
            other => other,
        });
        SyntaxError::at(
            self.position,
            SyntaxErrorKind::Unexpected { found, expected },
        )
    }
}

struct Parser<'a> {
    cursor: Cursor<'a>,
    builder: Builder,
}

impl Parser<'_> {
    fn document(mut self) -> Result<Document, SyntaxError> {
        let mut table = Builder::root();
        loop {
            self.cursor.skip_whitespace();
            match self.cursor.peek() {
                None => break,
                Some('\n' | '\r' | '#') => {}
                Some('[') => table = self.header()?,
                Some(character) if is_bare(character) || matches!(character, '"' | '\'') => {
                    self.key_value(table)?;
                }
                Some(_) => return Err(self.cursor.unexpected(Expected::LineStart)),
            }
            self.end_of_line()?;
        }
        Ok(self.builder.finish())
    }

    /// Whitespace, an optional comment, then a line end or the end of the
    /// file.
    fn end_of_line(&mut self) -> Result<(), SyntaxError> {
        self.cursor.skip_whitespace();
        if self.cursor.peek() == Some('#') {
            self.cursor.skip_comment();
        }
        match self.cursor.peek() {
            None => Ok(()),
            Some('\r') => {
                // The whole-file checks put LF after every CR.
                self.cursor.bump();
                self.cursor.bump();
                Ok(())
            }
            Some('\n') => {
                self.cursor.bump();
                Ok(())
            }
            Some(_) => Err(self.cursor.unexpected(Expected::EndOfLine)),
        }
    }

    /// A table header (section 5.1, point 6); returns its table.
    fn header(&mut self) -> Result<usize, SyntaxError> {
        let start = self.cursor.position();
        self.cursor.bump();
        if self.cursor.peek() == Some('[') {
            return Err(SyntaxError::at(
                start,
                SyntaxErrorKind::Unsupported(Feature::ArrayOfTables),
            ));
        }
        let mut segments = Vec::new();
        loop {
            self.cursor.skip_whitespace();
            let position = self.cursor.position();
            // A segment past the cap is refused before it is decoded. What
            // cannot start a segment is left to `segment`, which names
            // what was expected.
            if segments.len() >= MAX_DEPTH
                && self
                    .cursor
                    .peek()
                    .is_some_and(|character| is_bare(character) || character == '"')
            {
                return Err(SyntaxError::at(position, SyntaxErrorKind::TooDeep));
            }
            let text = self.segment()?;
            segments.push(Segment { text, position });
            self.cursor.skip_whitespace();
            match self.cursor.peek() {
                Some('.') => self.cursor.bump(),
                Some(']') => {
                    self.cursor.bump();
                    break;
                }
                _ => return Err(self.cursor.unexpected(Expected::DotOrCloseBracket)),
            }
        }
        self.builder.open_table(segments, start)
    }

    /// A key/value pair (section 5.1, point 7) in `table`.
    fn key_value(&mut self, table: usize) -> Result<(), SyntaxError> {
        let key_position = self.cursor.position();
        let segment = self.segment()?;
        let key = self.builder.check_key(table, segment, key_position)?;
        self.cursor.skip_whitespace();
        match self.cursor.peek() {
            Some('.') => {
                return Err(SyntaxError::at(
                    self.cursor.position(),
                    SyntaxErrorKind::Unsupported(Feature::DottedKey),
                ));
            }
            Some('=') => self.cursor.bump(),
            _ => return Err(self.cursor.unexpected(Expected::Equals)),
        }
        self.cursor.skip_whitespace();
        let value_position = self.cursor.position();
        let value = if self.cursor.peek() == Some('[') {
            self.array()?
        } else {
            Value::Scalar(self.scalar(Expected::Value)?)
        };
        self.builder.add_entry(key, value, value_position)
    }

    /// A bare or quoted key, or a table-path segment (section 5.1, point
    /// 8).
    fn segment(&mut self) -> Result<String, SyntaxError> {
        let start = self.cursor.position();
        match self.cursor.peek() {
            Some('"') => {
                let text = self.basic_string(MAX_KEY_BYTES, SyntaxErrorKind::KeyTooLong)?;
                if text.is_empty() {
                    return Err(SyntaxError::at(
                        start,
                        SyntaxErrorKind::Unsupported(Feature::EmptyKey),
                    ));
                }
                Ok(text)
            }
            Some('\'') => Err(SyntaxError::at(
                start,
                SyntaxErrorKind::Unsupported(Feature::LiteralString),
            )),
            Some(character) if is_bare(character) => {
                let mut text = String::new();
                while let Some(character) = self.cursor.peek().filter(|c| is_bare(*c)) {
                    if text.len() >= MAX_KEY_BYTES {
                        return Err(SyntaxError::at(start, SyntaxErrorKind::KeyTooLong));
                    }
                    text.push(character);
                    self.cursor.bump();
                }
                Ok(text)
            }
            _ => Err(self.cursor.unexpected(Expected::Key)),
        }
    }

    /// A string, an integer or a boolean. `expected` names what an empty
    /// token was expected to be.
    fn scalar(&mut self, expected: Expected) -> Result<Scalar, SyntaxError> {
        let start = self.cursor.position();
        match self.cursor.peek() {
            Some('"') => Ok(Scalar::String(
                self.basic_string(MAX_STRING_BYTES, SyntaxErrorKind::StringTooLong)?,
            )),
            Some('\'') => Err(SyntaxError::at(
                start,
                SyntaxErrorKind::Unsupported(Feature::LiteralString),
            )),
            Some('{') => Err(SyntaxError::at(
                start,
                SyntaxErrorKind::Unsupported(Feature::InlineTable),
            )),
            _ => {
                let begin = self.cursor.offset;
                while self.cursor.peek().is_some_and(is_token) {
                    self.cursor.bump();
                }
                let token = self
                    .cursor
                    .text
                    .get(begin..self.cursor.offset)
                    .unwrap_or_default();
                if token.is_empty() {
                    return Err(self.cursor.unexpected(expected));
                }
                classify(token).map_err(|kind| SyntaxError::at(start, kind))
            }
        }
    }

    /// An array of scalars of one type (section 5.1, point 12).
    fn array(&mut self) -> Result<Value, SyntaxError> {
        self.cursor.bump();
        let mut elements: Vec<Element> = Vec::new();
        loop {
            self.skip_array_space();
            if self.cursor.peek() == Some(']') {
                self.cursor.bump();
                return Ok(Value::Array(elements));
            }
            let position = self.cursor.position();
            if self.cursor.peek() == Some('[') {
                return Err(SyntaxError::at(
                    position,
                    SyntaxErrorKind::Unsupported(Feature::NestedArray),
                ));
            }
            if elements.len() >= MAX_ARRAY_ELEMENTS {
                return Err(SyntaxError::at(position, SyntaxErrorKind::TooManyElements));
            }
            let value = self.scalar(Expected::ValueOrCloseBracket)?;
            if elements
                .first()
                .is_some_and(|first| mem::discriminant(&first.value) != mem::discriminant(&value))
            {
                return Err(SyntaxError::at(
                    position,
                    SyntaxErrorKind::Unsupported(Feature::MixedArray),
                ));
            }
            elements.push(Element { value, position });
            self.skip_array_space();
            match self.cursor.peek() {
                Some(',') => self.cursor.bump(),
                Some(']') => {
                    self.cursor.bump();
                    return Ok(Value::Array(elements));
                }
                _ => return Err(self.cursor.unexpected(Expected::CommaOrCloseBracket)),
            }
        }
    }

    /// Whitespace, line ends and comments inside an array.
    fn skip_array_space(&mut self) {
        loop {
            match self.cursor.peek() {
                Some(' ' | '\t' | '\n' | '\r') => self.cursor.bump(),
                Some('#') => self.cursor.skip_comment(),
                _ => return,
            }
        }
    }

    /// A basic string on one line (section 5.1, point 9), holding at most
    /// `cap` bytes after escapes; a longer one is refused as `too_long`.
    fn basic_string(
        &mut self,
        cap: usize,
        too_long: SyntaxErrorKind,
    ) -> Result<String, SyntaxError> {
        let start = self.cursor.position();
        if self.cursor.rest().starts_with("\"\"\"") {
            return Err(SyntaxError::at(
                start,
                SyntaxErrorKind::Unsupported(Feature::MultiLineString),
            ));
        }
        self.cursor.bump();
        let unterminated = SyntaxError::at(start, SyntaxErrorKind::UnterminatedString);
        let mut text = String::new();
        loop {
            let here = self.cursor.position();
            let character = match self.cursor.peek() {
                None | Some('\n' | '\r') => return Err(unterminated),
                Some('"') => {
                    self.cursor.bump();
                    return Ok(text);
                }
                Some('\t') => {
                    return Err(SyntaxError::at(
                        here,
                        SyntaxErrorKind::ControlCharacter('\t'),
                    ));
                }
                Some('\\') => {
                    self.cursor.bump();
                    match self.cursor.peek() {
                        Some(escaped @ ('"' | '\\')) => escaped,
                        None | Some('\n' | '\r') => return Err(unterminated),
                        Some(other) => {
                            return Err(SyntaxError::at(
                                here,
                                SyntaxErrorKind::Unsupported(Feature::Escape(other)),
                            ));
                        }
                    }
                }
                Some(other) => other,
            };
            let length = text
                .len()
                .checked_add(character.len_utf8())
                .filter(|length| *length <= cap);
            if length.is_none() {
                return Err(SyntaxError::at(start, too_long));
            }
            text.push(character);
            self.cursor.bump();
        }
    }
}

/// What a token that is not a string or an array is: a boolean, a decimal
/// integer, or a refused construct named by its form (section 5.5).
fn classify(token: &str) -> Result<Scalar, SyntaxErrorKind> {
    match token {
        "true" => return Ok(Scalar::Boolean(true)),
        "false" => return Ok(Scalar::Boolean(false)),
        _ => {}
    }
    if token.starts_with('+') {
        return Err(SyntaxErrorKind::Unsupported(Feature::PlusSign));
    }
    let (negative, digits) = match token.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, token),
    };
    if matches!(digits, "inf" | "nan") {
        return Err(SyntaxErrorKind::Unsupported(Feature::Float));
    }
    if digits.contains(':') || starts_like_date(digits) {
        return Err(SyntaxErrorKind::Unsupported(Feature::DateTime));
    }
    if ["0x", "0o", "0b"]
        .iter()
        .any(|prefix| digits.starts_with(prefix))
    {
        return Err(SyntaxErrorKind::Unsupported(Feature::NonDecimalInteger));
    }
    if digits.starts_with(|c: char| c.is_ascii_digit()) && digits.contains(['.', 'e', 'E']) {
        return Err(SyntaxErrorKind::Unsupported(Feature::Float));
    }
    decimal(negative, digits).map(Scalar::Integer)
}

/// Four digits then `-`: the start of a date.
fn starts_like_date(digits: &str) -> bool {
    let mut bytes = digits.bytes();
    (0..4).all(|_| bytes.next().is_some_and(|byte| byte.is_ascii_digit()))
        && bytes.next() == Some(b'-')
}

/// A decimal integer's digits, after its sign (section 5.1, point 10).
fn decimal(negative: bool, digits: &str) -> Result<i64, SyntaxErrorKind> {
    let mut characters = digits.chars();
    match (characters.next(), characters.next()) {
        (None, _) => return Err(SyntaxErrorKind::InvalidValue),
        (Some('_'), _) => return Err(SyntaxErrorKind::MisplacedUnderscore),
        (Some('0'), Some(next)) if next.is_ascii_digit() || next == '_' => {
            return Err(SyntaxErrorKind::LeadingZero);
        }
        (Some(first), _) if !first.is_ascii_digit() => return Err(SyntaxErrorKind::InvalidValue),
        _ => {}
    }
    let mut value: i64 = 0;
    let mut after_underscore = false;
    for character in digits.chars() {
        if character == '_' {
            if after_underscore {
                return Err(SyntaxErrorKind::MisplacedUnderscore);
            }
            after_underscore = true;
            continue;
        }
        let Some(digit) = character.to_digit(10).map(i64::from) else {
            return Err(SyntaxErrorKind::InvalidValue);
        };
        after_underscore = false;
        let shifted = value.checked_mul(10);
        value = if negative {
            shifted.and_then(|shifted| shifted.checked_sub(digit))
        } else {
            shifted.and_then(|shifted| shifted.checked_add(digit))
        }
        .ok_or(SyntaxErrorKind::IntegerOutOfRange)?;
    }
    if after_underscore {
        return Err(SyntaxErrorKind::MisplacedUnderscore);
    }
    Ok(value)
}
