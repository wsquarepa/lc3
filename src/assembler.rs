use std::collections::HashMap;

use crate::{Diagnostic, Format, Loaded, Severity, load};

struct ParsedLine<'a> {
    labels: Vec<&'a str>,
    opcode: String,
    operands: &'a str,
}

struct Statement<'a> {
    line: u32,
    text: &'a str,
    address: u16,
    emission: Emission<'a>,
}

enum Emission<'a> {
    Data(Vec<u16>),
    Instruction { opcode: String, operands: &'a str },
}

struct Layout<'a> {
    origin: u16,
    statements: Vec<Statement<'a>>,
    labels: HashMap<String, u16>,
}

/// Assembles UTF-8 lc3as source bytes into an image without writing files.
/// Returns line-numbered errors for unsupported syntax, invalid operands or labels;
/// invalid UTF-8 errors identify the byte offset.
pub(crate) fn assemble(bytes: &[u8]) -> Result<Loaded, Vec<Diagnostic>> {
    let source = std::str::from_utf8(bytes).map_err(|error| {
        vec![Diagnostic {
            severity: Severity::Error,
            line: u32::try_from(
                bytes[..error.valid_up_to()]
                    .iter()
                    .filter(|byte| **byte == b'\n')
                    .count()
                    + 1,
            )
            .ok(),
            message: format!("invalid UTF-8 at byte offset {}", error.valid_up_to()),
        }]
    })?;
    assemble_source(source).map_err(|error| vec![error])
}

fn assemble_source(source: &str) -> Result<Loaded, Diagnostic> {
    let layout = layout(source)?;
    let mut words = vec![layout.origin];
    for statement in layout.statements {
        match statement.emission {
            Emission::Data(data) => words.extend(data),
            Emission::Instruction { opcode, operands } => words.push(
                encode_instruction(&opcode, operands, &layout.labels, statement.address)
                    .map_err(|reason| diagnostic(statement.line, statement.text, &reason))?,
            ),
        }
    }
    let bytes: Vec<_> = words.into_iter().flat_map(u16::to_be_bytes).collect();
    load(Format::Obj, &bytes)
        .map_err(|errors| diagnostic(1, source, &format!("invalid assembled image: {errors:?}")))
}

fn layout(source: &str) -> Result<Layout<'_>, Diagnostic> {
    let mut origin = None;
    let mut address = 0;
    let mut labels = HashMap::new();
    let mut statements = Vec::new();
    let mut ended = false;
    let mut last_line = 1;
    let mut last_text = "";
    for (index, text) in source.lines().enumerate() {
        let line = u32::try_from(index + 1)
            .map_err(|_| diagnostic(u32::MAX, text, "source exceeds the line number limit"))?;
        last_line = line;
        last_text = text;
        let parsed = parse_line(text).map_err(|reason| diagnostic(line, text, &reason))?;
        if parsed.labels.is_empty() && parsed.opcode.is_empty() {
            continue;
        }
        if ended {
            return Err(diagnostic(line, text, "text after .END is unsupported"));
        }
        for label in parsed.labels {
            if origin.is_none() {
                return Err(diagnostic(line, text, "label before .ORIG"));
            }
            if labels.insert(label.to_ascii_uppercase(), address).is_some() {
                return Err(diagnostic(
                    line,
                    text,
                    &format!("duplicate label {label:?}"),
                ));
            }
        }
        let emission = match parsed.opcode.as_str() {
            "" => continue,
            ".ORIG" => {
                if origin.is_some() {
                    return Err(diagnostic(line, text, "second .ORIG is not allowed"));
                }
                address = literal(parsed.operands, -32768, 65535)
                    .map_err(|reason| diagnostic(line, text, &reason))?;
                origin = Some(address);
                continue;
            }
            ".END" if parsed.operands.is_empty() => {
                ended = true;
                continue;
            }
            ".BLKW" => Emission::Data(vec![
                0;
                usize::from(
                    literal(parsed.operands, 0, 65535)
                        .map_err(|reason| diagnostic(line, text, &reason))?
                )
            ]),
            ".STRINGZ" => Emission::Data(
                string_words(parsed.operands).map_err(|reason| diagnostic(line, text, &reason))?,
            ),
            _ => Emission::Instruction {
                opcode: parsed.opcode,
                operands: parsed.operands,
            },
        };
        if origin.is_none() {
            return Err(diagnostic(
                line,
                text,
                "instruction or directive before .ORIG",
            ));
        }
        let word_count = match &emission {
            Emission::Data(words) => words.len(),
            Emission::Instruction { .. } => 1,
        };
        statements.push(Statement {
            line,
            text,
            address,
            emission,
        });
        address = address.wrapping_add(word_count as u16);
    }
    if !ended {
        return Err(diagnostic(last_line, last_text, "missing .END"));
    }
    let origin = origin.ok_or_else(|| diagnostic(last_line, ".END", "missing .ORIG"))?;
    Ok(Layout {
        origin,
        statements,
        labels,
    })
}

fn parse_line(text: &str) -> Result<ParsedLine<'_>, String> {
    let mut remaining = before_comment(text).trim();
    let mut labels = Vec::new();
    while !remaining.is_empty() {
        let length = remaining
            .bytes()
            .enumerate()
            .take_while(|(index, byte)| {
                byte.is_ascii_alphanumeric() || *byte == b'_' || (*index == 0 && *byte == b'.')
            })
            .count();
        let token = &remaining[..length];
        let tail = remaining[length..].trim_start_matches([' ', '\t']);
        if is_label(token)
            && let Some(tail) = tail.strip_prefix(':')
        {
            labels.push(token);
            remaining = tail.trim_start_matches([' ', '\t']);
            continue;
        }
        let opcode = token.to_ascii_uppercase();
        if is_opcode(&opcode) {
            if length < remaining.len() && !remaining[length..].starts_with([' ', '\t']) {
                return Err(format!("expected whitespace after {token:?}"));
            }
            return Ok(ParsedLine {
                labels,
                opcode,
                operands: tail.trim_end(),
            });
        }
        if !is_label(token) {
            return Err(format!("unrecognized syntax {remaining:?}"));
        }
        labels.push(token);
        remaining = tail;
    }
    Ok(ParsedLine {
        labels,
        opcode: String::new(),
        operands: "",
    })
}

fn before_comment(text: &str) -> &str {
    let mut quoted = false;
    let mut escaped = false;
    for (index, byte) in text.bytes().enumerate() {
        if escaped {
            escaped = false;
        } else if quoted && byte == b'\\' {
            escaped = true;
        } else if byte == b'"' {
            quoted = !quoted;
        } else if !quoted && byte == b';' {
            return &text[..index];
        }
    }
    text
}

fn is_opcode(opcode: &str) -> bool {
    matches!(
        opcode,
        "ADD"
            | "AND"
            | "NOT"
            | "JMP"
            | "RET"
            | "JSR"
            | "JSRR"
            | "LD"
            | "LDI"
            | "LDR"
            | "LEA"
            | "ST"
            | "STI"
            | "STR"
            | "TRAP"
            | "RTI"
            | "GETC"
            | "OUT"
            | "PUTS"
            | "IN"
            | "PUTSP"
            | "HALT"
            | ".ORIG"
            | ".END"
            | ".FILL"
            | ".BLKW"
            | ".STRINGZ"
    ) || branch_conditions(opcode).is_some()
}

fn is_label(text: &str) -> bool {
    text.as_bytes().first().is_some_and(u8::is_ascii_alphabetic)
        && text
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}

fn string_words(text: &str) -> Result<Vec<u16>, String> {
    let content = text
        .strip_prefix('"')
        .ok_or_else(|| format!("expected quoted string, got {text:?}"))?;
    let mut words = Vec::new();
    let mut bytes = content.as_bytes().iter().copied();
    while let Some(byte) = bytes.next() {
        match byte {
            b'"' => {
                if bytes.len() != 0 {
                    return Err(format!("unexpected text after string in {text:?}"));
                }
                words.push(0);
                return Ok(words);
            }
            b'\n' | b'\r' => return Err(format!("raw newline in string {text:?}")),
            b'\\' => {
                let escaped = match bytes.next() {
                    Some(b'n') => b'\n',
                    Some(b't') => b'\t',
                    Some(b'r') => b'\r',
                    Some(b'\\') => b'\\',
                    Some(b'"') => b'"',
                    Some(other) => {
                        return Err(format!(
                            "unsupported string escape \\{} in {text:?}",
                            char::from(other)
                        ));
                    }
                    None => return Err(format!("unterminated string escape in {text:?}")),
                };
                words.push(u16::from(escaped));
            }
            byte => words.push(u16::from(byte)),
        }
    }
    Err(format!("unterminated string or raw newline in {text:?}"))
}

fn label_address(text: &str, labels: &HashMap<String, u16>) -> Result<u16, String> {
    if !is_label(text) || is_register(text) {
        return Err(format!("expected a label or literal, got {text:?}"));
    }
    labels
        .get(&text.to_ascii_uppercase())
        .copied()
        .ok_or_else(|| format!("undefined label {text:?}"))
}

fn relative_field(
    text: &str,
    bits: u32,
    labels: &HashMap<String, u16>,
    address: u16,
) -> Result<u16, String> {
    if literal_parts(text).is_some() {
        return signed_field(text, bits);
    }
    let offset = i64::from(label_address(text, labels)?) - i64::from(address) - 1;
    let limit = 1_i64 << (bits - 1);
    if !(-limit..limit).contains(&offset) {
        return Err(format!(
            "label {text:?} at offset {offset} outside {}..={}",
            -limit,
            limit - 1
        ));
    }
    Ok((offset as u16) & ((1_u16 << bits) - 1))
}

fn encode_instruction(
    opcode: &str,
    operands: &str,
    labels: &HashMap<String, u16>,
    address: u16,
) -> Result<u16, String> {
    let operands: Vec<_> = if operands.is_empty() {
        Vec::new()
    } else {
        operands.split(',').map(str::trim).collect()
    };
    if let Some(conditions) = branch_conditions(opcode) {
        return match operands.as_slice() {
            [offset] => Ok(conditions | relative_field(offset, 9, labels, address)?),
            _ => Err(format!(
                "expected one offset or label for {opcode:?}, got {operands:?}"
            )),
        };
    }
    match (opcode, operands.as_slice()) {
        ("ADD" | "AND", [destination, source, last]) => {
            let base = if opcode == "ADD" { 0x1000 } else { 0x5000 };
            let last = if is_register(last) {
                register(last)?
            } else {
                0x20 | signed_field(last, 5)?
            };
            Ok(base | (register(destination)? << 9) | (register(source)? << 6) | last)
        }
        ("NOT", [destination, source]) => {
            Ok(0x903f | (register(destination)? << 9) | (register(source)? << 6))
        }
        ("JMP" | "JSRR", [base]) => {
            Ok(if opcode == "JMP" { 0xc000 } else { 0x4000 } | (register(base)? << 6))
        }
        ("JSR", [offset]) => Ok(0x4800 | relative_field(offset, 11, labels, address)?),
        ("LD" | "LDI" | "LEA" | "ST" | "STI", [register_operand, offset]) => {
            let base = match opcode {
                "LD" => 0x2000,
                "LDI" => 0xa000,
                "LEA" => 0xe000,
                "ST" => 0x3000,
                "STI" => 0xb000,
                _ => return Err(format!("unsupported opcode {opcode:?}")),
            };
            Ok(base
                | (register(register_operand)? << 9)
                | relative_field(offset, 9, labels, address)?)
        }
        ("LDR" | "STR", [register_operand, base, offset]) => {
            Ok(if opcode == "LDR" { 0x6000 } else { 0x7000 }
                | (register(register_operand)? << 9)
                | (register(base)? << 6)
                | signed_field(offset, 6)?)
        }
        (".FILL", [value]) => {
            if literal_parts(value).is_some() {
                literal(value, -32768, 65535)
            } else {
                label_address(value, labels)
            }
        }
        ("TRAP", [vector]) => Ok(0xf000 | literal(vector, 0, 255)?),
        ("GETC", []) => Ok(0xf020),
        ("OUT", []) => Ok(0xf021),
        ("PUTS", []) => Ok(0xf022),
        ("IN", []) => Ok(0xf023),
        ("PUTSP", []) => Ok(0xf024),
        ("HALT", []) => Ok(0xf025),
        ("RET", []) => Ok(0xc1c0),
        ("RTI", []) => Ok(0x8000),
        _ => Err(format!(
            "unsupported opcode or operands for {opcode:?}: {operands:?}"
        )),
    }
}

fn branch_conditions(opcode: &str) -> Option<u16> {
    match opcode {
        "BR" | "BRNZP" => Some(0x0e00),
        "BRN" => Some(0x0800),
        "BRZ" => Some(0x0400),
        "BRP" => Some(0x0200),
        "BRNZ" => Some(0x0c00),
        "BRNP" => Some(0x0a00),
        "BRZP" => Some(0x0600),
        _ => None,
    }
}

fn is_register(text: &str) -> bool {
    matches!(text.as_bytes(), [b'r' | b'R', b'0'..=b'7'])
}

fn register(text: &str) -> Result<u16, String> {
    match text.as_bytes() {
        [b'r' | b'R', digit @ b'0'..=b'7'] => Ok(u16::from(digit - b'0')),
        _ => Err(format!("invalid register {text:?}: expected R0 through R7")),
    }
}

fn signed_field(text: &str, bits: u32) -> Result<u16, String> {
    let limit = 1_i64 << (bits - 1);
    Ok(literal(text, -limit, limit - 1)? & ((1_u16 << bits) - 1))
}

fn literal_parts(text: &str) -> Option<(&str, u32)> {
    let (digits, radix) = if let Some(digits) = text.strip_prefix(['x', 'X']) {
        (digits, 16)
    } else {
        (text.strip_prefix('#').unwrap_or(text), 10)
    };
    let magnitude = digits.strip_prefix('-').unwrap_or(digits);
    if magnitude.is_empty()
        || !magnitude.bytes().all(|byte| {
            if radix == 16 {
                byte.is_ascii_hexdigit()
            } else {
                byte.is_ascii_digit()
            }
        })
    {
        return None;
    }
    Some((digits, radix))
}

fn literal(text: &str, minimum: i64, maximum: i64) -> Result<u16, String> {
    let (digits, radix) = literal_parts(text).ok_or_else(|| format!("invalid literal {text:?}"))?;
    let value =
        i64::from_str_radix(digits, radix).map_err(|_| format!("invalid literal {text:?}"))?;
    if !(minimum..=maximum).contains(&value) {
        return Err(format!("literal {text:?} outside {minimum}..={maximum}"));
    }
    Ok(value as u16)
}

fn diagnostic(line: u32, text: &str, reason: &str) -> Diagnostic {
    Diagnostic {
        severity: Severity::Error,
        line: Some(line),
        message: format!("line {line}: {reason}: {text:?}"),
    }
}
