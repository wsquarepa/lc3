//! LC-3 value literals as written in run files and `dump` ranges.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiteralError {
    /// The text is not a literal at all.
    Malformed,
    /// The text is a literal whose value does not fit the requested width.
    OutOfRange,
}

/// Parses `x`/`X` hex, `#` decimal or bare decimal text, each with an optional
/// `-` (before the prefix or, for `x` and `#`, after it). Rejects `+`, spaces
/// and empty digit strings with [`LiteralError::Malformed`]; values beyond
/// `i64` are [`LiteralError::OutOfRange`].
pub fn parse_literal(text: &str) -> Result<i64, LiteralError> {
    let (leading_minus, rest) = split_minus(text);
    let (radix, body) = match rest.strip_prefix(['x', 'X']) {
        Some(body) => (16, body),
        None => match rest.strip_prefix('#') {
            Some(body) => (10, body),
            None => (10, rest),
        },
    };
    let (trailing_minus, digits) = split_minus(body);
    let is_prefixed = body.len() != rest.len();
    if (leading_minus && trailing_minus) || (trailing_minus && !is_prefixed) {
        return Err(LiteralError::Malformed);
    }
    if digits.is_empty() || !digits.chars().all(|digit| digit.is_digit(radix)) {
        return Err(LiteralError::Malformed);
    }
    let magnitude = i64::from_str_radix(digits, radix).map_err(|_| LiteralError::OutOfRange)?;
    Ok(if leading_minus || trailing_minus {
        -magnitude
    } else {
        magnitude
    })
}

fn split_minus(text: &str) -> (bool, &str) {
    match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text),
    }
}

/// Narrows `value` to a memory or register word; negative values are two's complement.
pub fn word(value: i64) -> Result<u16, LiteralError> {
    address(value).or_else(|_| {
        i16::try_from(value)
            .map(i16::cast_unsigned)
            .map_err(|_| LiteralError::OutOfRange)
    })
}

/// Narrows `value` to an address; only 0..65535 is accepted.
pub fn address(value: i64) -> Result<u16, LiteralError> {
    u16::try_from(value).map_err(|_| LiteralError::OutOfRange)
}

pub const WORD_RANGE: &str = "-32768..65535";
pub const ADDRESS_RANGE: &str = "0..65535";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_hex_decimal_and_bare_forms_with_either_sign_position() {
        let cases = [
            ("x3000", 0x3000),
            ("X3000", 0x3000),
            ("xfF", 255),
            ("x-10", -16),
            ("-x10", -16),
            ("#12", 12),
            ("#-12", -12),
            ("-#12", -12),
            ("12", 12),
            ("-12", -12),
            ("0", 0),
        ];
        for (text, expected) in cases {
            assert_eq!(parse_literal(text), Ok(expected), "{text}");
        }
    }

    #[test]
    fn rejects_malformed_text() {
        for text in [
            "", "x", "#", "-", "+1", " 1", "1 ", "xG", "x1.0", "--1", "x--1", "-x-1", "1-", "-1-",
            "0x10", "1e3", "x 10", "#x10",
        ] {
            assert_eq!(
                parse_literal(text),
                Err(LiteralError::Malformed),
                "{text:?}"
            );
        }
    }

    #[test]
    fn overflowing_digits_are_out_of_range() {
        assert_eq!(
            parse_literal("99999999999999999999"),
            Err(LiteralError::OutOfRange)
        );
    }

    #[test]
    fn words_accept_minus_32768_through_65535() {
        assert_eq!(word(-32768), Ok(0x8000));
        assert_eq!(word(-1), Ok(0xFFFF));
        assert_eq!(word(0), Ok(0));
        assert_eq!(word(65535), Ok(0xFFFF));
        assert_eq!(word(-32769), Err(LiteralError::OutOfRange));
        assert_eq!(word(65536), Err(LiteralError::OutOfRange));
    }

    #[test]
    fn addresses_accept_only_0_through_65535() {
        assert_eq!(address(0), Ok(0));
        assert_eq!(address(65535), Ok(0xFFFF));
        assert_eq!(address(-1), Err(LiteralError::OutOfRange));
        assert_eq!(address(65536), Err(LiteralError::OutOfRange));
    }
}
