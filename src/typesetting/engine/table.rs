//! Tables: `<mtable>`, `<mtr>`, `<mlabeledtr>` and `<mtd>`, for matrices, cases and aligned
//! equations.
//!
//! The layout follows the MathML 3 table attributes: `columnalign`, `rowalign`,
//! `columnspacing`, `rowspacing`, `columnlines`, `rowlines`, `frame`, `framespacing` and
//! `align`. List-valued attributes repeat their last value. Cells may span rows and columns with
//! `rowspan` and `columnspan`; table lines stop at the cells that span them.

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
    /// The grid position of the cell's top left corner.
    row: usize,
    column: usize,
    row_span: usize,
    column_span: usize,
}

struct Row<Id> {
    node: Option<Id>,
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
        let mut rows: Vec<Row<T::NodeId>> = Vec::new();
        let mut cells: Vec<Cell<Box_<T>>> = Vec::new();
        // grid slots taken by cells that span several rows or columns
        let mut occupied = std::collections::HashSet::new();
        let table_children = self.children(node);
        let total_rows = table_children.len();
        for (i, child) in table_children.into_iter().enumerate() {
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

            let mut column = 0;
            for cell_node in cell_nodes {
                while occupied.contains(&(i, column)) {
                    column += 1;
                }
                let column_align = self
                    .list_attribute(cell_node, "columnalign", ColumnAlign::parse)
                    .first()
                    .copied()
                    .or(row_column_aligns.get(column).or(row_column_aligns.last()).copied())
                    .unwrap_or(nth(&column_aligns, column, ColumnAlign::Center));
                let row_align = self
                    .list_attribute(cell_node, "rowalign", RowAlign::parse)
                    .first()
                    .copied()
                    .unwrap_or(row_row_align);
                let span = |name| {
                    self.tree
                        .attribute(cell_node, name)
                        .and_then(|value| value.trim().parse::<usize>().ok())
                };
                // `rowspan="0"` spans all remaining rows. Spans can't reach past the last row.
                let row_span = match span("rowspan") {
                    Some(0) => total_rows - i,
                    Some(n) => n.min(total_rows - i),
                    None => 1,
                };
                let column_span = span("columnspan").filter(|&n| n > 0).unwrap_or(1);
                for r in i..i + row_span {
                    for c in column..column + column_span {
                        occupied.insert((r, c));
                    }
                }
                let lbox = self.layout_node(cell_node, Ctx::plain(Form::Infix));
                cells.push(Cell {
                    lbox,
                    column_align,
                    row_align,
                    row: i,
                    column,
                    row_span,
                    column_span,
                });
                column += column_span;
            }
            rows.push(Row {
                node: row_node,
                ascent: 0.0,
                descent: 0.0,
            });
        }

        // row heights: baseline-aligned cells share the baseline; the others only need room
        for (i, row) in rows.iter_mut().enumerate() {
            let in_row = || cells.iter().filter(|c| c.row == i && c.row_span == 1);
            for cell in in_row().filter(|c| c.row_align == RowAlign::Baseline) {
                row.ascent = row.ascent.max(cell.lbox.metrics.ascent);
                row.descent = row.descent.max(cell.lbox.metrics.descent);
            }
            for cell in in_row().filter(|c| c.row_align != RowAlign::Baseline) {
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
        // Cells spanning rows: the rows they cover share any missing height.
        let mut spanning: Vec<&Cell<Box_<T>>> = cells.iter().filter(|c| c.row_span > 1).collect();
        spanning.sort_by_key(|c| c.row_span);
        for cell in spanning {
            let covered = cell.row..cell.row + cell.row_span;
            let available: f32 = covered.clone().map(|r| rows[r].ascent + rows[r].descent).sum::<f32>()
                + covered.clone().take(cell.row_span - 1).map(|r| nth(&row_spacing, r, 0.0)).sum::<f32>();
            let missing = cell.lbox.metrics.height() - available;
            if missing > 0.0 {
                for r in covered {
                    rows[r].descent += missing / cell.row_span as f32;
                }
            }
        }

        let column_count = cells.iter().map(|c| c.column + c.column_span).max().unwrap_or(0);
        let mut column_widths = vec![0.0f32; column_count];
        for cell in cells.iter().filter(|c| c.column_span == 1) {
            column_widths[cell.column] = column_widths[cell.column].max(cell.lbox.metrics.advance);
        }
        let mut spanning: Vec<&Cell<Box_<T>>> = cells.iter().filter(|c| c.column_span > 1).collect();
        spanning.sort_by_key(|c| c.column_span);
        for cell in spanning {
            let covered = cell.column..cell.column + cell.column_span;
            let available: f32 = covered.clone().map(|c| column_widths[c]).sum::<f32>()
                + covered
                    .clone()
                    .take(cell.column_span - 1)
                    .map(|c| nth(&column_spacing, c, 0.0))
                    .sum::<f32>();
            let missing = cell.lbox.metrics.advance - available;
            if missing > 0.0 {
                for c in covered {
                    column_widths[c] += missing / cell.column_span as f32;
                }
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
        let inner_left = frame_thickness;
        let inner_width = width - 2.0 * frame_thickness;
        // The bands of the table that rows and columns occupy, from the middle of the gap above
        // (left of) each to the middle of the gap below (right of) it. They tile the inner area.
        let row_bands: Vec<(f32, f32)> = (0..rows.len())
            .map(|i| {
                let top = if i == 0 {
                    inner_top
                } else {
                    row_baselines[i] - rows[i].ascent - nth(&row_spacing, i - 1, 0.0) / 2.0
                };
                let bottom = if i + 1 == rows.len() {
                    inner_top + inner_height
                } else {
                    row_baselines[i] + rows[i].descent + nth(&row_spacing, i, 0.0) / 2.0
                };
                (top, bottom)
            })
            .collect();
        let column_bands: Vec<(f32, f32)> = (0..column_count)
            .map(|j| {
                let left = if j == 0 {
                    inner_left
                } else {
                    column_x[j] - nth(&column_spacing, j - 1, 0.0) / 2.0
                };
                let right = if j + 1 == column_count {
                    inner_left + inner_width
                } else {
                    column_x[j] + column_widths[j] + nth(&column_spacing, j, 0.0) / 2.0
                };
                (left, right)
            })
            .collect();
        // Merges the bands that aren't blocked into runs `(start, end)`.
        let runs = |bands: &[(f32, f32)], blocked: &dyn Fn(usize) -> bool| {
            let mut runs: Vec<(f32, f32)> = Vec::new();
            let mut open = false;
            for (k, &(start, end)) in bands.iter().enumerate() {
                if blocked(k) {
                    open = false;
                } else if open {
                    runs.last_mut().expect("a run is open").1 = end;
                } else {
                    runs.push((start, end));
                    open = true;
                }
            }
            runs
        };
        // Lines don't cross cells that span them.
        for j in 0..column_count.saturating_sub(1) {
            let gap = nth(&column_spacing, j, 0.0);
            let line_x = column_x[j] + column_widths[j] + gap / 2.0 - thickness / 2.0;
            let line_style = nth(&column_lines, j, LineStyle::None);
            let blocked = |i: usize| {
                cells.iter().any(|c| {
                    c.column <= j
                        && j + 1 < c.column + c.column_span
                        && (c.row..c.row + c.row_span).contains(&i)
                })
            };
            for (start, end) in runs(&row_bands, &blocked) {
                push_line(line_x, start, thickness, end - start, line_style);
            }
        }
        for i in 0..rows.len().saturating_sub(1) {
            let gap = nth(&row_spacing, i, 0.0);
            let row_bottom = row_baselines[i] + rows[i].descent;
            let line_y = row_bottom + gap / 2.0 - thickness / 2.0;
            let line_style = nth(&row_lines, i, LineStyle::None);
            let blocked = |j: usize| {
                cells.iter().any(|c| {
                    c.row <= i
                        && i + 1 < c.row + c.row_span
                        && (c.column..c.column + c.column_span).contains(&j)
                })
            };
            for (start, end) in runs(&column_bands, &blocked) {
                push_line(start, line_y, end - start, thickness, line_style);
            }
        }
        push_line(0.0, 0.0, width, thickness, frame);
        push_line(0.0, height - thickness, width, thickness, frame);
        push_line(0.0, 0.0, thickness, height, frame);
        push_line(width - thickness, 0.0, thickness, height, frame);

        // Rows span the table; cells fill their grid area. A cell belongs to the row it starts
        // in.
        let mut row_cells: Vec<Vec<Cell<Box_<T>>>> = (0..rows.len()).map(|_| Vec::new()).collect();
        for cell in cells {
            row_cells[cell.row].push(cell);
        }
        for (i, (row, cells)) in rows.iter().zip(row_cells).enumerate() {
            let baseline = row_baselines[i];
            let cells = cells
                .into_iter()
                .map(|cell| {
                    let last_column = cell.column + cell.column_span - 1;
                    let last_row = i + cell.row_span - 1;
                    let area_width =
                        column_x[last_column] + column_widths[last_column] - column_x[cell.column];
                    let area_descent = row_baselines[last_row] + rows[last_row].descent - baseline;
                    let m = cell.lbox.metrics;
                    let dx = match cell.column_align {
                        ColumnAlign::Left => 0.0,
                        ColumnAlign::Center => (area_width - m.advance) / 2.0,
                        ColumnAlign::Right => area_width - m.advance,
                    };
                    let dy = match cell.row_align {
                        RowAlign::Baseline => 0.0,
                        RowAlign::Top => -row.ascent + m.ascent,
                        RowAlign::Bottom => area_descent - m.descent,
                        RowAlign::Center => {
                            (area_descent - row.ascent) / 2.0 - (m.descent - m.ascent) / 2.0
                        }
                    };
                    let mut cell_box =
                        fill_cell(cell.lbox, area_width, row.ascent, area_descent, dx, dy);
                    cell_box.origin = Point::new(column_x[cell.column], 0.0);
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
