// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! A small JSON reader for the output of `cargo metadata`.

use std::fmt;

#[cfg(test)]
mod tests;

/// Deepest nesting of arrays and objects accepted.
const MAX_DEPTH: usize = 64;

/// A parsed JSON value. Numbers keep their text: no caller needs arithmetic.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Value {
    Null,
    Bool(bool),
    Number(String),
    String(String),
    Array(Vec<Value>),
    Object(Vec<(String, Value)>),
}

/// Where and why the input is not JSON.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ParseError {
    offset: usize,
    reason: &'static str,
}

impl fmt::Display for ParseError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} at byte {}", self.reason, self.offset)
    }
}

impl Value {
    /// The member named `key`, if this is an object that has one.
    pub(crate) fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Self::Object(members) => members
                .iter()
                .find_map(|(name, value)| (name == key).then_some(value)),
            _ => None,
        }
    }

    pub(crate) fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(text) => Some(text),
            _ => None,
        }
    }

    pub(crate) fn as_array(&self) -> Option<&[Value]> {
        match self {
            Self::Array(items) => Some(items),
            _ => None,
        }
    }
}

/// Parses one JSON document.
///
/// # Errors
///
/// Returns the offset and reason of the first thing that is not JSON.
pub(crate) fn parse(text: &str) -> Result<Value, ParseError> {
    let mut parser = Parser {
        bytes: text.as_bytes(),
        offset: 0,
    };
    let value = parser.value(0)?;
    parser.skip_whitespace();
    if parser.peek().is_some() {
        return Err(parser.error("trailing characters"));
    }
    Ok(value)
}

struct Parser<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl Parser<'_> {
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.offset).copied()
    }

    fn error(&self, reason: &'static str) -> ParseError {
        ParseError {
            offset: self.offset,
            reason,
        }
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.offset += 1;
        }
    }

    fn expect(&mut self, byte: u8, reason: &'static str) -> Result<(), ParseError> {
        self.skip_whitespace();
        if self.peek() == Some(byte) {
            self.offset += 1;
            Ok(())
        } else {
            Err(self.error(reason))
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value, ParseError> {
        self.skip_whitespace();
        match self.peek() {
            Some(b'{' | b'[') if depth >= MAX_DEPTH => Err(self.error("nesting too deep")),
            Some(b'{') => self.object(depth + 1),
            Some(b'[') => self.array(depth + 1),
            Some(b'"') => self.string().map(Value::String),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => self.literal(),
        }
    }

    fn literal(&mut self) -> Result<Value, ParseError> {
        for (word, value) in [
            (&b"null"[..], Value::Null),
            (b"true", Value::Bool(true)),
            (b"false", Value::Bool(false)),
        ] {
            if self
                .bytes
                .get(self.offset..)
                .is_some_and(|rest| rest.starts_with(word))
            {
                self.offset += word.len();
                return Ok(value);
            }
        }
        Err(self.error("expected a value"))
    }

    fn object(&mut self, depth: usize) -> Result<Value, ParseError> {
        self.offset += 1;
        let mut members = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(b'}') {
            self.offset += 1;
            return Ok(Value::Object(members));
        }
        loop {
            self.skip_whitespace();
            if self.peek() != Some(b'"') {
                return Err(self.error("expected a string"));
            }
            let name = self.string()?;
            self.expect(b':', "expected ':'")?;
            members.push((name, self.value(depth)?));
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => self.offset += 1,
                Some(b'}') => {
                    self.offset += 1;
                    return Ok(Value::Object(members));
                }
                _ => return Err(self.error("expected ',' or '}'")),
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<Value, ParseError> {
        self.offset += 1;
        let mut items = Vec::new();
        self.skip_whitespace();
        if self.peek() == Some(b']') {
            self.offset += 1;
            return Ok(Value::Array(items));
        }
        loop {
            items.push(self.value(depth)?);
            self.skip_whitespace();
            match self.peek() {
                Some(b',') => self.offset += 1,
                Some(b']') => {
                    self.offset += 1;
                    return Ok(Value::Array(items));
                }
                _ => return Err(self.error("expected ',' or ']'")),
            }
        }
    }

    fn digits(&mut self) -> Result<(), ParseError> {
        if !matches!(self.peek(), Some(b'0'..=b'9')) {
            return Err(self.error("expected a digit"));
        }
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.offset += 1;
        }
        Ok(())
    }

    fn number(&mut self) -> Result<Value, ParseError> {
        let start = self.offset;
        if self.peek() == Some(b'-') {
            self.offset += 1;
        }
        if self.peek() == Some(b'0') {
            self.offset += 1;
        } else {
            self.digits()?;
        }
        if self.peek() == Some(b'.') {
            self.offset += 1;
            self.digits()?;
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.offset += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.offset += 1;
            }
            self.digits()?;
        }
        let text = self
            .bytes
            .get(start..self.offset)
            .and_then(|bytes| std::str::from_utf8(bytes).ok())
            .ok_or_else(|| self.error("expected a digit"))?;
        Ok(Value::Number(text.to_owned()))
    }

    fn hex4(&mut self) -> Result<u32, ParseError> {
        let mut code = 0;
        for _ in 0..4 {
            let digit = self
                .peek()
                .and_then(|byte| char::from(byte).to_digit(16))
                .ok_or_else(|| self.error("invalid escape"))?;
            self.offset += 1;
            code = code * 16 + digit;
        }
        Ok(code)
    }

    fn escape(&mut self, text: &mut String) -> Result<(), ParseError> {
        let decoded = match self.peek() {
            Some(b'"') => '"',
            Some(b'\\') => '\\',
            Some(b'/') => '/',
            Some(b'b') => '\u{8}',
            Some(b'f') => '\u{c}',
            Some(b'n') => '\n',
            Some(b'r') => '\r',
            Some(b't') => '\t',
            Some(b'u') => {
                self.offset += 1;
                let high = self.hex4()?;
                let code = if (0xd800..0xdc00).contains(&high) {
                    if self.bytes.get(self.offset..self.offset + 2) != Some(b"\\u") {
                        return Err(self.error("unpaired surrogate"));
                    }
                    self.offset += 2;
                    let low = self.hex4()?;
                    if !(0xdc00..0xe000).contains(&low) {
                        return Err(self.error("unpaired surrogate"));
                    }
                    0x1_0000 + ((high - 0xd800) << 10) + (low - 0xdc00)
                } else {
                    high
                };
                let decoded =
                    char::from_u32(code).ok_or_else(|| self.error("unpaired surrogate"))?;
                text.push(decoded);
                return Ok(());
            }
            _ => return Err(self.error("invalid escape")),
        };
        self.offset += 1;
        text.push(decoded);
        Ok(())
    }

    fn string(&mut self) -> Result<String, ParseError> {
        self.offset += 1;
        let mut text = String::new();
        loop {
            let run_start = self.offset;
            while matches!(self.peek(), Some(byte) if byte != b'"' && byte != b'\\' && byte >= 0x20)
            {
                self.offset += 1;
            }
            // The input is a `str` and the run stops only at ASCII bytes, so
            // the run is whole characters.
            let run = self
                .bytes
                .get(run_start..self.offset)
                .and_then(|bytes| std::str::from_utf8(bytes).ok())
                .ok_or_else(|| self.error("invalid UTF-8"))?;
            text.push_str(run);
            match self.peek() {
                Some(b'"') => {
                    self.offset += 1;
                    return Ok(text);
                }
                Some(b'\\') => {
                    self.offset += 1;
                    self.escape(&mut text)?;
                }
                Some(_) => return Err(self.error("control character in string")),
                None => return Err(self.error("unterminated string")),
            }
        }
    }
}
