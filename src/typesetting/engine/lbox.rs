//! The engine's internal box tree. All values are in px. Each box is positioned by its origin:
//! the left end of its baseline, relative to the parent box's origin (y down).

use crate::host::{Edges, GlyphCode, MathFont, MathGlyph, Point, Vector};

/// Metrics of a laid-out box.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub(crate) struct Metrics {
    /// Distance from this box's origin to the origin of the box that follows it.
    pub advance: f32,
    pub ascent: f32,
    pub descent: f32,
    /// Horizontal ink extent, relative to the origin.
    pub ink_left: f32,
    pub ink_width: f32,
    /// Extra space to add after this box if the next box is upright.
    pub italic_correction: f32,
    /// x coordinate at which a top accent should be centered.
    pub top_accent_attachment: f32,
}

impl Metrics {
    pub fn height(&self) -> f32 {
        self.ascent + self.descent
    }

    pub fn ink_center(&self) -> f32 {
        self.ink_left + self.ink_width / 2.0
    }

    pub fn ink_right(&self) -> f32 {
        self.ink_left + self.ink_width
    }
}

/// A run of glyphs from the font of node `owner`.
#[derive(Debug, Clone)]
pub(crate) struct GlyphRun<Id> {
    pub owner: Id,
    /// px per font unit
    pub scale: f32,
    pub font_size: f32,
    /// Pen position of every glyph in font units. The glyph is drawn at `pen + glyph.offset`.
    pub glyphs: Vec<(Vector<i32>, MathGlyph)>,
}

#[derive(Debug, Clone)]
pub(crate) enum Content<Id> {
    Empty,
    Glyphs(GlyphRun<Id>),
    /// A horizontal rule of the given width, centered vertically on the baseline.
    Rule { width: f32, thickness: f32 },
    /// A filled rectangle extending `height` above the origin and `width` to the right.
    Rect { width: f32, height: f32, dashed: bool },
    Boxes(Vec<LBox<Id>>),
}

#[derive(Debug, Clone)]
pub(crate) struct LBox<Id> {
    pub origin: Point,
    pub metrics: Metrics,
    pub content: Content<Id>,
    /// Set if this box is the margin box of a host node.
    pub node: Option<Id>,
    /// The node's margins; `metrics` include them.
    pub margin: Edges<f32>,
    /// Lay out but don't paint the contents (`<mphantom>`).
    pub phantom: bool,
}

impl<Id: Copy> LBox<Id> {
    fn with_content(content: Content<Id>, metrics: Metrics) -> Self {
        LBox {
            origin: Point::ZERO,
            metrics,
            content,
            node: None,
            margin: Edges::default(),
            phantom: false,
        }
    }

    pub fn empty(advance: f32, ascent: f32, descent: f32) -> Self {
        LBox::with_content(
            Content::Empty,
            Metrics {
                advance,
                ascent,
                descent,
                ink_left: 0.0,
                ink_width: advance,
                italic_correction: 0.0,
                top_accent_attachment: advance / 2.0,
            },
        )
    }

    pub fn rule(width: f32, thickness: f32) -> Self {
        LBox::with_content(
            Content::Rule { width, thickness },
            Metrics {
                advance: width,
                ascent: thickness / 2.0,
                descent: thickness / 2.0,
                ink_left: 0.0,
                ink_width: width,
                italic_correction: 0.0,
                top_accent_attachment: width / 2.0,
            },
        )
    }

    /// A filled rectangle with its bottom left corner at the origin.
    pub fn rect(width: f32, height: f32, dashed: bool) -> Self {
        LBox::with_content(
            Content::Rect {
                width,
                height,
                dashed,
            },
            Metrics {
                advance: width,
                ascent: height,
                descent: 0.0,
                ink_left: 0.0,
                ink_width: width,
                italic_correction: 0.0,
                top_accent_attachment: width / 2.0,
            },
        )
    }

    pub fn glyphs(run: GlyphRun<Id>) -> Self {
        let s = run.scale;
        let mut advance = 0;
        let mut ascent = None::<i32>;
        let mut descent = None::<i32>;
        let mut ink_left = None::<i32>;
        let mut ink_right = None::<i32>;
        for (pen, glyph) in &run.glyphs {
            let x = pen.x + glyph.offset.x;
            let y = pen.y + glyph.offset.y;
            advance = advance.max(pen.x + glyph.advance_width);
            ascent = Some(ascent.map_or(i32::MIN, |a| a).max(glyph.extents.ascent - y));
            descent = Some(descent.map_or(i32::MIN, |d| d).max(glyph.extents.descent + y));
            let left = x + glyph.extents.left_side_bearing;
            ink_left = Some(ink_left.map_or(left, |l| l.min(left)));
            let right = left + glyph.extents.width;
            ink_right = Some(ink_right.map_or(right, |r| r.max(right)));
        }
        let ink_left = ink_left.unwrap_or(0);
        let italic_correction = run
            .glyphs
            .last()
            .map_or(0, |(_, glyph)| glyph.italic_correction);
        let accent = match run.glyphs.as_slice() {
            [(pen, glyph)] if glyph.top_accent_attachment != 0 => {
                (pen.x + glyph.top_accent_attachment) as f32 * s
            }
            _ => advance as f32 * s / 2.0,
        };
        let metrics = Metrics {
            advance: advance as f32 * s,
            ascent: ascent.unwrap_or(0) as f32 * s,
            descent: descent.unwrap_or(0) as f32 * s,
            ink_left: ink_left as f32 * s,
            ink_width: (ink_right.unwrap_or(0) - ink_left) as f32 * s,
            italic_correction: italic_correction as f32 * s,
            top_accent_attachment: accent,
        };
        LBox::with_content(Content::Glyphs(run), metrics)
    }

    /// A box containing `children`, with metrics aggregated from them.
    pub fn boxes(children: Vec<LBox<Id>>) -> Self {
        let mut metrics = Metrics::default();
        let mut first = true;
        for child in &children {
            let m = &child.metrics;
            let (x, y) = (child.origin.x, child.origin.y);
            if first {
                metrics.advance = x + m.advance;
                metrics.ascent = m.ascent - y;
                metrics.descent = m.descent + y;
                metrics.ink_left = x + m.ink_left;
                metrics.ink_width = m.ink_width;
                first = false;
            } else {
                let ink_right = metrics.ink_right().max(x + m.ink_right());
                metrics.advance = metrics.advance.max(x + m.advance);
                metrics.ascent = metrics.ascent.max(m.ascent - y);
                metrics.descent = metrics.descent.max(m.descent + y);
                metrics.ink_left = metrics.ink_left.min(x + m.ink_left);
                metrics.ink_width = ink_right - metrics.ink_left;
            }
        }
        metrics.italic_correction = children
            .last()
            .map_or(0.0, |child| child.metrics.italic_correction);
        metrics.top_accent_attachment = match children.as_slice() {
            [child] => child.origin.x + child.metrics.top_accent_attachment,
            _ => metrics.advance / 2.0,
        };
        LBox::with_content(Content::Boxes(children), metrics)
    }

    /// Wraps `self`, moved down by `dy`, into a new box at the origin so that the shift is part
    /// of the metrics.
    pub fn shifted_down(mut self, dy: f32) -> Self {
        if dy == 0.0 {
            return self;
        }
        self.origin.y += dy;
        LBox::boxes(vec![self])
    }

    /// The first glyph in this box (searching the first child recursively), with the run's
    /// owner and scale.
    pub fn first_glyph(&self) -> Option<(MathGlyph, Id, f32)> {
        match &self.content {
            Content::Glyphs(run) => run.glyphs.first().map(|(_, g)| (*g, run.owner, run.scale)),
            Content::Boxes(children) => children.first().and_then(LBox::first_glyph),
            _ => None,
        }
    }

    pub fn last_glyph(&self) -> Option<(MathGlyph, Id, f32)> {
        match &self.content {
            Content::Glyphs(run) => run.glyphs.last().map(|(_, g)| (*g, run.owner, run.scale)),
            Content::Boxes(children) => children.last().and_then(LBox::last_glyph),
            _ => None,
        }
    }
}

/// A font scaled to a node's `font-size`.
pub(crate) struct ScaledFont<F> {
    pub font: F,
    pub font_size: f32,
    /// px per font unit
    pub scale: f32,
}

impl<F: MathFont> ScaledFont<F> {
    pub fn new(font: F, font_size: f32) -> Self {
        let upem = font.units_per_em().max(1) as f32;
        ScaledFont {
            font,
            font_size,
            scale: font_size / upem,
        }
    }

    /// A MATH table length constant in px.
    pub fn constant(&self, c: crate::host::MathConstant) -> f32 {
        self.font.constant(c) as f32 * self.scale
    }

    /// Converts px to font units.
    pub fn units(&self, px: f32) -> i32 {
        (px / self.scale).round() as i32
    }

    /// A glyph run from glyphs in shaping order: each pen position follows the previous
    /// glyph's advance.
    pub fn run_from_shaped<Id>(&self, owner: Id, glyphs: Vec<MathGlyph>) -> GlyphRun<Id> {
        let mut pen = 0;
        let glyphs = glyphs
            .into_iter()
            .map(|glyph| {
                let position = Vector { x: pen, y: 0 };
                pen += glyph.advance_width;
                (position, glyph)
            })
            .collect();
        self.run(owner, glyphs)
    }

    pub fn run<Id>(&self, owner: Id, glyphs: Vec<(Vector<i32>, MathGlyph)>) -> GlyphRun<Id> {
        GlyphRun {
            owner,
            scale: self.scale,
            font_size: self.font_size,
            glyphs,
        }
    }

    pub fn math_kerning(
        &self,
        glyph: GlyphCode,
        corner: crate::host::CornerPosition,
        height_px: f32,
    ) -> f32 {
        self.font.math_kerning(glyph, corner, self.units(height_px)) as f32 * self.scale
    }
}
