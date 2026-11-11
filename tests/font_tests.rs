
mod util;

use formel::host::MathConstant;
use crate::util::TEST_FONT;

#[test]
fn constants_test() {
    TEST_FONT.with(|font| {
        let latin_moder_consts = [70i32, 50, 1300, 1300, 154, 250, 450, 664, 247, 344, 200, 363,
                                  289, 108, 250, 160, 344, 56, 200, 111, 167, 600, 444, 677, 345,
                                  686, 120, 280, 111, 600, 200, 167, 394, 677, 345, 686, 40, 120,
                                  40, 40, 120, 350, 96, 120, 40, 40, 120, 40, 40, 50, 148, 40, 40,
                                  278, -556, 60];
        for (num, latin_const) in latin_moder_consts.iter().enumerate() {
            let math_const: MathConstant = unsafe { ::std::mem::transmute(num as u32) };
            let value = font.math_constant(math_const);
            println!("constant num {:?}, named: {:?}; expected value: {:?}, computed value: {:?}",
                     num,
                     math_const,
                     *latin_const,
                     value);
            assert!(value == *latin_const);
        }
    })
}

/// A glyph assembly is as long as asked (MathML Core rounds the overlap of the parts down to a
/// whole unit, which makes it up to `parts - 1` units longer; TeX does not): the vector arrow of
/// Latin Modern Math is a left end of 205 units, extenders of 137 and a right end of 205.
#[test]
fn glyph_assembly_is_as_long_as_the_target() {
    use formel::host::{MathFont, ShapeOptions};
    TEST_FONT.with(|font| {
        let arrow = font.shape("\u{20D7}", ShapeOptions::default())[0].glyph_code;
        for target in [1405, 1500, 2000, 3001] {
            let parts = font.stretch(arrow, true, target, ShapeOptions::default());
            assert!(parts.len() > 3, "an assembly for {target}");
            let (offset, last) = parts.last().unwrap();
            let length = offset.x + last.advance_width;
            assert!((length - target).abs() <= 1, "asked for {target}, got {length}");
        }
    })
}
