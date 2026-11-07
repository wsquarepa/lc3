/// Words placed in memory starting at an origin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    origin: u16,
    words: Vec<u16>,
}

impl Image {
    /// Returns the address at which the first word is loaded.
    pub fn origin(&self) -> u16 {
        self.origin
    }

    /// Returns the program words, excluding the source's origin word.
    pub fn words(&self) -> &[u16] {
        &self.words
    }

    /// Returns the number of program words, excluding the origin word.
    pub fn word_count(&self) -> usize {
        self.words.len()
    }
}

/// A source format accepted by [`load`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Format {
    /// Binary object file: a big-endian origin word followed by big-endian words.
    Obj,
    /// UTF-8 lc3as assembly with case-insensitive mnemonics and labels.
    Asm,
    /// UTF-8 lines of 16 binary digits, with spaces, tabs and `;` comments.
    Bin,
    /// UTF-8 lines of one to four hex digits, with an optional `x` or `X` prefix.
    Hex,
}

/// Whether a [`Diagnostic`] rejects the source or only notes something about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    /// The source is rejected and no image is produced.
    Error,
    /// The source loads; the note is returned in [`Loaded::warnings`].
    Warning,
}

/// A problem found while loading a source, tied to a line where one applies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// Whether the source was rejected.
    pub severity: Severity,
    /// One-based source line, or `None` when no source line applies.
    pub line: Option<u32>,
    /// Actionable text that names the offending input and what to supply instead.
    pub message: String,
}

/// A successfully loaded source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loaded {
    /// The words the source places in memory.
    pub image: Image,
    /// Warning diagnostics, in source order; empty for every format but `.asm`.
    pub warnings: Vec<Diagnostic>,
}

/// Loads an image from `.obj`, `.asm`, `.bin` or `.hex` source bytes.
/// `.obj` is a big-endian origin followed by words. The text formats accept
/// UTF-8: `.asm` uses lc3as syntax, `.bin` uses 16 binary digits per word,
/// and `.hex` uses one to four hex digits with an optional `x`/`X` prefix.
/// Text formats allow `;` comments; converters take the first word as origin.
/// Returns an image with assembler warnings for NOP labels and ignored text
/// after `.END`, or error diagnostics without an image. Errors cover malformed
/// bytes or text, missing origins, and invalid assembler operands or structure.
/// Text diagnostics carry one-based lines when applicable, while `.obj` errors
/// and converter sources without words have no line. Origins without words are
/// accepted. Diagnostic messages do not repeat the structured line number.
pub fn load(format: Format, bytes: &[u8]) -> Result<Loaded, Vec<Diagnostic>> {
    match format {
        Format::Obj => load_obj(bytes),
        Format::Asm => crate::assembler::assemble(bytes),
        Format::Bin => crate::convert::bin(bytes).map(|(origin, words)| Loaded {
            image: Image { origin, words },
            warnings: Vec::new(),
        }),
        Format::Hex => crate::convert::hex(bytes).map(|(origin, words)| Loaded {
            image: Image { origin, words },
            warnings: Vec::new(),
        }),
    }
}

fn load_obj(bytes: &[u8]) -> Result<Loaded, Vec<Diagnostic>> {
    if bytes.is_empty() || !bytes.len().is_multiple_of(2) {
        return Err(vec![Diagnostic {
            severity: Severity::Error,
            line: None,
            message: format!(
                "invalid .obj byte length {}: supply a two-byte big-endian origin followed by complete two-byte words",
                bytes.len()
            ),
        }]);
    }
    Ok(Loaded {
        image: Image {
            origin: u16::from_be_bytes([bytes[0], bytes[1]]),
            words: bytes[2..]
                .as_chunks::<2>()
                .0
                .iter()
                .copied()
                .map(u16::from_be_bytes)
                .collect(),
        },
        warnings: Vec::new(),
    })
}
