//! Tables: `<mtable>`, `<mtr>`, `<mlabeledtr>` and `<mtd>`, for matrices, cases and aligned
//! equations.
//!
//! The layout follows the MathML 3 table attributes: `columnalign`, `rowalign`,
//! `columnspacing`, `rowspacing`, `columnlines`, `rowlines`, `frame`, `framespacing` and
//! `align`. List-valued attributes repeat their last value. Cell spanning is not supported.

use super::lbox::{Content, LBox, Metrics};
use super::{Box_, Ctx, Engine};
use crate::host::{
    Form, MathComputedStyle, MathConstant, MathElement, MathLayoutHost, Point, parse_math_length,
};

#[derive(Debug, Clone, Copy, PartialEq)]
enum ColumnAlign {
    Left,
    Center,
    Right,
}

impl ColumnAlign {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "left" => Some(ColumnAlign::Left),
            "center" => Some(ColumnAlign::Center),
            "right" => Some(ColumnAlign::Right),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum RowAlign {
    Top,
    Bottom,
    Center,
    /// `baseline` and `axis` (cells share a font, so their axes line up when their baselines
    /// do).
    Baseline,
}

impl RowAlign {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "top" => Some(RowAlign::Top),
            "bottom" => Some(RowAlign::Bottom),
            "center" => Some(RowAlign::Center),
            "baseline" | "axis" => Some(RowAlign::Baseline),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum LineStyle {
    None,
    Solid,
    Dashed,
}

impl LineStyle {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "none" => Some(LineStyle::None),
            "solid" => Some(LineStyle::Solid),
            "dashed" => Some(LineStyle::Dashed),
            _ => None,
        }
    }
}

/// The `i`th entry of a MathML list attribute; the last entry repeats.
fn nth<T: Copy>(items: &[T], i: usize, default: T) -> T {
    items.get(i).or(items.last()).copied().unwrap_or(default)
}

struct Cell<B> {
    lbox: B,
    column_align: ColumnAlign,
    row_align: RowAlign,
}

struct Row<Id, B> {
    node: Option<Id>,
    cells: Vec<Cell<B>>,
    ascent: f32,
    descent: f32,
}

impl<'t, T: MathLayoutHost> Engine<'t, T> {
    /// A whitespace-separated list attribute, parsed item by item. Invalid items are dropped.
    fn list_attribute<V>(
        &self,
        node: T::NodeId,
        name: &str,
        parse: impl Fn(&str) -> Option<V>,
    ) -> Vec<V> {
        self.tree
            .attribute(node, name)
            .map(|value| value.split_whitespace().filter_map(&parse).collect())
            .unwrap_or_default()
    }

    pub(crate) fn layout_table(&mut self, node: T::NodeId, style: &MathComputedStyle) -> Box_<T> {
        let length = |value: &str| parse_math_length(value, style);
        let column_aligns = self.list_attribute(node, "columnalign", ColumnAlign::parse);
        let row_aligns = self.list_attribute(node, "rowalign", RowAlign::parse);
        let mut column_spacing = self.list_attribute(node, "columnspacing", length);
        if column_spacing.is_empty() {
            column_spacing.push(0.8 * style.font_size);
        }
        let mut row_spacing = self.list_attribute(node, "rowspacing", length);
        if row_spacing.is_empty() {
            // 1.0ex, approximated as 0.5em
            row_spacing.push(0.5 * style.font_size);
        }
        let column_lines = self.list_attribute(node, "columnlines", LineStyle::parse);
        let row_lines = self.list_attribute(node, "rowlines", LineStyle::parse);
        let frame = self
            .tree
            .attribute(node, "frame")
            .and_then(|value| LineStyle::parse(value.trim()))
            .unwrap_or(LineStyle::None);
        let frame_spacing = self.list_attribute(node, "framespacing", length);

        let font = self.font(node, style);
        let thickness = font.constant(MathConstant::FractionRuleThickness);
        let axis_height = font.constant(MathConstant::AxisHeight);

        // Lay out the cells. Children that aren't rows form a row of their own; children of a
        // row that aren't cells are cells themselves.
        let mut rows: Vec<Row<T::NodeId, Box_<T>>> = Vec::new();
        for (i, child) in self.children(node).into_iter().enumerate() {
            let (row_node, cell_nodes) = match self.element(child) {
                Some(MathElement::Mtr) => (Some(child), self.children(child)),
                Some(MathElement::Mlabeledtr) => {
                    (Some(child), self.children(child).into_iter().skip(1).collect())
                }
                _ => (None, vec![child]),
            };
            let row_column_aligns = row_node
                .map(|r| self.list_attribute(r, "columnalign", ColumnAlign::parse))
                .unwrap_or_default();
            let row_row_align = row_node
                .and_then(|r| self.list_attribute(r, "rowalign", RowAlign::parse).first().copied())
                .unwrap_or(nth(&row_aligns, i, RowAlign::Baseline));

            let mut cells = Vec::with_capacity(cell_nodes.len());
            for (j, cell_node) in cell_nodes.into_iter().enumerate() {
                let column_align = self
                    .list_attribute(cell_node, "columnalign", ColumnAlign::parse)
                    .first()
                    .copied()
                    .or(row_column_aligns.get(j).or(row_column_aligns.last()).copied())
                    .unwrap_or(nth(&column_aligns, j, ColumnAlign::Center));
                let row_align = self
                    .list_attribute(cell_node, "rowalign", RowAlign::parse)
                    .first()
                    .copied()
                    .unwrap_or(row_row_align);
                let lbox = self.layout_node(cell_node, Ctx::plain(Form::Infix));
                cells.push(Cell {
                    lbox,
                    column_align,
                    row_align,
                });
            }
            rows.push(Row {
                node: row_node,
                cells,
                ascent: 0.0,
                descent: 0.0,
            });
        }

        // row heights: baseline-aligned cells share the baseline; the others only need room
        for row in &mut rows {
            for cell in row.cells.iter().filter(|c| c.row_align == RowAlign::Baseline) {
                row.ascent = row.ascent.max(cell.lbox.metrics.ascent);
                row.descent = row.descent.max(cell.lbox.metrics.descent);
            }
            for cell in row.cells.iter().filter(|c| c.row_align != RowAlign::Baseline) {
                let missing = cell.lbox.metrics.height() - (row.ascent + row.descent);
                if missing > 0.0 {
                    if row.ascent + row.descent == 0.0 {
                        // no baseline cells: put the baseline in the middle
                        row.ascent = missing / 2.0;
                        row.descent = missing / 2.0;
                    } else {
                        row.descent += missing;
                    }
                }
            }
        }

        let column_count = rows.iter().map(|r| r.cells.len()).max().unwrap_or(0);
        let mut column_widths = vec![0.0f32; column_count];
        for row in &rows {
            for (j, cell) in row.cells.iter().enumerate() {
                column_widths[j] = column_widths[j].max(cell.lbox.metrics.advance);
            }
        }

        // horizontal and vertical positions, measured from the table's top left corner
        let has_frame = frame != LineStyle::None;
        let frame_thickness = if has_frame { thickness } else { 0.0 };
        let (frame_space_x, frame_space_y) = if has_frame {
            (
                frame_spacing.first().copied().unwrap_or(0.4 * style.font_size),
                frame_spacing.get(1).copied().unwrap_or(0.25 * style.font_size),
            )
        } else {
            (0.0, 0.0)
        };
        let mut column_x = Vec::with_capacity(column_count);
        let mut x = frame_thickness + frame_space_x;
        for (j, width) in column_widths.iter().enumerate() {
            column_x.push(x);
            x += width;
            if j + 1 < column_count {
                x += nth(&column_spacing, j, 0.0);
            }
        }
        let width = x + frame_space_x + frame_thickness;

        let mut row_baselines = Vec::with_capacity(rows.len());
        let mut y = frame_thickness + frame_space_y;
        for (i, row) in rows.iter().enumerate() {
            row_baselines.push(y + row.ascent);
            y += row.ascent + row.descent;
            if i + 1 < rows.len() {
                y += nth(&row_spacing, i, 0.0);
            }
        }
        let height = y + frame_space_y + frame_thickness;

        // vertical placement of the table relative to the surrounding baseline
        let align = self
            .tree
            .attribute(node, "align")
            .and_then(|value| value.split_whitespace().next().map(str::to_owned));
        let top = match align.as_deref() {
            Some("top") => 0.0,
            Some("bottom") => -height,
            Some("center") => -height / 2.0,
            // axis (default) and baseline: center the table on the math axis
            _ => -axis_height - height / 2.0,
        };

        // Lines run through the middle of the gaps between rows and columns.
        let mut children = Vec::with_capacity(rows.len() + column_count + 4);
        let mut push_line = |x: f32, y: f32, w: f32, h: f32, style: LineStyle| {
            if style != LineStyle::None && w > 0.0 && h > 0.0 {
                let mut line = LBox::rect(w, h, style == LineStyle::Dashed);
                line.origin = Point::new(x, top + y + h);
                children.push(line);
            }
        };
        let inner_top = frame_thickness;
        let inner_height = height - 2.0 * frame_thickness;
        for j in 0..column_count.saturating_sub(1) {
            let gap = nth(&column_spacing, j, 0.0);
            let line_x = column_x[j] + column_widths[j] + gap / 2.0 - thickness / 2.0;
            push_line(
                line_x,
                inner_top,
                thickness,
                inner_height,
                nth(&column_lines, j, LineStyle::None),
            );
        }
        let inner_left = frame_thickness;
        let inner_width = width - 2.0 * frame_thickness;
        for i in 0..rows.len().saturating_sub(1) {
            let gap = nth(&row_spacing, i, 0.0);
            let row_bottom = row_baselines[i] + rows[i].descent;
            push_line(
                inner_left,
                row_bottom + gap / 2.0 - thickness / 2.0,
                inner_width,
                thickness,
                nth(&row_lines, i, LineStyle::None),
            );
        }
        push_line(0.0, 0.0, width, thickness, frame);
        push_line(0.0, height - thickness, width, thickness, frame);
        push_line(0.0, 0.0, thickness, height, frame);
        push_line(width - thickness, 0.0, thickness, height, frame);

        // Rows span the table; cells fill their column and row.
        for (row, baseline) in rows.into_iter().zip(row_baselines) {
            let cells = row
                .cells
                .into_iter()
                .enumerate()
                .map(|(j, cell)| {
                    let m = cell.lbox.metrics;
                    let dx = match cell.column_align {
                        ColumnAlign::Left => 0.0,
                        ColumnAlign::Center => (column_widths[j] - m.advance) / 2.0,
                        ColumnAlign::Right => column_widths[j] - m.advance,
                    };
                    let dy = match cell.row_align {
                        RowAlign::Baseline => 0.0,
                        RowAlign::Top => -row.ascent + m.ascent,
                        RowAlign::Bottom => row.descent - m.descent,
                        RowAlign::Center => {
                            (row.descent - row.ascent) / 2.0 - (m.descent - m.ascent) / 2.0
                        }
                    };
                    let mut cell_box =
                        fill_cell(cell.lbox, column_widths[j], row.ascent, row.descent, dx, dy);
                    cell_box.origin = Point::new(column_x[j], 0.0);
                    cell_box
                })
                .collect();
            let mut row_box = LBox::boxes(cells);
            row_box.metrics = box_metrics(width, row.ascent, row.descent);
            row_box.origin = Point::new(0.0, top + baseline);
            row_box.node = row.node;
            children.push(row_box);
        }

        let mut table = LBox::boxes(children);
        table.metrics = box_metrics(width, -top, top + height);
        table
    }
}

fn box_metrics(width: f32, ascent: f32, descent: f32) -> Metrics {
    Metrics {
        advance: width,
        ascent,
        descent,
        ink_left: 0.0,
        ink_width: width,
        italic_correction: 0.0,
        top_accent_attachment: width / 2.0,
    }
}

/// Grows a cell's box to the full cell (`width` by `ascent` + `descent`) and moves its content
/// by (`dx`, `dy`) inside it.
fn fill_cell<Id: Copy>(
    mut cell: LBox<Id>,
    width: f32,
    ascent: f32,
    descent: f32,
    dx: f32,
    dy: f32,
) -> LBox<Id> {
    match &mut cell.content {
        Content::Boxes(children) => {
            for child in children {
                child.origin.x += dx;
                child.origin.y += dy;
            }
        }
        _ => {
            // Move the content into a child box so that it can be shifted.
            let content = std::mem::replace(&mut cell.content, Content::Empty);
            let mut inner = LBox::boxes(Vec::new());
            inner.content = content;
            inner.metrics = cell.metrics;
            inner.origin = Point::new(dx, dy);
            cell.content = Content::Boxes(vec![inner]);
        }
    }
    cell.metrics = box_metrics(width, ascent, descent);
    cell
}
