use std::fs;
use std::path::Path;

use lc3::{Format, load};

#[test]
fn asm_matches_native_records_and_images_for_every_corpus_source_and_quirk() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/asm");
    for directory in [&fixtures, &fixtures.join("quirks")] {
        let mut sources: Vec<_> = fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "asm"))
            .collect();
        sources.sort();
        assert!(!sources.is_empty());
        for source in sources {
            assert_differential_fixture(&source, directory != &fixtures);
        }
    }
}

fn assert_differential_fixture(source: &Path, quirk: bool) {
    let record = fs::read_to_string(source.with_extension("native")).unwrap();
    let mut native = record.lines();
    let exit = native
        .next()
        .unwrap()
        .strip_prefix("exit=")
        .unwrap()
        .parse::<i32>()
        .unwrap();
    let errors = native
        .next()
        .unwrap()
        .strip_prefix("errors=")
        .unwrap()
        .parse::<bool>()
        .unwrap();
    let warnings = native
        .next()
        .unwrap()
        .strip_prefix("warnings=")
        .unwrap()
        .parse::<bool>()
        .unwrap();
    let obj_bytes = native
        .next()
        .unwrap()
        .strip_prefix("obj_bytes=")
        .unwrap()
        .parse::<usize>()
        .unwrap();
    assert_eq!(native.next(), Some("sym_exists=true"));
    let zero_object = parse_zero_object(&record);
    assert!(record.contains("[stdout]\nSTARTING PASS 1\n"));
    assert!(record.contains("[stderr]\n"));
    assert_eq!(exit != 0, errors, "{}", source.display());
    assert_eq!(
        warnings,
        record.contains("WARNING") || record.contains("multiple .ORIG")
    );
    let actual = load(Format::Asm, &fs::read(source).unwrap());
    if !errors {
        let bytes = match zero_object {
            Some(bytes) => bytes,
            None => fs::read(source.with_extension("obj")).unwrap(),
        };
        assert_eq!(bytes.len(), obj_bytes);
        let expected = load(Format::Obj, &bytes).unwrap();
        if let Ok(loaded) = &actual {
            assert_eq!(
                loaded.image,
                expected.image,
                "ADR 0002: {}",
                source.display()
            );
        }
    } else if record.contains("errors found in first pass") && !record.contains("STARTING PASS 2") {
        assert_eq!(obj_bytes, 0, "pass-1 error: {}", source.display());
    } else {
        assert!(obj_bytes > 0, "partial image: {}", source.display());
    }
    let expected = if quirk {
        fs::read_to_string(source.with_extension("expect")).unwrap()
    } else {
        assert!(
            !errors && !warnings,
            "corpus must be clean: {}",
            source.display()
        );
        "accept".to_owned()
    };
    assert_expected_diagnostics(&actual, &expected, source);
}

/// Object bytes of an lc3as output that is one origin followed by zero words, rebuilt from the
/// record's `obj_origin=` and `obj_zero_words=` lines so the large file is never committed.
fn parse_zero_object(record: &str) -> Option<Vec<u8>> {
    let field = |key: &str| record.lines().find_map(|line| line.strip_prefix(key));
    let origin = u16::from_str_radix(field("obj_origin=x")?, 16).unwrap();
    let zero_words = field("obj_zero_words=")?.parse::<usize>().unwrap();
    let mut bytes = origin.to_be_bytes().to_vec();
    bytes.resize(2 + 2 * zero_words, 0);
    Some(bytes)
}

fn assert_expected_diagnostics(
    actual: &Result<lc3::Loaded, Vec<lc3::Diagnostic>>,
    expected: &str,
    source: &Path,
) {
    if expected.trim() == "accept" {
        assert!(
            actual
                .as_ref()
                .unwrap_or_else(|errors| panic!("{}: {errors:?}", source.display()))
                .warnings
                .is_empty()
        );
        return;
    }
    let diagnostics = if expected.starts_with("warning\t") {
        &actual
            .as_ref()
            .unwrap_or_else(|errors| panic!("{}: {errors:?}", source.display()))
            .warnings
    } else {
        actual.as_ref().expect_err(&source.display().to_string())
    };
    assert_eq!(
        diagnostics.len(),
        expected.lines().count(),
        "{}: {diagnostics:?}",
        source.display()
    );
    for (diagnostic, expectation) in diagnostics.iter().zip(expected.lines()) {
        let fields: Vec<_> = expectation.splitn(3, '\t').collect();
        let severity = match fields[0] {
            "warning" => lc3::Severity::Warning,
            "reject" => lc3::Severity::Error,
            other => panic!("invalid fixture severity {other:?}"),
        };
        let line = fields[1].parse::<u32>().unwrap();
        assert_eq!(diagnostic.severity, severity, "{}", source.display());
        assert_eq!(diagnostic.line, Some(line), "{}", source.display());
        assert!(
            diagnostic.message.contains(fields[2]),
            "{}: {diagnostic:?}, expected {expectation:?}",
            source.display()
        );
        assert!(!diagnostic.message.starts_with(&format!("line {line}:")));
    }
}

#[test]
fn asm_counts_reserved_words_and_resolves_labels_case_insensitively() {
    let source = b".oRiG x3000\nFirst\n.blkw 3\nLd R0,fIrSt\n.fill FIRST\n.EnD\n";
    let loaded = load(Format::Asm, source).unwrap();
    assert_eq!(loaded.image.origin(), 0x3000);
    assert_eq!(loaded.image.words(), &[0, 0, 0, 0x21fc, 0x3000]);
    assert_eq!(loaded.image.word_count(), 5);
}

#[test]
fn asm_emits_each_supported_string_escape_and_a_terminator() {
    let source = br#".ORIG x3000
.STRINGZ "\n\t\r\\\""
.END
"#;
    let loaded = load(Format::Asm, source).unwrap();
    assert_eq!(loaded.image.words(), &[10, 9, 13, 92, 34, 0]);
}

#[test]
fn asm_rejects_unsupported_escapes_raw_newlines_and_unsafe_immediates_with_context() {
    for (statement, offending) in [
        (r#".STRINGZ "\0""#, r"\0"),
        (r#".STRINGZ "\a""#, r"\a"),
        (r#".STRINGZ "\q""#, r"\q"),
        (".STRINGZ \"raw\nnewline\"", "raw"),
        ("ADD R0,R0,#16", "#16"),
        ("ADD R0,R0,xFFFF", "xFFFF"),
        ("ADD R0,R0,#-17", "#-17"),
        ("BR #256", "#256"),
        ("LDR R0,R0,#32", "#32"),
        ("JSR #1024", "#1024"),
        ("TRAP #256", "#256"),
    ] {
        let source = format!("; heading\n.ORIG x3000\n{statement}\n.END\n");
        let diagnostics = load(Format::Asm, source.as_bytes()).unwrap_err();
        assert_eq!(
            diagnostics.len(),
            if statement.contains('\n') { 2 } else { 1 },
            "{statement}"
        );
        assert_eq!(diagnostics[0].severity, lc3::Severity::Error);
        assert_eq!(diagnostics[0].line, Some(3), "{statement}");
        assert!(
            diagnostics[0].message.contains(offending),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn asm_rejects_invalid_utf8_with_the_byte_offset() {
    let diagnostics = load(Format::Asm, b"; valid\n.ORIG x3000\n\xff\n.END\n").unwrap_err();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].severity, lc3::Severity::Error);
    assert_eq!(diagnostics[0].line, Some(3));
    assert!(diagnostics[0].message.contains("UTF-8"));
    assert!(diagnostics[0].message.contains("byte offset 20"));
}

#[test]
fn asm_rejects_ambiguous_structure_and_malformed_operands() {
    for (source, line, offending) in [
        (".ORIG x3000\n.ORIG x4000\n.END\n", 2, ".ORIG"),
        (".ORIG x3000\nHALT\n", 2, "HALT"),
        (".ORIG x3000\nSame\nsAmE\n.END\n", 3, "sAmE"),
        (".ORIG x3000\nBR Missing\n.END\n", 2, "Missing"),
        (".ORIG x3000\nADD R0 R1 R2\n.END\n", 2, "ADD"),
        (".ORIG x3000\nADD R8,R0,R0\n.END\n", 2, "R8"),
        (".ORIG x3000\n.FILL #+1\n.END\n", 2, "#+1"),
        (".ORIG x3000\n.FILL R0\n.END\n", 2, "R0"),
        (".ORIG x3000\n.STRINGZ \"open\n.END\n", 2, "open"),
        (".ORIG x3000\n.END extra\n", 2, "extra"),
    ] {
        let diagnostics = load(Format::Asm, source.as_bytes()).unwrap_err();
        assert_eq!(diagnostics[0].line, Some(line), "{source}");
        assert!(
            diagnostics[0].message.contains(offending),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn asm_rejects_strings_that_lc3as_extends_across_later_lines() {
    let sources = [
        ".ORIG x3000\n.STRINGZ \"a\\\\\"\nHALT\n; trailing quote \"\n.END\n",
        ".ORIG x3000\n.STRINGZ \"a\\\\\"\nHALT ; done\"\n.END\n",
        ".ORIG x3000\r\n.STRINGZ \"a\\\\\"\r\nHALT\r\n; trailing quote \"\r\n.END\r\n",
        ".ORIG x3000\n.STRINGZ \"a\\\\\"\nHidden HALT\n; trailing quote \"\n.END\n",
        ".ORIG x3000\n.STRINGZ \"a\\\\\"\nHALT ; escaped \\\"\n; trailing quote \"\n.END\n",
        ".ORIG x3000\n.STRINGZ \"a\\\\\"\nHALT ; escaped \\\"\n; blocked \" extra\n.END\n",
        ".ORIG x3000\n.STRINGZ \"a\\\\\"\nHALT ; done\" \t ; suffix\n.END\n",
    ];
    for source in sources {
        let diagnostics = load(Format::Asm, source.as_bytes()).unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, lc3::Severity::Error);
        assert_eq!(diagnostics[0].line, Some(2), "{source:?}");
        assert!(
            diagnostics[0].message.contains(".STRINGZ"),
            "{diagnostics:?}"
        );
        assert!(
            diagnostics[0].message.contains("swallow"),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn asm_rejects_whitespace_and_stray_operand_forms_that_lc3as_rejects() {
    let statements = [
        "ADD R0,\u{a0}R1,#1",
        "\u{b}HALT",
        "\u{c}HALT",
        "LOOP R0",
        "LOOP x10",
        "  x10",
        "\tR0",
        "ADD R0,R1,\u{b}#1",
        "HALT\u{c}",
    ];
    for statement in statements {
        let source = format!(".ORIG x3000\n{statement}\n.END\n");
        let diagnostics = load(Format::Asm, source.as_bytes()).unwrap_err();
        assert_eq!(diagnostics.len(), 1, "{statement:?}");
        assert_eq!(diagnostics[0].severity, lc3::Severity::Error);
        assert_eq!(diagnostics[0].line, Some(2), "{statement:?}");
        assert!(
            diagnostics[0].message.contains(&format!("{statement:?}")),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn asm_warns_for_nop_labels_and_only_the_first_text_after_end() {
    let source = b".ORIG x3000\n  nOp ; label only\nBR NOP\n.END\n ; comment\n\n  ? ignored\n.STRINGZ \"invalid\n";
    let loaded = load(Format::Asm, source).unwrap();
    assert_eq!(loaded.image.words(), &[0x0fff]);
    assert_eq!(loaded.warnings.len(), 2);
    for (warning, line, reason) in [
        (&loaded.warnings[0], 2, "NOP is a label and emits no word"),
        (&loaded.warnings[1], 7, "text after .END is ignored"),
    ] {
        assert_eq!(warning.severity, lc3::Severity::Warning);
        assert_eq!(warning.line, Some(line));
        assert!(warning.message.contains(reason), "{warning:?}");
        assert!(!warning.message.contains(&format!("line {line}")));
    }
}

#[test]
fn asm_collects_range_duplicate_and_undefined_label_errors_across_both_passes() {
    let source = b".ORIG x3000\n.ORIG #65536\nSame HALT\nsAmE ADD R0,R0,#16\nAND R1,R1,#-17\nLDR R0,R0,#32\nSTR R0,R0,#-33\nBR #256\nLD R0,#-257\nJSR #1024\nJSR #-1025\nTRAP #-1\nTRAP #256\n.FILL #-32769\n.FILL #65536\n.BLKW #-1\n.BLKW #65536\nBR Missing\n.FILL Absent\n.END extra\n";
    let diagnostics = load(Format::Asm, source).unwrap_err();
    for (line, reason) in [
        (2, "second .ORIG"),
        (2, "outside 0..=65535"),
        (4, "duplicate label \"sAmE\""),
        (4, "outside -16..=15"),
        (5, "outside -16..=15"),
        (6, "outside -32..=31"),
        (7, "outside -32..=31"),
        (8, "outside -256..=255"),
        (9, "outside -256..=255"),
        (10, "outside -1024..=1023"),
        (11, "outside -1024..=1023"),
        (12, "outside 0..=255"),
        (13, "outside 0..=255"),
        (14, "outside -32768..=65535"),
        (15, "outside -32768..=65535"),
        (16, "outside 0..=65535"),
        (17, "outside 0..=65535"),
        (18, "undefined label \"Missing\""),
        (19, "undefined label \"Absent\""),
        (20, ".END takes no operands"),
    ] {
        assert!(
            diagnostics.iter().any(|diagnostic| {
                diagnostic.line == Some(line) && diagnostic.message.contains(reason)
            }),
            "missing {line}: {reason} in {diagnostics:?}"
        );
    }
    assert_eq!(diagnostics.len(), 20, "{diagnostics:?}");
    assert!(diagnostics.iter().all(|diagnostic| {
        diagnostic.severity == lc3::Severity::Error && !diagnostic.message.contains("line ")
    }));
}

#[test]
fn asm_accepts_exact_field_boundaries_without_reinterpreting_literals() {
    let loaded = load(Format::Asm, b".ORIG x0000\nADD R0,R0,#-16\nAND R0,R0,#15\nLDR R0,R0,#-32\nSTR R0,R0,#31\nBR #-256\nLD R0,#255\nJSR #-1024\nJSR #1023\nTRAP x00\nTRAP xFF\n.FILL #-32768\n.FILL #65535\n.BLKW #0\n.END\n").unwrap();
    assert_eq!(loaded.image.origin(), 0);
    assert_eq!(
        loaded.image.words(),
        &[
            0x1030, 0x502f, 0x6020, 0x701f, 0x0f00, 0x20ff, 0x4c00, 0x4bff, 0xf000, 0xf0ff, 0x8000,
            0xffff
        ]
    );
    assert!(loaded.warnings.is_empty());
    for source in [b".ORIG xFFFF\n.END\n".as_slice(), b".ORIG #65535\n.END\n"] {
        assert_eq!(load(Format::Asm, source).unwrap().image.origin(), 65535);
    }
    let reserved = load(Format::Asm, b".ORIG #0\n.BLKW #65535\n.END\n").unwrap();
    assert_eq!(reserved.image.word_count(), 65535);
    assert!(reserved.image.words().iter().all(|word| *word == 0));
}

#[test]
fn asm_takes_the_minus_after_a_literal_prefix_and_rejects_it_before() {
    let loaded = load(
        Format::Asm,
        b".ORIG x3000\n.FILL x-10\n.FILL #-12\n.FILL -12\n.END\n",
    )
    .unwrap();
    assert_eq!(loaded.image.words(), &[0xfff0, 0xfff4, 0xfff4]);
    for literal in ["-x10", "-#12"] {
        let source = format!(".ORIG x3000\n.FILL {literal}\n.END\n");
        assert!(load(Format::Asm, source.as_bytes()).is_err(), "{literal}");
    }
}

#[test]
fn asm_rejects_negative_and_oversized_origins() {
    for literal in ["#-1", "x-1", "#65536", "x10000"] {
        let source = format!(".ORIG {literal}\nHALT\n.END\n");
        let diagnostics = load(Format::Asm, source.as_bytes()).unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].line, Some(1));
        assert!(diagnostics[0].message.contains("outside 0..=65535"));
    }
}

#[test]
fn asm_names_labels_and_distances_at_pc_relative_boundaries() {
    for (opcode, reserved, expected) in [
        ("BR", 255, Ok(0x0eff)),
        ("BR", 256, Err("offset 256 outside -256..=255")),
        ("JSR", 1023, Ok(0x4bff)),
        ("JSR", 1024, Err("offset 1024 outside -1024..=1023")),
    ] {
        let source =
            format!(".ORIG x3000\n{opcode} Target\n.BLKW #{reserved}\nTarget HALT\n.END\n");
        match expected {
            Ok(word) => assert_eq!(
                load(Format::Asm, source.as_bytes()).unwrap().image.words()[0],
                word
            ),
            Err(reason) => {
                let diagnostics = load(Format::Asm, source.as_bytes()).unwrap_err();
                assert_eq!(diagnostics.len(), 1);
                assert_eq!(diagnostics[0].line, Some(2));
                assert!(diagnostics[0].message.contains("label \"Target\""));
                assert!(diagnostics[0].message.contains(reason));
            }
        }
    }
    for (opcode, reserved, expected) in [
        ("LD R0,", 254, Ok(0x2100)),
        ("ST R0,", 255, Err("offset -257 outside -256..=255")),
        ("JSR", 1022, Ok(0x4c00)),
        ("JSR", 1023, Err("offset -1025 outside -1024..=1023")),
    ] {
        let source =
            format!(".ORIG x3000\nTarget HALT\n.BLKW #{reserved}\n{opcode} Target\n.END\n");
        match expected {
            Ok(word) => assert_eq!(
                *load(Format::Asm, source.as_bytes())
                    .unwrap()
                    .image
                    .words()
                    .last()
                    .unwrap(),
                word
            ),
            Err(reason) => {
                let diagnostics = load(Format::Asm, source.as_bytes()).unwrap_err();
                assert_eq!(diagnostics.len(), 1);
                assert_eq!(diagnostics[0].line, Some(4));
                assert!(diagnostics[0].message.contains("label \"Target\""));
                assert!(diagnostics[0].message.contains(reason));
            }
        }
    }
}
