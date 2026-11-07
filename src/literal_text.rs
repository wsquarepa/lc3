//! The LC-3 literal grammar, compiled into both the library (the assembler) and
//! the `lc3` binary (run files and dump ranges) so the two cannot disagree.

/// Why text is not an integer literal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LiteralError {
    /// The text is not a literal at all.
    Malformed,
    /// The text is a literal whose magnitude does not fit an `i64`.
    OutOfRange,
}

/// Parses `x`/`X` hex, `#` decimal or bare decimal text. Each form takes an
/// optional `-` directly before its digits, which for the prefixed forms means
/// after the prefix (`x-10`, `#-12`, `-12`); `-x10` and `-#12` are malformed.
/// Rejects `+`, spaces and empty digit strings with [`LiteralError::Malformed`];
/// values beyond `i64` are [`LiteralError::OutOfRange`].
pub(crate) fn parse(text: &str) -> Result<i64, LiteralError> {
    let (radix, body) = match text.strip_prefix(['x', 'X']) {
        Some(body) => (16, body),
        None => (10, text.strip_prefix('#').unwrap_or(text)),
    };
    let digits = body.strip_prefix('-').unwrap_or(body);
    if digits.is_empty() || !digits.chars().all(|digit| digit.is_digit(radix)) {
        return Err(LiteralError::Malformed);
    }
    i64::from_str_radix(body, radix).map_err(|_| LiteralError::OutOfRange)
}
