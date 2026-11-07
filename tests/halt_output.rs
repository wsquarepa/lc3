use lc3::{Case, Format, Halted, Image, MemoryPoke, Outcome, Pokes, Trigger, load, prepare, run};
use std::fs;
use std::io::{self, Write};
use std::num::NonZeroU32;
use std::path::Path;

fn assemble(source: &str) -> io::Result<Image> {
    load(Format::Asm, source.as_bytes())
        .map(|loaded| loaded.image)
        .map_err(|diagnostics| io::Error::other(format!("assembly failed: {diagnostics:?}")))
}

fn output_os() -> io::Result<Image> {
    assemble(
        ".ORIG x0024
         .FILL UNDEFINED_TRAP
         .FILL HALT_ROUTINE
         .BLKW x01DA
         BOOT AND R5,R5,#0
         STI R5,MCR
         MCR .FILL xFFFE
         UNDEFINED_TRAP LD R0,UNDEFINED_BYTE
         STI R0,DISPLAY
         BRnzp HALT_ROUTINE
         UNDEFINED_BYTE .FILL x0055
         DISPLAY .FILL xFE06
         HALT_ROUTINE LD R0,HALT_BYTE
         STI R0,DISPLAY
         STI R5,MCR
         HALT_BYTE .FILL x0048
         .END",
    )
}

fn run_halted(os: &Image, program: &Image, case: &Case) -> io::Result<Halted> {
    let prepared = prepare(os, std::slice::from_ref(program), NonZeroU32::MIN)
        .map_err(|error| io::Error::other(format!("preparation failed: {error:?}")))?;
    match run(&prepared, case) {
        Outcome::Halted(halted) => Ok(halted),
        Outcome::Failed(reason) => Err(io::Error::other(format!("run failed: {reason:?}"))),
    }
}

#[test]
fn undefined_trap_message_precedes_halt_output_when_the_os_branches_to_halt() -> io::Result<()> {
    let program = assemble(
        ".ORIG x3000
         LD R0,MARKER
         STI R0,DISPLAY
         TRAP x24
         MARKER .FILL x0021
         DISPLAY .FILL xFE06
         .END",
    )?;
    let result = run_halted(&output_os()?, &program, &Case::default())?;
    assert_eq!(result.program_output, b"!U");
    assert_eq!(result.halt_output, b"H");
    Ok(())
}

#[test]
fn direct_jsr_into_the_os_halt_routine_starts_halt_output() -> io::Result<()> {
    let os = output_os()?;
    let halt_address = os.words()[1];
    let origin = 0x0300u16;
    let offset = i32::from(halt_address) - i32::from(origin) - 1;
    let program = assemble(&format!(".ORIG x{origin:04X}\nJSR #{offset}\n.END"))?;
    let result = run_halted(&os, &program, &Case::default())?;
    assert!(result.program_output.is_empty());
    assert_eq!(result.halt_output, b"H");
    Ok(())
}

#[test]
fn halt_address_uses_case_pokes_before_the_first_trigger_and_fetch() -> io::Result<()> {
    let program = assemble(
        ".ORIG x3000
         LD R0,MARKER
         STI R0,DISPLAY
         JSR ALTERNATE_HALT
         ALTERNATE_HALT STI R0,DISPLAY
         STI R5,MCR
         MARKER .FILL x0021
         DISPLAY .FILL xFE06
         MCR .FILL xFFFE
         .END",
    )?;
    let pokes = Pokes {
        memory: vec![MemoryPoke {
            address: 0x0025,
            words: vec![0x3003],
        }],
        ..Pokes::default()
    };
    for (case, program_output, halt_output) in [
        (Case::default(), &b"!!"[..], &b""[..]),
        (
            Case {
                pokes: pokes.clone(),
                ..Case::default()
            },
            &b"!"[..],
            &b"!"[..],
        ),
        (
            Case {
                triggers: vec![Trigger { pc: 0x3000, pokes }],
                ..Case::default()
            },
            &b"!!"[..],
            &b""[..],
        ),
    ] {
        let result = run_halted(&output_os()?, &program, &case)?;
        assert_eq!(result.program_output, program_output);
        assert_eq!(result.halt_output, halt_output);
    }
    Ok(())
}

#[test]
fn program_vector_stores_do_not_change_the_captured_halt_address() -> io::Result<()> {
    let os = output_os()?;
    let halt_address = os.words()[1];
    let program = assemble(&format!(
        ".ORIG x3000
         LD R0,MARKER
         STI R0,DISPLAY
         LEA R1,ALTERNATE_HALT
         STI R1,HALT_VECTOR
         JSR ALTERNATE_HALT
         ALTERNATE_HALT STI R0,DISPLAY
         LD R1,ORIGINAL_HALT
         JMP R1
         MARKER .FILL x0021
         DISPLAY .FILL xFE06
         HALT_VECTOR .FILL x0025
         ORIGINAL_HALT .FILL x{halt_address:04X}
         .END"
    ))?;
    let result = run_halted(&os, &program, &Case::default())?;
    assert_eq!(result.program_output, b"!!");
    assert_eq!(result.halt_output, b"H");
    Ok(())
}

#[test]
fn halt_trap_starts_halt_output_without_fetching_the_captured_halt_address() -> io::Result<()> {
    let program = assemble(
        ".ORIG x3000
         LEA R0,ALTERNATE_HALT
         STI R0,HALT_VECTOR
         TRAP x25
         ALTERNATE_HALT LD R0,HALT_BYTE
         STI R0,DISPLAY
         STI R5,MCR
         HALT_VECTOR .FILL x0025
         HALT_BYTE .FILL x0048
         DISPLAY .FILL xFE06
         MCR .FILL xFFFE
         .END",
    )?;
    let result = run_halted(&output_os()?, &program, &Case::default())?;
    assert!(result.program_output.is_empty());
    assert_eq!(result.halt_output, b"H");
    Ok(())
}

#[test]
fn direct_mcr_halt_without_either_event_has_empty_halt_output() -> io::Result<()> {
    let program = assemble(
        ".ORIG x3000
         LD R0,MARKER
         STI R0,DISPLAY
         STI R5,MCR
         MARKER .FILL x0021
         DISPLAY .FILL xFE06
         MCR .FILL xFFFE
         .END",
    )?;
    let result = run_halted(&output_os()?, &program, &Case::default())?;
    assert_eq!(result.program_output, b"!");
    assert!(result.halt_output.is_empty());
    Ok(())
}

#[test]
fn reference_os_undefined_trap_message_precedes_halt_output() -> io::Result<()> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("reference/lc3tools/lc3os.obj");
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            writeln!(
                std::io::stderr(),
                "skipping: reference OS not found at {}",
                path.display()
            )?;
            return Ok(());
        }
        Err(error) => {
            return Err(io::Error::new(
                error.kind(),
                format!("cannot read reference OS at {}: {error}", path.display()),
            ));
        }
    };
    let os = load(Format::Obj, &bytes)
        .map_err(|diagnostics| {
            io::Error::other(format!(
                "cannot load reference OS at {}: {diagnostics:?}",
                path.display()
            ))
        })?
        .image;
    let program = assemble(
        ".ORIG x3000
         LD R0,MARKER
         OUT
         TRAP x26
         MARKER .FILL x0021
         .END",
    )?;
    let result = run_halted(&os, &program, &Case::default())?;
    assert_eq!(
        result.program_output,
        b"!\n\n--- undefined trap executed ---\n\n"
    );
    assert_eq!(result.halt_output, b"\n\n--- halting the LC-3 ---\n\n");
    Ok(())
}
