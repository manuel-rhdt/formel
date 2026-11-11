//! Best-effort substitutes for characters a font cannot draw, or cannot stretch.
//!
//! Math fonts differ a lot in coverage: Fira Math has no script, fraktur or sans-serif
//! alphabets, and many fonts have either the spacing or the combining form of an accent, but
//! not both. Instead of drawing `.notdef`, we fall back to the closest character the font has,
//! and draw bars the font cannot stretch as rules.

use crate::host::{MathFont, ShapeOptions};
use crate::typesetting::unicode_math::{Family, convert_character_to_family, split_family};

/// Characters that draw the same accent (spacing and combining forms), in order of
/// preference when standing in for `c`. Empty if `c` is not an accent.
fn equivalent_accents(c: char) -> &'static [char] {
    match c {
        '\u{2D9}' | '\u{307}' => &['\u{2D9}', '\u{307}'], // dot above
        '\u{A8}' | '\u{308}' => &['\u{A8}', '\u{308}'],   // diaeresis
        '\u{2C6}' | '\u{302}' | '^' => &['\u{2C6}', '\u{302}', '^'], // circumflex
        '\u{2DC}' | '\u{303}' | '~' => &['\u{2DC}', '\u{303}', '~'], // tilde
        '\u{2C7}' | '\u{30C}' => &['\u{2C7}', '\u{30C}'], // caron
        '\u{2D8}' | '\u{306}' => &['\u{2D8}', '\u{306}'], // breve
        '\u{2DA}' | '\u{30A}' => &['\u{2DA}', '\u{30A}'], // ring above
        '\u{B4}' | '\u{2CA}' | '\u{301}' => &['\u{B4}', '\u{2CA}', '\u{301}'], // acute
        '`' | '\u{2CB}' | '\u{300}' => &['\u{2CB}', '`', '\u{300}'], // grave
        // macron: the short bar
        '\u{AF}' | '\u{2C9}' | '\u{304}' => &['\u{AF}', '\u{2C9}', '\u{304}', '\u{305}', '\u{203E}'],
        // overline: the long bar
        '\u{203E}' | '\u{305}' => &['\u{203E}', '\u{305}', '\u{AF}', '\u{304}', '\u{2C9}'],
        '_' | '\u{332}' => &['_', '\u{332}'], // low line
        _ => &[],
    }
}

/// The families to try, in order, when the font lacks the characters of `family`: drop the
/// most unusual property first.
fn family_fallbacks(family: Family) -> &'static [Family] {
    use Family::*;
    match family {
        Normal => &[],
        Italics | Bold | Fraktur | SansSerif | Monospace => &[Normal],
        BoldItalics => &[Italics, Bold, Normal],
        Script => &[Italics, Normal],
        BoldScript => &[Script, BoldItalics, Italics, Normal],
        DoubleStruck => &[Bold, Normal],
        BoldFraktur => &[Fraktur, Bold, Normal],
        SansSerifBold => &[SansSerif, Bold, Normal],
        SansSerifItalics => &[SansSerif, Italics, Normal],
        SansSerifBoldItalics => &[
            SansSerifItalics,
            SansSerifBold,
            SansSerif,
            BoldItalics,
            Italics,
            Normal,
        ],
    }
}

/// The characters to try, in order, in place of `c`.
fn fallbacks(c: char) -> impl Iterator<Item = char> {
    let accents = equivalent_accents(c).iter().copied().filter(move |&e| e != c);
    let (base, family) = split_family(c);
    let letters = family_fallbacks(family)
        .iter()
        .map(move |&f| convert_character_to_family(base, f));
    accents.chain(letters)
}

fn has_glyph<F: MathFont + ?Sized>(font: &F, c: char, options: ShapeOptions) -> bool {
    let glyphs = font.shape(c.encode_utf8(&mut [0; 4]), options);
    !glyphs.is_empty() && glyphs.iter().all(|g| g.glyph_code != 0)
}

/// `text` with each character the font has no glyph for replaced by the first fallback it has.
/// `None` if nothing could be replaced.
pub(crate) fn substitute_missing<F: MathFont + ?Sized>(
    font: &F,
    text: &str,
    options: ShapeOptions,
) -> Option<String> {
    let mut changed = false;
    let substituted = text
        .chars()
        .map(|c| {
            if has_glyph(font, c, options) {
                return c;
            }
            match fallbacks(c).find(|&f| has_glyph(font, f, options)) {
                Some(f) => {
                    changed = true;
                    f
                }
                None => c,
            }
        })
        .collect();
    changed.then_some(substituted)
}

/// Whether `text` is a single horizontal bar, which can be drawn as a rule of any width when
/// the font cannot stretch it.
pub(crate) fn is_bar(text: &str) -> bool {
    matches!(
        single_char(text),
        Some(
            '\u{AF}' | '\u{2C9}' | '\u{304}' | '\u{305}' | '\u{203E}' | '_' | '\u{332}'
                | '\u{2015}'
        )
    )
}

fn single_char(text: &str) -> Option<char> {
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => Some(c),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_falls_back_to_italic_then_normal() {
        let script_l = convert_character_to_family('L', Family::Script);
        let chain: Vec<char> = fallbacks(script_l).collect();
        assert_eq!(
            chain,
            [convert_character_to_family('L', Family::Italics), 'L']
        );
    }

    #[test]
    fn accents_fall_back_to_equivalent_forms() {
        assert_eq!(fallbacks('\u{2D9}').collect::<Vec<_>>(), ['\u{307}']);
        assert_eq!(fallbacks('\u{307}').collect::<Vec<_>>(), ['\u{2D9}']);
        assert_eq!(fallbacks('+').count(), 0);
    }
}
