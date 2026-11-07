use std::fs;
use std::path::Path;

use lc3::{Format, load};

#[test]
fn asm_matches_reference_images_for_every_corpus_source() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/asm");
    let mut sources: Vec<_> = fs::read_dir(fixtures)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "asm"))
        .collect();
    sources.sort();
    assert!(!sources.is_empty());
    for source in sources {
        let expected = load(
            Format::Obj,
            &fs::read(source.with_extension("obj")).unwrap(),
        )
        .unwrap();
        let actual = load(Format::Asm, &fs::read(&source).unwrap())
            .unwrap_or_else(|errors| panic!("{}: {errors:?}", source.display()));
        assert_eq!(actual.image, expected.image, "{}", source.display());
        assert!(actual.warnings.is_empty(), "{}", source.display());
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
        assert_eq!(diagnostics.len(), 1, "{statement}");
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
        (".ORIG x3000\n.END\nHALT\n", 3, "HALT"),
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
