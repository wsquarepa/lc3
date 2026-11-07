use lc3::{Format, Severity, load};
use std::{fs, path::Path};

#[test]
fn bin_loads_origin_and_words_with_spacing_comments_and_blank_lines() {
    let source = "; origin follows\r\n\t \r\n 0011 0000\t0000 0000 ; origin\r\n\
                  0000\t0000 0000\t0000\r\n1111111111111111\r\n1010 1011 1100 1101 ; λ";
    let loaded = load(Format::Bin, source.as_bytes()).unwrap();
    assert_eq!(loaded.image.origin(), 0x3000);
    assert_eq!(loaded.image.words(), &[0x0000, 0xffff, 0xabcd]);
    assert_eq!(loaded.image.word_count(), 3);
    assert!(loaded.warnings.is_empty());
}

#[test]
fn hex_loads_one_to_four_digits_with_optional_prefix_and_either_case() {
    let source = b"; origin follows\r\n\t \r\n x3000 ; origin\r\n0\r\nXf\r\n aB \t; word\r\nXABC\r\nxFFFF\r\nffff";
    let loaded = load(Format::Hex, source).unwrap();
    assert_eq!(loaded.image.origin(), 0x3000);
    assert_eq!(
        loaded.image.words(),
        &[0x0, 0xf, 0xab, 0xabc, 0xffff, 0xffff]
    );
    assert_eq!(loaded.image.word_count(), 6);
    assert!(loaded.warnings.is_empty());
}

#[test]
fn bin_reports_every_malformed_line_with_its_number_and_text() {
    let bad_lines = [
        "000000000000000",
        "00000000000000000",
        "000000000000000g",
        "00000000\u{b}00000000",
        "00000000\r00000000",
        "00000000\u{a0}00000000",
    ];
    let source = format!(
        "; heading\n\n0011000000000000\n{}\n1111000000100101\n",
        bad_lines.join("\n")
    );
    let diagnostics = load(Format::Bin, source.as_bytes()).unwrap_err();
    assert_eq!(diagnostics.len(), bad_lines.len());
    for (index, (diagnostic, bad_line)) in diagnostics.iter().zip(bad_lines).enumerate() {
        assert_eq!(diagnostic.severity, Severity::Error);
        assert_eq!(diagnostic.line, Some(index as u32 + 4));
        assert!(!diagnostic.message.contains(&format!("line {}", index + 4)));
        assert!(diagnostic.message.contains(&format!("{bad_line:?}")));
        assert!(diagnostic.message.contains("exactly 16 binary digits"));
    }
}

#[test]
fn hex_reports_every_malformed_line_with_its_number_and_text() {
    let bad_lines = [
        "-1", "+1", "x-1", "x+1", "12345", "x00000", "g", "x", "X", "0x12", "1 2", "x\t1",
        "\u{b}1", "1\r2", "\u{a0}1",
    ];
    let source = format!("; heading\n\nx3000\n{}\nxF025\n", bad_lines.join("\n"));
    let diagnostics = load(Format::Hex, source.as_bytes()).unwrap_err();
    assert_eq!(diagnostics.len(), bad_lines.len());
    for (index, (diagnostic, bad_line)) in diagnostics.iter().zip(bad_lines).enumerate() {
        assert_eq!(diagnostic.severity, Severity::Error);
        assert_eq!(diagnostic.line, Some(index as u32 + 4));
        assert!(!diagnostic.message.contains(&format!("line {}", index + 4)));
        assert!(diagnostic.message.contains(&format!("{bad_line:?}")));
        assert!(diagnostic.message.contains("one to four hex digits"));
    }
}

#[test]
fn converters_reject_empty_blank_and_comment_only_sources_without_a_line_number() {
    for format in [Format::Bin, Format::Hex] {
        for source in ["", " \t\r\n\n\t ", "; comment\r\n \t; another comment"] {
            let diagnostics = load(format.clone(), source.as_bytes()).unwrap_err();
            assert_eq!(diagnostics.len(), 1);
            assert_eq!(diagnostics[0].severity, Severity::Error);
            assert_eq!(diagnostics[0].line, None);
            assert!(diagnostics[0].message.contains("origin word"));
        }
    }
}

#[test]
fn converters_accept_an_origin_without_image_words() {
    for (format, source) in [
        (Format::Bin, " \t1111 1111 1111 1111 ; origin"),
        (Format::Hex, " \txFFFF\t; origin"),
    ] {
        let loaded = load(format, source.as_bytes()).unwrap();
        assert_eq!(loaded.image.origin(), 0xffff);
        assert!(loaded.image.words().is_empty());
        assert_eq!(loaded.image.word_count(), 0);
        assert!(loaded.warnings.is_empty());
    }
}

#[test]
fn converters_report_invalid_utf8_on_every_line_even_in_comments() {
    for (format, origin) in [(Format::Bin, "0011000000000000"), (Format::Hex, "x3000")] {
        let mut source = format!("{origin}\r\n").into_bytes();
        source.extend_from_slice(b"\xff\r\n; comment \xfe\n");
        source.extend_from_slice(origin.as_bytes());
        let diagnostics = load(format, &source).unwrap_err();
        assert_eq!(diagnostics.len(), 2);
        for (diagnostic, line, byte) in [(&diagnostics[0], 2, "ff"), (&diagnostics[1], 3, "fe")] {
            assert_eq!(diagnostic.severity, Severity::Error);
            assert_eq!(diagnostic.line, Some(line));
            assert!(!diagnostic.message.contains(&format!("line {line}")));
            assert!(diagnostic.message.contains("UTF-8"));
            assert!(diagnostic.message.contains(byte));
        }
    }
}

#[test]
fn malformed_origins_report_only_the_bad_lines_including_at_eof() {
    for format in [Format::Bin, Format::Hex] {
        let diagnostics = load(format, b"?\n\n; comment\n!").unwrap_err();
        assert_eq!(diagnostics.len(), 2);
        assert_eq!(diagnostics[0].line, Some(1));
        assert_eq!(diagnostics[1].line, Some(4));
        assert!(diagnostics[0].message.contains('?'));
        assert!(diagnostics[1].message.contains('!'));
        assert!(
            diagnostics
                .iter()
                .all(|diagnostic| diagnostic.severity == Severity::Error)
        );
    }
}

#[test]
fn converters_match_native_reference_images_from_committed_fixtures() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/convert");
    for (format, name) in [
        (Format::Bin, "spacing.bin"),
        (Format::Bin, "boundaries.bin"),
        (Format::Bin, "origin.bin"),
        (Format::Hex, "mixed.hex"),
        (Format::Hex, "limits.hex"),
        (Format::Hex, "empty-image.hex"),
    ] {
        let source = fixtures.join(name);
        let expected = load(
            Format::Obj,
            &fs::read(source.with_extension("obj")).unwrap(),
        )
        .unwrap();
        let actual = load(format, &fs::read(&source).unwrap()).unwrap();
        assert_eq!(actual, expected, "fixture {name}");
    }
}
