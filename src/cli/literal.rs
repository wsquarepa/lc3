//! LC-3 value literals as written in run files and `dump` ranges.

pub(crate) use super::literal_text::{LiteralError, parse as parse_literal};

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
    fn accepts_the_assembler_grammar_with_the_minus_after_the_prefix() {
        let cases = [
            ("x3000", 0x3000),
            ("X3000", 0x3000),
            ("xfF", 255),
            ("x-10", -16),
            ("#12", 12),
            ("#-12", -12),
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
            "0x10", "1e3", "x 10", "#x10", "-x10", "-#12",
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
