//! The font interface that math layout needs. Values are in font design units; the layout
//! engine converts them to px with each node's `font-size`.

use std::ops::{Add, Sub};
use std::rc::Rc;
use std::sync::Arc;

/// An identifier of a glyph inside a font.
pub type GlyphCode = u32;

/// A point or offset in font units. The y coordinate increases downwards.
#[derive(Debug, Default, PartialEq, Eq, Clone, Copy, Hash)]
pub struct Vector<T> {
    pub x: T,
    pub y: T,
}

impl Add for Vector<i32> {
    type Output = Vector<i32>;
    fn add(self, rhs: Vector<i32>) -> Vector<i32> {
        Vector {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Sub for Vector<i32> {
    type Output = Vector<i32>;
    fn sub(self, rhs: Vector<i32>) -> Vector<i32> {
        Vector {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

/// The ink extents of a glyph, in font units.
#[derive(Debug, Default, PartialEq, Eq, Clone, Copy, Hash)]
pub struct Extents<T> {
    /// Horizontal offset of the ink from the glyph origin.
    pub left_side_bearing: T,
    pub width: T,
    /// Maximum extent above the baseline.
    pub ascent: T,
    /// Maximum extent below the baseline.
    pub descent: T,
}

impl Extents<i32> {
    pub fn new(left_side_bearing: i32, width: i32, ascent: i32, descent: i32) -> Self {
        Extents {
            left_side_bearing,
            width,
            ascent,
            descent,
        }
    }

    /// `ascent + descent`
    pub fn height(&self) -> i32 {
        self.ascent + self.descent
    }
}

/// The constants of the OpenType MATH table, in table order.
#[derive(Debug, Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
#[repr(C)]
pub enum MathConstant {
    ScriptPercentScaleDown = 0,
    ScriptScriptPercentScaleDown,
    DelimitedSubFormulaMinHeight,
    DisplayOperatorMinHeight,
    MathLeading,
    AxisHeight,
    AccentBaseHeight,
    FlattenedAccentBaseHeight,
    SubscriptShiftDown,
    SubscriptTopMax,
    SubscriptBaselineDropMin,
    SuperscriptShiftUp,
    SuperscriptShiftUpCramped,
    SuperscriptBottomMin,
    SuperscriptBaselineDropMax,
    SubSuperscriptGapMin,
    SuperscriptBottomMaxWithSubscript,
    SpaceAfterScript,
    UpperLimitGapMin,
    UpperLimitBaselineRiseMin,
    LowerLimitGapMin,
    LowerLimitBaselineDropMin,
    StackTopShiftUp,
    StackTopDisplayStyleShiftUp,
    StackBottomShiftDown,
    StackBottomDisplayStyleShiftDown,
    StackGapMin,
    StackDisplayStyleGapMin,
    StretchStackTopShiftUp,
    StretchStackBottomShiftDown,
    StretchStackGapAboveMin,
    StretchStackGapBelowMin,
    FractionNumeratorShiftUp,
    FractionNumeratorDisplayStyleShiftUp,
    FractionDenominatorShiftDown,
    FractionDenominatorDisplayStyleShiftDown,
    FractionNumeratorGapMin,
    FractionNumDisplayStyleGapMin,
    FractionRuleThickness,
    FractionDenominatorGapMin,
    FractionDenomDisplayStyleGapMin,
    SkewedFractionHorizontalGap,
    SkewedFractionVerticalGap,
    OverbarVerticalGap,
    OverbarRuleThickness,
    OverbarExtraAscender,
    UnderbarVerticalGap,
    UnderbarRuleThickness,
    UnderbarExtraDescender,
    RadicalVerticalGap,
    RadicalDisplayStyleVerticalGap,
    RadicalRuleThickness,
    RadicalExtraAscender,
    RadicalKernBeforeDegree,
    RadicalKernAfterDegree,
    RadicalDegreeBottomRaisePercent,
}

/// A structure that describes an individual glyph in a font.
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct MathGlyph {
    /// The font-specific glyph code
    pub glyph_code: u32,
    /// the utf-8 offset into the field that generated this glyph
    pub cluster: u32,
    /// The Offset at which the glyph should be rendered.
    pub offset: Vector<i32>,
    /// The glyph's advance width.
    pub advance_width: i32,
    /// The exact dimensions of the glyph's outline.
    pub extents: Extents<i32>,
    /// The italic correction to apply after this glyph.
    pub italic_correction: i32,
    /// The x-coordinate where a top accent should be attached.
    pub top_accent_attachment: i32,
}

/// Possible positions of multiscripts relative to the base. The values match HarfBuzz'
/// `hb_ot_math_kern_t`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum CornerPosition {
    /// Prescript top
    TopLeft = 1,
    /// Superscript position
    TopRight = 0,
    /// Prescript bottom
    BottomLeft = 3,
    /// Subscript position
    BottomRight = 2,
}

impl CornerPosition {
    /// Returns true if the position is left of the base
    pub fn is_left(self) -> bool {
        use CornerPosition::*;
        match self {
            TopLeft | BottomLeft => true,
            _ => false,
        }
    }

    /// Returns true if the position is right of the base
    pub fn is_top(self) -> bool {
        use CornerPosition::*;
        match self {
            TopLeft | TopRight => true,
            _ => false,
        }
    }

    /// Returns the position that is horizontally "on the other side".
    pub fn horizontal_mirror(self) -> Self {
        use CornerPosition::*;
        match self {
            TopLeft => TopRight,
            TopRight => TopLeft,
            BottomLeft => BottomRight,
            BottomRight => BottomLeft,
        }
    }

    /// Returns the position that is vertically opposite.
    pub fn vertical_mirror(self) -> Self {
        use CornerPosition::*;
        match self {
            TopLeft => BottomLeft,
            TopRight => BottomRight,
            BottomLeft => TopLeft,
            BottomRight => TopRight,
        }
    }

    /// Returns the position that is both horizontally and vertically mirrored.
    pub fn diagonal_mirror(self) -> Self {
        use CornerPosition::*;
        match self {
            TopLeft => BottomRight,
            TopRight => BottomLeft,
            BottomLeft => TopRight,
            BottomRight => TopLeft,
        }
    }
}

/// Options that affect glyph selection when shaping.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShapeOptions {
    /// Script level for the OpenType `ssty` feature: 0 (off), 1 or 2. Derived from
    /// `math-depth`.
    pub script_level: u8,
    /// Use flattened accent forms (OpenType `flac`).
    pub flat_accent: bool,
}

/// A font with an OpenType MATH table.
///
/// All lengths are in font design units ([`units_per_em`](MathFont::units_per_em) per em).
pub trait MathFont {
    fn units_per_em(&self) -> u32;

    /// The value of a MATH table constant. Most are lengths in font units; the `*Percent*`
    /// constants are percentages.
    fn constant(&self, c: MathConstant) -> i32;

    /// Shapes `text` with the `math` script.
    fn shape(&self, text: &str, options: ShapeOptions) -> Vec<MathGlyph>;

    /// Metrics for a glyph by index (no cmap lookup).
    fn glyph(&self, glyph: GlyphCode, options: ShapeOptions) -> MathGlyph;

    /// Whether the font has size variants or a glyph assembly for `glyph` in the given
    /// direction.
    fn is_stretchable(&self, glyph: GlyphCode, horizontal: bool) -> bool;

    /// Returns a variant or assembly of `glyph` that covers `target_size` font units in the
    /// given direction, as glyphs with offsets relative to the first one, following the
    /// algorithm to shape a stretchy glyph of MathML Core (5.3.2): the base glyph if its advance
    /// is at least the target, else the first size variant that is, else a glyph assembly, else
    /// the largest variant. Falls back to the base glyph, so the result is never empty.
    fn stretch(
        &self,
        glyph: GlyphCode,
        horizontal: bool,
        target_size: i32,
        options: ShapeOptions,
    ) -> Vec<(Vector<i32>, MathGlyph)>;

    /// The MATH table kerning of `glyph` at `corner` for the given correction height.
    fn math_kerning(&self, glyph: GlyphCode, corner: CornerPosition, correction_height: i32)
    -> i32;
}

macro_rules! forward_math_font {
    ($($ptr:ty),*) => {$(
        impl<F: MathFont + ?Sized> MathFont for $ptr {
            fn units_per_em(&self) -> u32 {
                (**self).units_per_em()
            }
            fn constant(&self, c: MathConstant) -> i32 {
                (**self).constant(c)
            }
            fn shape(&self, text: &str, options: ShapeOptions) -> Vec<MathGlyph> {
                (**self).shape(text, options)
            }
            fn glyph(&self, glyph: GlyphCode, options: ShapeOptions) -> MathGlyph {
                (**self).glyph(glyph, options)
            }
            fn is_stretchable(&self, glyph: GlyphCode, horizontal: bool) -> bool {
                (**self).is_stretchable(glyph, horizontal)
            }
            fn stretch(
                &self,
                glyph: GlyphCode,
                horizontal: bool,
                target_size: i32,
                options: ShapeOptions,
            ) -> Vec<(Vector<i32>, MathGlyph)> {
                (**self).stretch(glyph, horizontal, target_size, options)
            }
            fn math_kerning(
                &self,
                glyph: GlyphCode,
                corner: CornerPosition,
                correction_height: i32,
            ) -> i32 {
                (**self).math_kerning(glyph, corner, correction_height)
            }
        }
    )*};
}

forward_math_font!(&F, Rc<F>, Arc<F>, Box<F>);
