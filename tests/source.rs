use lc3::{Format, Image, Severity, load};

#[test]
fn new_image_equals_the_same_image_loaded_from_obj() {
    let image = Image::new(0x3000, vec![0x1234, 0xabcd]);
    assert_eq!(image.origin(), 0x3000);
    assert_eq!(image.words(), &[0x1234, 0xabcd]);
    let loaded = load(Format::Obj, &[0x30, 0x00, 0x12, 0x34, 0xab, 0xcd]).unwrap();
    assert_eq!(loaded.image, image);
}

#[test]
fn obj_loads_big_endian_origin_and_program_words() {
    let loaded = load(Format::Obj, &[0x30, 0x00, 0x12, 0x34, 0xab, 0xcd]).unwrap();
    assert_eq!(loaded.image.origin(), 0x3000);
    assert_eq!(loaded.image.words(), &[0x1234, 0xabcd]);
    assert_eq!(loaded.image.word_count(), 2);
    assert!(loaded.warnings.is_empty());
}

#[test]
fn obj_accepts_an_origin_without_program_words() {
    let loaded = load(Format::Obj, &[0xff, 0xff]).unwrap();
    assert_eq!(loaded.image.origin(), 0xffff);
    assert!(loaded.image.words().is_empty());
    assert_eq!(loaded.image.word_count(), 0);
}

#[test]
fn obj_rejects_empty_and_odd_sources_with_byte_length_context() {
    for bytes in [&[][..], &[0x30][..], &[0x30, 0x00, 0xab][..]] {
        let diagnostics = load(Format::Obj, bytes).unwrap_err();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, Severity::Error);
        assert_eq!(diagnostics[0].line, None);
        assert!(diagnostics[0].message.contains(&bytes.len().to_string()));
        assert!(
            diagnostics[0]
                .message
                .contains("two-byte big-endian origin")
        );
        assert!(diagnostics[0].message.contains("complete two-byte words"));
    }
}
