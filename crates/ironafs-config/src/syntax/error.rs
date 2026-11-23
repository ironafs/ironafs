// SPDX-FileCopyrightText: 2026 Adrian Ana <adrian@adrianana.com>
// SPDX-License-Identifier: GPL-3.0-or-later

//! What a refused file gets: the kind of error, where it is, and for a
//! refused TOML construct a hint (ADR 0004, O18, point 2;
//! `docs/specs/config-syntax.md`, sections 5.5 and 7).

use core::error::Error;
use core::fmt;

use super::{
    MAX_ARRAY_ELEMENTS, MAX_DEFINITIONS, MAX_DEPTH, MAX_FILE_BYTES, MAX_KEY_BYTES, MAX_LINE_BYTES,
    MAX_STRING_BYTES, Position,
};

/// Why a file was refused, and where.
///
/// Its message is lowercase and names the line and column; the caller
/// adds the file's name.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SyntaxError {
    position: Option<Position>,
    kind: SyntaxErrorKind,
}

impl SyntaxError {
    pub(super) fn at(position: Position, kind: SyntaxErrorKind) -> Self {
        Self {
            position: Some(position),
            kind,
        }
    }

    pub(super) fn file_too_large() -> Self {
        Self {
            position: None,
            kind: SyntaxErrorKind::FileTooLarge,
        }
    }

    /// Where the error was found. `None` only for
    /// [`SyntaxErrorKind::FileTooLarge`], which is about the whole file.
    #[must_use]
    pub fn position(&self) -> Option<Position> {
        self.position
    }

    /// What is wrong.
    #[must_use]
    pub fn kind(&self) -> SyntaxErrorKind {
        self.kind
    }
}

impl fmt::Display for SyntaxError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(position) = self.position {
            write!(
                formatter,
                "line {}, column {}: ",
                position.line, position.column
            )?;
        }
        write!(formatter, "{}", self.kind)?;
        if let SyntaxErrorKind::Unsupported(feature) = self.kind {
            write!(formatter, "; {}", feature.hint())?;
        }
        Ok(())
    }
}

impl Error for SyntaxError {}

/// What is wrong with a refused file.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SyntaxErrorKind {
    /// The file is longer than [`MAX_FILE_BYTES`].
    FileTooLarge,
    /// The bytes here are not UTF-8.
    InvalidUtf8,
    /// A carriage return not followed by a line feed.
    BareCarriageReturn,
    /// A control character other than tab, or tab inside a string.
    ControlCharacter(char),
    /// A bidirectional control character (Unicode's `Bidi_Control` set),
    /// which can make text display in an order other than the order it is
    /// read in.
    BidiControl(char),
    /// A line longer than [`MAX_LINE_BYTES`].
    LineTooLong,
    /// A TOML construct the subset refuses.
    Unsupported(Feature),
    /// Something other than what the grammar allows here. `found` is
    /// `None` at the end of the file.
    Unexpected {
        /// What was found.
        found: Option<char>,
        /// What the grammar allows here.
        expected: Expected,
    },
    /// A string with no closing quote on its line.
    UnterminatedString,
    /// A key or table-path segment longer than [`MAX_KEY_BYTES`].
    KeyTooLong,
    /// A string value longer than [`MAX_STRING_BYTES`].
    StringTooLong,
    /// A table header with more than [`MAX_DEPTH`] segments.
    TooDeep,
    /// An array with more than [`MAX_ARRAY_ELEMENTS`] elements.
    TooManyElements,
    /// More than [`MAX_DEFINITIONS`] key/value pairs and tables.
    TooManyDefinitions,
    /// An integer with a leading zero.
    LeadingZero,
    /// An underscore in an integer that is not between two digits.
    MisplacedUnderscore,
    /// An integer outside the signed 64-bit range.
    IntegerOutOfRange,
    /// A value that is not a string, integer, boolean or array.
    InvalidValue,
    /// A key or table defined a second time; `first` is where it was
    /// defined.
    Redefined {
        /// Where the key or table was first defined.
        first: Position,
    },
    /// A table header whose path passes through a key that holds a value;
    /// `first` is where that key was defined.
    NotATable {
        /// Where the key that holds a value was defined.
        first: Position,
    },
}

impl fmt::Display for SyntaxErrorKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::FileTooLarge => write!(formatter, "file is larger than {MAX_FILE_BYTES} bytes"),
            Self::InvalidUtf8 => formatter.write_str("not UTF-8"),
            Self::BareCarriageReturn => formatter.write_str("carriage return without a line feed"),
            Self::ControlCharacter(character) => write!(
                formatter,
                "control character U+{:04X} is not allowed here",
                u32::from(character)
            ),
            Self::BidiControl(character) => write!(
                formatter,
                "bidirectional control character U+{:04X} is not allowed",
                u32::from(character)
            ),
            Self::LineTooLong => write!(formatter, "line is longer than {MAX_LINE_BYTES} bytes"),
            Self::Unsupported(feature) => write!(formatter, "{feature}"),
            Self::Unexpected { found, expected } => {
                formatter.write_str("found ")?;
                match found {
                    None => formatter.write_str("the end of the file")?,
                    Some('\n' | '\r') => formatter.write_str("the end of the line")?,
                    Some(character) => write!(formatter, "'{}'", character.escape_debug())?,
                }
                write!(formatter, ", expected {expected}")
            }
            Self::UnterminatedString => formatter.write_str("string has no closing quote"),
            Self::KeyTooLong => write!(formatter, "key is longer than {MAX_KEY_BYTES} bytes"),
            Self::StringTooLong => {
                write!(formatter, "string is longer than {MAX_STRING_BYTES} bytes")
            }
            Self::TooDeep => write!(formatter, "table header has more than {MAX_DEPTH} segments"),
            Self::TooManyElements => write!(
                formatter,
                "array has more than {MAX_ARRAY_ELEMENTS} elements"
            ),
            Self::TooManyDefinitions => write!(
                formatter,
                "file defines more than {MAX_DEFINITIONS} keys and tables"
            ),
            Self::LeadingZero => formatter.write_str("integer has a leading zero"),
            Self::MisplacedUnderscore => {
                formatter.write_str("underscore in an integer is not between two digits")
            }
            Self::IntegerOutOfRange => {
                formatter.write_str("integer is outside the signed 64-bit range")
            }
            Self::InvalidValue => {
                formatter.write_str("expected a string, an integer, true, false or an array")
            }
            Self::Redefined { first } => write!(
                formatter,
                "already defined at line {}, column {}",
                first.line, first.column
            ),
            Self::NotATable { first } => write!(
                formatter,
                "not a table: it holds a value set at line {}, column {}",
                first.line, first.column
            ),
        }
    }
}

/// A TOML construct the subset refuses (`docs/specs/config-syntax.md`,
/// section 5.5).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Feature {
    /// A byte order mark at the start of the file.
    ByteOrderMark,
    /// A literal string, `'...'` or `'''...'''`.
    LiteralString,
    /// A multi-line basic string, `"""..."""`.
    MultiLineString,
    /// An inline table, `{ ... }`.
    InlineTable,
    /// An array of tables, `[[...]]`.
    ArrayOfTables,
    /// A dotted key, `a.b = ...`.
    DottedKey,
    /// A float, `inf` or `nan`.
    Float,
    /// A date, a time or a date-time.
    DateTime,
    /// A hexadecimal, octal or binary integer.
    NonDecimalInteger,
    /// A `+` before a number.
    PlusSign,
    /// An escape other than `\"` and `\\`; the character after `\`.
    Escape(char),
    /// An array whose elements have different types.
    MixedArray,
    /// An array inside an array.
    NestedArray,
    /// The empty quoted key, `""`.
    EmptyKey,
}

impl Feature {
    /// What to write instead.
    #[must_use]
    pub fn hint(self) -> &'static str {
        match self {
            Self::ByteOrderMark => "save the file as UTF-8 without a byte order mark",
            Self::LiteralString => "use a double-quoted string",
            Self::MultiLineString => "write the value on one line",
            Self::InlineTable => "use a [table] header",
            Self::ArrayOfTables => "use one table per item",
            Self::DottedKey => "put the key under its table's header",
            Self::Float => "write a whole number",
            Self::DateTime => "write the value as a string",
            Self::NonDecimalInteger => "write the number in decimal",
            Self::PlusSign => "write the number without '+'",
            Self::Escape(_) => "write the character itself",
            Self::MixedArray | Self::NestedArray => "use one type of value per array",
            Self::EmptyKey => "give the key a name",
        }
    }
}

impl fmt::Display for Feature {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Self::ByteOrderMark => formatter.write_str("a byte order mark is not supported"),
            Self::LiteralString => formatter.write_str("literal strings are not supported"),
            Self::MultiLineString => formatter.write_str("multi-line strings are not supported"),
            Self::InlineTable => formatter.write_str("inline tables are not supported"),
            Self::ArrayOfTables => formatter.write_str("arrays of tables are not supported"),
            Self::DottedKey => formatter.write_str("dotted keys are not supported"),
            Self::Float => formatter.write_str("floats are not supported"),
            Self::DateTime => formatter.write_str("dates and times are not supported"),
            Self::NonDecimalInteger => {
                formatter.write_str("hexadecimal, octal and binary integers are not supported")
            }
            Self::PlusSign => formatter.write_str("a '+' sign is not supported"),
            Self::Escape(character) => write!(
                formatter,
                "the escape '\\{}' is not supported",
                character.escape_debug()
            ),
            Self::MixedArray => formatter.write_str("arrays of mixed types are not supported"),
            Self::NestedArray => formatter.write_str("nested arrays are not supported"),
            Self::EmptyKey => formatter.write_str("an empty key is not supported"),
        }
    }
}

/// What the grammar allows where an [`SyntaxErrorKind::Unexpected`] was
/// found.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Expected {
    /// The start of a line: a key, a table header, a comment or nothing.
    LineStart,
    /// A bare or quoted key, or a table-path segment.
    Key,
    /// `=` after a key.
    Equals,
    /// A value.
    Value,
    /// A value or `]` inside an array.
    ValueOrCloseBracket,
    /// `,` or `]` after an array element.
    CommaOrCloseBracket,
    /// `.` or `]` after a table-path segment.
    DotOrCloseBracket,
    /// A comment or the end of the line, after a header or a value.
    EndOfLine,
}

impl fmt::Display for Expected {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::LineStart => "a key, a [table] header or a comment",
            Self::Key => "a key",
            Self::Equals => "'='",
            Self::Value => "a value",
            Self::ValueOrCloseBracket => "a value or ']'",
            Self::CommaOrCloseBracket => "',' or ']'",
            Self::DotOrCloseBracket => "'.' or ']'",
            Self::EndOfLine => "the end of the line",
        })
    }
}
