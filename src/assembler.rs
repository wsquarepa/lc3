use std::collections::{HashMap, hash_map::Entry};

use crate::literal_text::{LiteralError, parse as parse_literal};
use crate::{Diagnostic, Image, Loaded, Severity};

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
    origin: Option<u16>,
    statements: Vec<Statement<'a>>,
    labels: HashMap<String, u16>,
    warnings: Vec<Diagnostic>,
    errors: Vec<Diagnostic>,
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
    assemble_source(source)
}

fn assemble_source(source: &str) -> Result<Loaded, Vec<Diagnostic>> {
    let layout = layout(source);
    let mut errors = layout.errors;
    let mut words = Vec::new();
    for statement in layout.statements {
        match statement.emission {
            Emission::Data(data) => words.extend(data),
            Emission::Instruction { opcode, operands } => {
                match encode_instruction(&opcode, operands, &layout.labels, statement.address) {
                    Ok(word) => words.push(word),
                    Err(reason) => errors.push(diagnostic(statement.line, statement.text, &reason)),
                }
            }
        }
    }
    if !errors.is_empty() {
        errors.sort_by_key(|error| error.line);
        return Err(errors);
    }
    let origin = layout
        .origin
        .ok_or_else(|| vec![diagnostic(1, source, "missing .ORIG")])?;
    Ok(Loaded {
        image: Image::new(origin, words),
        warnings: layout.warnings,
    })
}

fn layout(source: &str) -> Layout<'_> {
    let mut origin = None;
    let mut saw_origin = false;
    let mut address = 0;
    let mut labels = HashMap::new();
    let mut statements = Vec::new();
    let mut ended = false;
    let mut warnings = Vec::new();
    let mut errors = Vec::new();
    let mut warned_after_end = false;
    let mut last_line = 1;
    let mut last_text = "";
    let mut source_offset = 0;
    for (index, raw_line) in source.split_inclusive('\n').enumerate() {
        let source_tail = &source[source_offset..];
        source_offset += raw_line.len();
        let text = raw_line.strip_suffix('\n').unwrap_or(raw_line);
        let text = text.strip_suffix('\r').unwrap_or(text);
        let Ok(line) = u32::try_from(index + 1) else {
            errors.push(diagnostic(
                u32::MAX,
                text,
                "source exceeds the line number limit",
            ));
            break;
        };
        last_line = line;
        last_text = text;
        if ended {
            if !warned_after_end && !before_comment(text).trim_matches([' ', '\t']).is_empty() {
                warnings.push(Diagnostic {
                    severity: Severity::Warning,
                    ..diagnostic(line, text, "text after .END is ignored")
                });
                warned_after_end = true;
            }
            continue;
        }
        let parsed = match parse_line(text) {
            Ok(parsed) => parsed,
            Err(reason) => {
                errors.push(diagnostic(line, text, &reason));
                continue;
            }
        };
        if parsed
            .labels
            .first()
            .is_some_and(|label| label.eq_ignore_ascii_case("NOP"))
        {
            warnings.push(Diagnostic {
                severity: Severity::Warning,
                ..diagnostic(line, text, "NOP is a label and emits no word")
            });
        }
        for label in parsed.labels {
            if !saw_origin {
                errors.push(diagnostic(line, text, "label before .ORIG"));
            }
            match labels.entry(label.to_ascii_uppercase()) {
                Entry::Occupied(_) => {
                    errors.push(diagnostic(
                        line,
                        text,
                        &format!("duplicate label {label:?}"),
                    ));
                }
                Entry::Vacant(entry) => {
                    entry.insert(address);
                }
            }
        }
        match parsed.opcode.as_str() {
            "" => continue,
            ".ORIG" => {
                if saw_origin {
                    errors.push(diagnostic(line, text, "second .ORIG is not allowed"));
                }
                match literal(parsed.operands, 0, 65535) {
                    Ok(value) if !saw_origin => {
                        address = value;
                        origin = Some(value);
                    }
                    Ok(_) => {}
                    Err(reason) => errors.push(diagnostic(line, text, &reason)),
                }
                saw_origin = true;
                continue;
            }
            ".END" => {
                ended = true;
                if !parsed.operands.is_empty() {
                    errors.push(diagnostic(line, text, ".END takes no operands"));
                }
                continue;
            }
            _ => {}
        }
        if !saw_origin {
            errors.push(diagnostic(
                line,
                text,
                "instruction or directive before .ORIG",
            ));
        }
        let parsed_emission = match emission(parsed.opcode, parsed.operands, text, source_tail) {
            Ok(parsed_emission) => parsed_emission,
            Err(reason) => {
                errors.push(diagnostic(line, text, &reason));
                continue;
            }
        };
        let word_count = match &parsed_emission {
            Emission::Data(words) => words.len(),
            Emission::Instruction { .. } => 1,
        };
        statements.push(Statement {
            line,
            text,
            address,
            emission: parsed_emission,
        });
        address = address.wrapping_add(word_count as u16);
    }
    if !ended {
        errors.push(diagnostic(last_line, last_text, "missing .END"));
    }
    if !saw_origin {
        errors.push(diagnostic(last_line, last_text, "missing .ORIG"));
    }
    Layout {
        origin,
        statements,
        labels,
        warnings,
        errors,
    }
}

fn emission<'a>(
    opcode: String,
    operands: &'a str,
    text: &str,
    source_tail: &str,
) -> Result<Emission<'a>, String> {
    match opcode.as_str() {
        ".BLKW" => {
            let count = usize::from(literal(operands, 0, 65535)?);
            Ok(Emission::Data(vec![0; count]))
        }
        ".STRINGZ" => {
            let words = string_words(operands)?;
            let code_end = before_comment(text).trim_end_matches([' ', '\t']).len();
            if string_swallows_lines(operands, &source_tail[code_end..]) {
                return Err(
                    ".STRINGZ would swallow later lines in lc3as; remove the ambiguous closing backslash or later quote".to_owned(),
                );
            }
            Ok(Emission::Data(words))
        }
        _ => Ok(Emission::Instruction { opcode, operands }),
    }
}

fn parse_line(text: &str) -> Result<ParsedLine<'_>, String> {
    let mut remaining = before_comment(text)
        .trim_matches('\r')
        .trim_end_matches([' ', '\t']);
    let mut labels = Vec::new();
    while !remaining.is_empty() {
        let after_space = remaining.trim_start_matches([' ', '\t']);
        if after_space.len() < remaining.len() && is_stray_operand_format(after_space) {
            return Err(format!("operands without an opcode: {after_space:?}"));
        }
        remaining = after_space;
        let length = remaining
            .bytes()
            .enumerate()
            .take_while(|(index, byte)| {
                byte.is_ascii_alphanumeric() || *byte == b'_' || (*index == 0 && *byte == b'.')
            })
            .count();
        let token = &remaining[..length];
        let tail = &remaining[length..];
        if is_label(token)
            && let Some(tail) = tail.trim_start_matches([' ', '\t']).strip_prefix(':')
        {
            labels.push(token);
            remaining = tail;
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
                operands: tail.trim_matches([' ', '\t']),
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

fn is_stray_operand_format(text: &str) -> bool {
    // INITIAL's operand rules outmatch leading SPACE in lc3.f.
    // O_L is absent there, so ordinary label-only remainders are valid.
    let operands: Vec<_> = text
        .split(',')
        .map(|operand| operand.trim_matches([' ', '\t']))
        .collect();
    match operands.as_slice() {
        [operand] => is_register(operand) || is_literal(operand) || operand.starts_with('"'),
        [first, second] => {
            is_register(first) && (is_register(second) || is_literal(second) || is_label(second))
        }
        [first, second, third] => {
            is_register(first) && is_register(second) && (is_register(third) || is_literal(third))
        }
        _ => false,
    }
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

fn string_swallows_lines(string: &str, suffix: &str) -> bool {
    if !string
        .strip_suffix('"')
        .is_some_and(|content| content.ends_with('\\'))
    {
        return false;
    }
    let Some(newline) = suffix.find('\n') else {
        return false;
    };
    // lc3.f's STRING can treat any backslash-preceded quote as interior,
    // regardless of backslash parity. O_S wins the longest match across lines
    // when a later quote ends a line, but generation stops at the original quote.
    for (index, _) in suffix.match_indices('"') {
        if index > newline && string_quote_ends_line(&suffix[index + 1..]) {
            return true;
        }
        if !suffix[..index].ends_with('\\') {
            return false;
        }
    }
    false
}

fn string_quote_ends_line(suffix: &str) -> bool {
    let mut suffix = suffix.trim_start_matches([' ', '\t']);
    if suffix.starts_with(';') {
        let Some(end) = suffix.find(['\r', '\n']) else {
            return false;
        };
        suffix = &suffix[end..];
    }
    suffix.starts_with('\n') || suffix.starts_with("\r\n")
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
    if is_literal(text) {
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
        operands
            .split(',')
            .map(|operand| operand.trim_matches([' ', '\t']))
            .collect()
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
            if is_literal(value) {
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

fn is_literal(text: &str) -> bool {
    parse_literal(text) != Err(LiteralError::Malformed)
}

fn literal(text: &str, minimum: i64, maximum: i64) -> Result<u16, String> {
    let value = parse_literal(text).map_err(|_| format!("invalid literal {text:?}"))?;
    if !(minimum..=maximum).contains(&value) {
        return Err(format!("literal {text:?} outside {minimum}..={maximum}"));
    }
    Ok(value as u16)
}

fn diagnostic(line: u32, text: &str, reason: &str) -> Diagnostic {
    Diagnostic {
        severity: Severity::Error,
        line: Some(line),
        message: format!("{reason}: {text:?}"),
    }
}
