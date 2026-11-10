use harfbuzz_rs::sys as hb;
use std::cell::RefCell;
use std::cmp::min;

use harfbuzz_rs::{
    shape, Blob, Feature, Font, GlyphBuffer, GlyphInfo, GlyphPosition, HarfbuzzObject, Shared, Tag,
    UnicodeBuffer,
};
use harfbuzz_rs::{FontFuncs, Glyph};
use crate::host::{
    CornerPosition, Extents, GlyphCode, MathConstant, MathFont, MathGlyph, ShapeOptions, Vector,
};

/// The basic font structure used
#[derive(Debug)]
pub struct HarfbuzzShaper<'a> {
    pub font: Shared<Font<'a>>,
    pub no_cmap_font: Shared<Font<'a>>,
    buffer: RefCell<Option<UnicodeBuffer>>,
    math_table: Shared<Blob<'a>>,
}

pub struct IdentityFuncs;

impl FontFuncs for IdentityFuncs {
    fn get_nominal_glyph(&self, _font: &Font<'_>, unicode: char) -> Option<Glyph> {
        Some(unicode as Glyph)
    }
}

impl<'a> HarfbuzzShaper<'a> {
    pub fn new(font: Shared<Font>) -> HarfbuzzShaper {
        let buffer = Some(UnicodeBuffer::new()).into();
        let mut no_cmap_font = Font::create_sub_font(font.clone());
        no_cmap_font.set_font_funcs(IdentityFuncs);
        let math_table = font
            .face()
            .table_with_tag(b"MATH")
            .expect("MATH table must be present");
        HarfbuzzShaper {
            font,
            no_cmap_font: no_cmap_font.into(),
            buffer,
            math_table,
        }
    }

    /// The value of a MATH table constant.
    pub fn math_constant(&self, c: MathConstant) -> i32 {
        unsafe { hb::hb_ot_math_get_constant(self.font.as_raw(), c as _) }
    }

    /// The raw OpenType MATH table.
    pub fn math_table(&self) -> &[u8] {
        &self.math_table
    }

    fn shape_str(&self, string: &str, options: ShapeOptions) -> Vec<MathGlyph> {
        let buffer = self.buffer.borrow_mut().take().unwrap();
        *self.buffer.borrow_mut() = Some(buffer.add_str(string));
        self.do_shape(&self.font, options)
    }

    fn glyphs_from_index(&self, glyph_index: u32, options: ShapeOptions) -> Vec<MathGlyph> {
        let buffer = self.buffer.borrow_mut().take().unwrap();
        *self.buffer.borrow_mut() = Some(buffer.add(glyph_index, 0));
        self.do_shape(&self.no_cmap_font, options)
    }

    fn do_shape(&self, font: &Font, options: ShapeOptions) -> Vec<MathGlyph> {
        let mut features = Vec::with_capacity(2);
        if options.script_level >= 1 {
            let math_variants_tag = Tag::new('s', 's', 't', 'y');
            let variant_num = options.script_level as u32;

            features.push(Feature::new(math_variants_tag, variant_num, ..));
        }
        if options.flat_accent {
            features.push(Feature::new(Tag::from(b"flac"), 1, ..));
        }

        let buffer = self
            .buffer
            .borrow_mut()
            .take()
            .expect("Buffer not available");
        let glyph_buffer = shape(font, buffer.set_script(Tag::from(b"Math")), &features);
        let glyphs = self.layout_boxes(&glyph_buffer).collect();
        *self.buffer.borrow_mut() = Some(glyph_buffer.clear());

        glyphs
    }

    fn layout_boxes<'b>(
        &'b self,
        glyph_buffer: &'b GlyphBuffer,
    ) -> impl 'b + Iterator<Item = MathGlyph> {
        let positions = glyph_buffer.get_glyph_positions();
        let infos = glyph_buffer.get_glyph_infos();
        positions
            .iter()
            .zip(infos.iter())
            .map(move |(pos, info)| self.math_glyph(*pos, *info))
    }

    fn math_glyph(&self, pos: GlyphPosition, info: GlyphInfo) -> MathGlyph {
        let glyph = info.codepoint;
        let font = self.font.as_raw();
        let extents = self.font.get_glyph_extents(glyph).unwrap_or(hb::hb_glyph_extents_t {
            x_bearing: 0,
            y_bearing: 0,
            width: 0,
            height: 0,
        });
        MathGlyph {
            glyph_code: glyph,
            cluster: info.cluster,
            // harfbuzz' y axis points upwards
            offset: Vector {
                x: pos.x_offset,
                y: -pos.y_offset,
            },
            advance_width: pos.x_advance,
            extents: Extents {
                left_side_bearing: extents.x_bearing,
                width: extents.width,
                ascent: extents.y_bearing,
                descent: -(extents.height + extents.y_bearing),
            },
            italic_correction: unsafe { hb::hb_ot_math_get_glyph_italics_correction(font, glyph) },
            top_accent_attachment: unsafe {
                hb::hb_ot_math_get_glyph_top_accent_attachment(font, glyph)
            },
        }
    }

    fn stretch_parts(
        &self,
        glyph: u32,
        horizontal: bool,
        target_size: i32,
        options: ShapeOptions,
    ) -> Stretched {
        // MathML Core 5.3.2: the base glyph if it is big enough, else the first variant that
        // is, else an assembly, else the last thing that was tried (the largest variant).
        try_base_glyph(self, glyph, horizontal, target_size, options)
            .or_else(|| try_variant(self, glyph, horizontal, target_size, options))
            .map(Stretched::Single)
            .or_else(|| {
                try_assembly(self, glyph, horizontal, target_size, options).map(Stretched::Assembly)
            })
            .or_else(|| {
                largest_variant(self, glyph, horizontal, options).map(Stretched::Single)
            })
            .unwrap_or_else(|| Stretched::Single(self.glyphs_from_index(glyph, options)))
    }

    fn has_variants_or_assembly(&self, glyph: u32, horizontal: bool) -> bool {
        let direction = direction(horizontal);

        let variant_iter = VariantIterator {
            shaper: self,
            glyph,
            direction,
            index: 0,
        };

        if variant_iter.len() > 0 {
            return true;
        }

        let assembly_iter = AssemblyIterator {
            shaper: self,
            glyph,
            direction,
            index: 0,
        };

        assembly_iter.len() > 0
    }
}

/// The result of stretching a glyph: either a single glyph (a variant) or an assembly of
/// parts, each with its offset from the first part.
enum Stretched {
    Single(Vec<MathGlyph>),
    Assembly(Vec<(Vector<i32>, Vec<MathGlyph>)>),
}

fn direction(horizontal: bool) -> hb::hb_direction_t {
    if horizontal {
        hb::HB_DIRECTION_LTR
    } else {
        hb::HB_DIRECTION_TTB
    }
}

fn point_with_offset(offset: i32, horizontal: bool) -> Vector<i32> {
    if horizontal {
        Vector { x: offset, y: 0 }
    } else {
        Vector { x: 0, y: offset }
    }
}

impl<'a> MathFont for HarfbuzzShaper<'a> {
    fn units_per_em(&self) -> u32 {
        self.font.face().upem()
    }

    fn constant(&self, c: MathConstant) -> i32 {
        self.math_constant(c)
    }

    fn shape(&self, text: &str, options: ShapeOptions) -> Vec<MathGlyph> {
        self.shape_str(text, options)
    }

    fn glyph(&self, glyph: GlyphCode, options: ShapeOptions) -> MathGlyph {
        self.glyphs_from_index(glyph, options)
            .first()
            .copied()
            .unwrap_or_default()
    }

    fn is_stretchable(&self, glyph: GlyphCode, horizontal: bool) -> bool {
        self.has_variants_or_assembly(glyph, horizontal)
    }

    fn stretch(
        &self,
        glyph: GlyphCode,
        horizontal: bool,
        target_size: i32,
        options: ShapeOptions,
    ) -> Vec<(Vector<i32>, MathGlyph)> {
        match self.stretch_parts(glyph, horizontal, target_size, options) {
            Stretched::Single(glyphs) => glyphs
                .into_iter()
                .scan(0, |advance, glyph| {
                    let offset = Vector { x: *advance, y: 0 };
                    *advance += glyph.advance_width;
                    Some((offset, glyph))
                })
                .collect(),
            Stretched::Assembly(parts) => parts
                .into_iter()
                .flat_map(|(origin, glyphs)| glyphs.into_iter().map(move |g| (origin, g)))
                .collect(),
        }
    }

    fn math_kerning(
        &self,
        glyph: GlyphCode,
        corner: CornerPosition,
        correction_height: i32,
    ) -> i32 {
        unsafe {
            hb::hb_ot_math_get_glyph_kerning(
                self.font.as_raw(),
                glyph,
                std::mem::transmute(corner),
                correction_height,
            )
        }
    }
}

fn try_base_glyph(
    shaper: &HarfbuzzShaper,
    glyph: u32,
    horizontal: bool,
    target_size: i32,
    options: ShapeOptions,
) -> Option<Vec<MathGlyph>> {
    let glyph = shaper.glyphs_from_index(glyph, options)[0];

    let advance = if horizontal {
        glyph.advance_width
    } else {
        -glyph.extents.height()
    };

    if advance >= target_size {
        Some(vec![glyph])
    } else {
        None
    }
}

#[derive(Debug, Copy, Clone)]
struct VariantIterator<'a> {
    shaper: &'a HarfbuzzShaper<'a>,
    glyph: u32,
    direction: hb::hb_direction_t,
    index: u32,
}

impl<'a> Iterator for VariantIterator<'a> {
    type Item = hb::hb_ot_math_glyph_variant_t;

    fn next(&mut self) -> Option<hb::hb_ot_math_glyph_variant_t> {
        let mut glyph_variant: hb::hb_ot_math_glyph_variant_t = unsafe { ::std::mem::zeroed() };
        let mut num_elements: u32 = 1;
        unsafe {
            hb::hb_ot_math_get_glyph_variants(
                self.shaper.font.as_raw(),
                self.glyph,
                self.direction,
                self.index,
                &mut num_elements,
                &mut glyph_variant,
            )
        };
        self.index += 1;
        if num_elements == 1 {
            Some(glyph_variant)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let total_variants = unsafe {
            hb::hb_ot_math_get_glyph_variants(
                self.shaper.font.as_raw(),
                self.glyph,
                self.direction,
                self.index,
                &mut 0,
                std::ptr::null_mut(),
            )
        } as usize;
        let remaining_elements = total_variants - self.index as usize;
        (remaining_elements, Some(remaining_elements))
    }
}

impl<'a> ExactSizeIterator for VariantIterator<'a> {}

/// The first size variant that is at least `target_size` long.
fn try_variant(
    shaper: &HarfbuzzShaper,
    glyph: u32,
    horizontal: bool,
    target_size: i32,
    options: ShapeOptions,
) -> Option<Vec<MathGlyph>> {
    let mut variants = VariantIterator {
        shaper,
        glyph,
        direction: direction(horizontal),
        index: 0,
    };
    let variant = variants.find(|variant| variant.advance >= target_size)?;
    Some(shaper.glyphs_from_index(variant.glyph, options))
}

/// The last size variant of the glyph, which is the largest one.
fn largest_variant(
    shaper: &HarfbuzzShaper,
    glyph: u32,
    horizontal: bool,
    options: ShapeOptions,
) -> Option<Vec<MathGlyph>> {
    let variants = VariantIterator {
        shaper,
        glyph,
        direction: direction(horizontal),
        index: 0,
    };
    let variant = variants.last()?;
    Some(shaper.glyphs_from_index(variant.glyph, options))
}

struct AssemblyIterator<'a> {
    shaper: &'a HarfbuzzShaper<'a>,
    glyph: u32,
    direction: hb::hb_direction_t,
    index: u32,
}

impl<'a> Iterator for AssemblyIterator<'a> {
    type Item = hb::hb_ot_math_glyph_part_t;

    fn next(&mut self) -> Option<hb::hb_ot_math_glyph_part_t> {
        let mut glyph_part: hb::hb_ot_math_glyph_part_t = unsafe { ::std::mem::zeroed() };
        let mut num_elements: u32 = 1;
        let mut italics_correction: i32 = 0;
        unsafe {
            hb::hb_ot_math_get_glyph_assembly(
                self.shaper.font.as_raw(),
                self.glyph,
                self.direction,
                self.index,
                &mut num_elements,
                &mut glyph_part,
                &mut italics_correction,
            )
        };
        self.index += 1;
        if num_elements == 1 {
            Some(glyph_part)
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let total_parts = unsafe {
            hb::hb_ot_math_get_glyph_assembly(
                self.shaper.font.as_raw(),
                self.glyph,
                self.direction,
                self.index,
                &mut 0,
                std::ptr::null_mut(),
                &mut 0,
            )
        } as usize;
        let remaining_elements = total_parts - self.index as usize;
        (remaining_elements, Some(remaining_elements))
    }
}

impl<'a> ExactSizeIterator for AssemblyIterator<'a> {}

fn try_assembly(
    shaper: &HarfbuzzShaper,
    glyph: u32,
    horizontal: bool,
    target_size: i32,
    options: ShapeOptions,
) -> Option<Vec<(Vector<i32>, Vec<MathGlyph>)>> {
    let min_connector_overlap: i32 = 0;

    let mut assembly_iter = AssemblyIterator {
        shaper,
        glyph,
        direction: direction(horizontal),
        index: 0,
    };

    let mut full_advance_sum_non_ext: i32 = 0;
    let mut full_advance_sum_ext: i32 = 0;
    let mut part_count_non_ext: u32 = 0;
    let mut part_count_ext: u32 = 0;

    for part in &mut assembly_iter {
        if part.flags == hb::HB_OT_MATH_GLYPH_PART_FLAG_EXTENDER {
            full_advance_sum_ext += part.full_advance;
            part_count_ext += 1;
        } else {
            full_advance_sum_non_ext += part.full_advance;
            part_count_non_ext += 1;
        }
    }

    let a = full_advance_sum_non_ext - min_connector_overlap * (part_count_non_ext as i32 - 1);
    let b = full_advance_sum_ext - min_connector_overlap * part_count_ext as i32;
    if b == 0 {
        // there probably is no glyph assembly for this glyph
        return None;
    };
    let repeat_count_ext = ((target_size - a) as f32 / b as f32).ceil().max(0.0) as u32;

    // Total number of parts needed to assemble the glyph including repetitions of extenders.
    let part_count = part_count_non_ext + part_count_ext * repeat_count_ext;

    if part_count == 0 || part_count > 2000 {
        return None;
    }

    let connector_overlap = if part_count >= 2 {
        // First determine the ideal overlap that would get closest to the target
        // size. The following quotient is integer operation and gives the best
        // lower approximation of the actual value with fractional pixels.
        let c = full_advance_sum_non_ext + repeat_count_ext as i32 * full_advance_sum_ext;
        let mut connector_overlap = (c - target_size) / (part_count as i32 - 1);

        // We now consider the constraints on connectors. In general, only the
        // start of the first part and then end of the last part are not connected
        // so it is the minimum of StartConnector_i for all i > 0 and of
        // EndConnector_i for all i < glyphAssembly.part_record_count()-1. However,
        // if the first or last part is an extender then it will be connected too
        // with a copy of itself.
        //
        assembly_iter.index = 0;
        for (index, part) in assembly_iter.by_ref().enumerate() {
            let will_be_repeated =
                repeat_count_ext >= 2 && part.flags == hb::HB_OT_MATH_GLYPH_PART_FLAG_EXTENDER;
            if index < (part_count_ext + part_count_non_ext - 1) as usize || will_be_repeated {
                connector_overlap = min(connector_overlap, part.end_connector_length);
            }
            if index > 0 || will_be_repeated {
                connector_overlap = min(connector_overlap, part.start_connector_length);
            }
        }
        if connector_overlap < min_connector_overlap {
            return None;
        };
        connector_overlap
    } else {
        0
    };

    assembly_iter.index = 0;
    let result = assembly_iter
        // Repeat the extenders `repeat_count_ext` times .
        .flat_map(move |part| {
            let repeat_count = if part.flags == hb::HB_OT_MATH_GLYPH_PART_FLAG_EXTENDER {
                repeat_count_ext
            } else {
                1
            } as usize;
            std::iter::repeat(part).take(repeat_count)
        })
        // Offset the each glyph from the previous glyph by the advance of the part minus the
        // connector overlap.
        .scan(/* initial offset */ 0, move |current_offset, part| {
            let delta_offset = part.full_advance - connector_overlap;
            let origin = point_with_offset(*current_offset, horizontal);
            let glyphs = shaper.glyphs_from_index(part.glyph, options);

            if horizontal {
                *current_offset += delta_offset;
            } else {
                *current_offset -= delta_offset;
            }
            Some((origin, glyphs))
        });

    Some(result.collect())
}

#[cfg(test)]
mod test {

    #[test]
    fn test_assembly() {}
}
