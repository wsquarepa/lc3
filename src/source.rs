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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Format {
    Obj,
    Asm,
    /// UTF-8 lines of 16 binary digits, with spaces, tabs and `;` comments.
    Bin,
    /// UTF-8 lines of one to four hex digits, with an optional `x` or `X` prefix.
    Hex,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: Severity,
    /// One-based source line, or `None` for `.obj` bytes.
    pub line: Option<u32>,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loaded {
    pub image: Image,
    pub warnings: Vec<Diagnostic>,
}

/// Loads big-endian `.obj` bytes containing an origin followed by program words.
/// Returns an image and warnings, or one error diagnostic for an empty source or
/// an odd byte length. An origin without program words is accepted.
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
