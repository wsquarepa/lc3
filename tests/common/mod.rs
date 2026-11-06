use lc3::{Case, Format, Halted, Image, Outcome, load, prepare, run};
use std::num::NonZeroU32;

pub fn image(origin: u16, words: &[u16]) -> Image {
    let bytes: Vec<u8> = std::iter::once(origin)
        .chain(words.iter().copied())
        .flat_map(u16::to_be_bytes)
        .collect();
    load(Format::Obj, &bytes).unwrap().image
}

pub fn os() -> Image {
    // TRAP x25 points to x0210. Boot at x0200 and HALT at x0210 each
    // execute STI R5,#0 through an adjacent xFFFE pointer. R5 starts zero;
    // these routines preserve registers and condition codes.
    let mut words = vec![0; 0x0212 - 0x0025];
    words[0] = 0x0210;
    words[0x0200 - 0x0025] = 0xba00;
    words[0x0201 - 0x0025] = 0xfffe;
    words[0x0210 - 0x0025] = 0xba00;
    words[0x0211 - 0x0025] = 0xfffe;
    image(0x0025, &words)
}

pub fn halted(origin: u16, words: &[u16], n: u32) -> Halted {
    let prepared = prepare(&os(), &[image(origin, words)], NonZeroU32::new(n).unwrap()).unwrap();
    match run(&prepared, &Case::default()) {
        Outcome::Halted(halted) => halted,
        outcome => panic!("expected halted, got {outcome:?}"),
    }
}
