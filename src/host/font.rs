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

/// A small list of OpenType feature settings, as in CSS `font-feature-settings`
/// (for example `ss03` or `cv01` with value 2).
///
/// Setting a tag again replaces its earlier value. A value of 0 turns the feature off. The
/// list is fixed-size so that it stays `Copy`; settings beyond [`FontFeatures::CAPACITY`] are
/// dropped.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontFeatures {
    items: [([u8; 4], u32); FontFeatures::CAPACITY],
    len: u8,
}

impl FontFeatures {
    /// The maximum number of settings in the list.
    pub const CAPACITY: usize = 8;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Sets a feature. Returns `false` (and does nothing) if the list is full.
    pub fn set(&mut self, tag: [u8; 4], value: u32) -> bool {
        if let Some(item) = self.items[..self.len as usize]
            .iter_mut()
            .find(|(t, _)| *t == tag)
        {
            item.1 = value;
            return true;
        }
        if (self.len as usize) == Self::CAPACITY {
            return false;
        }
        self.items[self.len as usize] = (tag, value);
        self.len += 1;
        true
    }

    /// The settings in the order they were first set.
    pub fn iter(&self) -> impl Iterator<Item = ([u8; 4], u32)> + '_ {
        self.items[..self.len as usize].iter().copied()
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
    /// Additional features from the computed `font-feature-settings`, such as `ss03`. They
    /// override `ssty` and `flac` of the same tag.
    pub font_features: FontFeatures,
}

impl ShapeOptions {
    /// All GSUB features to apply, as (tag, value) with a non-zero value. Later settings
    /// override earlier ones of the same tag.
    pub(crate) fn gsub_features(&self) -> Vec<([u8; 4], u32)> {
        let mut all = FontFeatures::new();
        if self.script_level >= 1 {
            all.set(*b"ssty", self.script_level as u32);
        }
        if self.flat_accent {
            all.set(*b"flac", 1);
        }
        let mut result: Vec<([u8; 4], u32)> = all.iter().collect();
        for (tag, value) in self.font_features.iter() {
            match result.iter_mut().find(|(t, _)| *t == tag) {
                Some(item) => item.1 = value,
                None => result.push((tag, value)),
            }
        }
        result.retain(|&(_, value)| value != 0);
        result
    }
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

    /// Whether the font has a glyph for the character `c`, that is, whether it is in the font's
    /// character map. The default shapes the character and looks for glyph 0 (`.notdef`).
    fn has_glyph(&self, c: char) -> bool {
        self.shape(c.encode_utf8(&mut [0; 4]), ShapeOptions::default())
            .first()
            .is_some_and(|glyph| glyph.glyph_code != 0)
    }

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

    /// [`stretch`](Self::stretch), also returning the italic correction of the result, as the
    /// algorithm to shape a stretchy glyph of MathML Core sets it: that of the variant glyph (its
    /// `MathItalicsCorrectionInfo` entry), or for a glyph assembly the font's
    /// `GlyphAssembly.italicsCorrection`.
    ///
    /// The default implementation knows only the first case: it is the correction of a result of
    /// one glyph, and 0 for several glyphs. Fonts that can tell the correction of an assembly
    /// should override it.
    fn stretch_with_italic_correction(
        &self,
        glyph: GlyphCode,
        horizontal: bool,
        target_size: i32,
        options: ShapeOptions,
    ) -> (Vec<(Vector<i32>, MathGlyph)>, i32) {
        let parts = self.stretch(glyph, horizontal, target_size, options);
        let italic_correction = match parts.as_slice() {
            [(_, glyph)] => glyph.italic_correction,
            _ => 0,
        };
        (parts, italic_correction)
    }

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
            fn has_glyph(&self, c: char) -> bool {
                (**self).has_glyph(c)
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
            fn stretch_with_italic_correction(
                &self,
                glyph: GlyphCode,
                horizontal: bool,
                target_size: i32,
                options: ShapeOptions,
            ) -> (Vec<(Vector<i32>, MathGlyph)>, i32) {
                (**self).stretch_with_italic_correction(glyph, horizontal, target_size, options)
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
