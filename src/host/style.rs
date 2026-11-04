//! The computed CSS values mlayout needs from the host for every node.

use super::font::{MathConstant, MathFont};
use super::geometry::Edges;
pub use crate::mathmlparser::Form;

/// The CSS `math-style` property.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MathStyle {
    /// Display style (`math-style: normal`): large operators, tall fractions.
    #[default]
    Normal,
    /// Inline style (`math-style: compact`): flatter layout to fit into a line of text.
    Compact,
}

/// The CSS `math-shift` property.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MathShift {
    #[default]
    Normal,
    /// "Cramped" style: superscripts are raised less (denominators, subscripts, radicands).
    Compact,
}

/// The CSS `direction` property.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    #[default]
    Ltr,
    /// Reserved; right-to-left math layout is not implemented yet.
    Rtl,
}

/// Computed style of a node, as resolved by the host's CSS engine.
///
/// mlayout never derives these values itself: script levels, font sizes, display/inline style
/// and cramping all come from here. That keeps the host's stylesheet the single source of
/// truth.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MathComputedStyle {
    /// Computed `font-size` in px. It already includes any `font-size: math` scaling
    /// (see [`script_scale_factor`]).
    pub font_size: f32,
    /// Computed `math-depth`. Only used to select script glyph variants (OpenType `ssty`). It
    /// never scales anything by itself.
    pub math_depth: i32,
    pub math_style: MathStyle,
    pub math_shift: MathShift,
    pub direction: Direction,
    /// Used padding in px. The host resolves percentages.
    pub padding: Edges<f32>,
    /// Used border widths in px (0 for `border-style: none`).
    pub border: Edges<f32>,
    /// Used margins in px. `auto` must be resolved to 0.
    ///
    /// The margins of the node passed to [`layout_math`](super::layout_math) are ignored; the
    /// host places that box itself.
    pub margin: Edges<f32>,
}

impl Default for MathComputedStyle {
    fn default() -> Self {
        MathComputedStyle {
            font_size: 16.0,
            math_depth: 0,
            math_style: MathStyle::Normal,
            math_shift: MathShift::Normal,
            direction: Direction::Ltr,
            padding: Edges::default(),
            border: Edges::default(),
            margin: Edges::default(),
        }
    }
}

/// Operator properties that override the MathML operator dictionary.
///
/// In MathML these are attributes of `<mo>`. A host may also take them from CSS (e.g. a
/// custom `stretchy` property). `None` means "use the operator dictionary".
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub struct OperatorOverrides {
    pub form: Option<Form>,
    pub stretchy: Option<bool>,
    pub symmetric: Option<bool>,
    pub largeop: Option<bool>,
    pub movablelimits: Option<bool>,
    /// in px
    pub lspace: Option<f32>,
    /// in px
    pub rspace: Option<f32>,
    /// in px
    pub minsize: Option<f32>,
    /// in px
    pub maxsize: Option<f32>,
}

/// Parses a MathML/CSS length such as `"0.5em"`, `"3px"` or `"thinmathspace"` into px.
///
/// Percentages and unitless numbers return `None`: what they are relative to depends on the
/// attribute.
pub fn parse_math_length(value: &str, style: &MathComputedStyle) -> Option<f32> {
    let value = value.trim();
    let em = style.font_size;
    let named = match value {
        "veryverythinmathspace" => Some(1.0),
        "verythinmathspace" => Some(2.0),
        "thinmathspace" => Some(3.0),
        "mediummathspace" => Some(4.0),
        "thickmathspace" => Some(5.0),
        "verythickmathspace" => Some(6.0),
        "veryverythickmathspace" => Some(7.0),
        "negativeveryverythinmathspace" => Some(-1.0),
        "negativeverythinmathspace" => Some(-2.0),
        "negativethinmathspace" => Some(-3.0),
        "negativemediummathspace" => Some(-4.0),
        "negativethickmathspace" => Some(-5.0),
        "negativeverythickmathspace" => Some(-6.0),
        "negativeveryverythickmathspace" => Some(-7.0),
        _ => None,
    };
    if let Some(eighteenths) = named {
        return Some(eighteenths / 18.0 * em);
    }

    let split = value
        .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+'))
        .unwrap_or(value.len());
    let (number, unit) = value.split_at(split);
    let number: f32 = number.parse().ok()?;
    let px = match unit.trim() {
        "px" => number,
        "em" => number * em,
        // Without access to the font's x-height, approximate 1ex as 0.5em.
        "ex" => number * em * 0.5,
        "in" => number * 96.0,
        "cm" => number * 96.0 / 2.54,
        "mm" => number * 96.0 / 25.4,
        "pt" => number * 96.0 / 72.0,
        "pc" => number * 16.0,
        "" if number == 0.0 => 0.0,
        _ => return None,
    };
    Some(px)
}

/// The factor by which `font-size: math` scales the font size when `math-depth` changes from
/// `parent_depth` to `depth` (CSS Fonts 4 / MathML Core).
///
/// It uses the font's `ScriptPercentScaleDown` and `ScriptScriptPercentScaleDown` constants
/// (0.71 and 0.5041 if the font doesn't set them). Every step beyond script-script level scales
/// by a further 0.71.
pub fn script_scale_factor<F: MathFont + ?Sized>(font: &F, parent_depth: i32, depth: i32) -> f32 {
    let percent = |c, default: f32| {
        let value = font.constant(c);
        if value > 0 {
            value as f32 / 100.0
        } else {
            default
        }
    };
    let a = percent(MathConstant::ScriptPercentScaleDown, 0.71);
    let b = percent(MathConstant::ScriptScriptPercentScaleDown, 0.5041);

    // Scale of depth `level` relative to depth 0.
    let scale_at = |level: i32| -> f32 {
        match level {
            l if l <= 0 => 0.71f32.powi(l),
            1 => a,
            l => b * 0.71f32.powi(l - 2),
        }
    };
    scale_at(depth) / scale_at(parent_depth)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths() {
        let style = MathComputedStyle {
            font_size: 18.0,
            ..Default::default()
        };
        assert_eq!(parse_math_length("2em", &style), Some(36.0));
        assert_eq!(parse_math_length(" 3px ", &style), Some(3.0));
        assert_eq!(parse_math_length("thinmathspace", &style), Some(3.0));
        assert_eq!(parse_math_length("-1em", &style), Some(-18.0));
        assert_eq!(parse_math_length("0", &style), Some(0.0));
        assert_eq!(parse_math_length("50%", &style), None);
        assert_eq!(parse_math_length("2", &style), None);
    }
}
