#![no_main]

use frankenterm_core::Parser;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Feed arbitrary bytes to the VT/ANSI parser.
    // The parser must never panic regardless of input.
    let mut parser = Parser::new();
    let _ = parser.feed(data);

    // Also test incremental (byte-at-a-time) feeding for the same input,
    // which exercises UTF-8 split-feed and state machine edge cases.
    let mut parser2 = Parser::new();
    for &b in data {
        let _ = parser2.advance(b);
    }
});
