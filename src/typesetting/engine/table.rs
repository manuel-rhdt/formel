//! Tables: `<mtable>`, `<mtr>`, `<mlabeledtr>` and `<mtd>`, for matrices, cases and aligned
//! equations.
//!
//! Alignment groups: `<maligngroup>` and `<malignmark>` that are direct children of `<mtd>`
//! line up the cells of a column (see [`Engine::align_groups`]).
//!
//! The layout follows the MathML 3 table attributes: `columnalign`, `rowalign`,
//! `columnspacing`, `rowspacing`, `columnlines`, `rowlines`, `frame`, `framespacing` and
//! `align`, `side` and `minlabelspacing` (for the labels of `<mlabeledtr>`). List-valued attributes repeat their last value. Cells may span rows and columns with
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

struct Cell<Id, B> {
    node: Id,
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
        // Labels sit outside the table on this side; the `overlap` variants are treated alike.
        let label_on_left = self
            .tree
            .attribute(node, "side")
            .is_some_and(|side| side.trim().starts_with("left"));
        let label_spacing = self
            .tree
            .attribute(node, "minlabelspacing")
            .and_then(|value| parse_math_length(&value, style))
            .unwrap_or(0.8 * style.font_size);

        let font = self.font(node, style);
        let thickness = font.constant(MathConstant::FractionRuleThickness);
        let axis_height = font.constant(MathConstant::AxisHeight);

        // Lay out the cells. Children that aren't rows form a row of their own; children of a
        // row that aren't cells are cells themselves.
        let mut rows: Vec<Row<T::NodeId>> = Vec::new();
        // the label of every `<mlabeledtr>`, indexed by row
        let mut labels: Vec<Option<Box_<T>>> = Vec::new();
        let mut cells: Vec<Cell<T::NodeId, Box_<T>>> = Vec::new();
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
            let label = match self.element(child) {
                Some(MathElement::Mlabeledtr) => self
                    .children(child)
                    .first()
                    .map(|&label| self.layout_node(label, Ctx::plain(Form::Infix))),
                _ => None,
            };
            labels.push(label);
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
                    node: cell_node,
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

        self.align_groups(&mut cells);

        // row heights: baseline-aligned cells share the baseline; the others only need room
        for (i, row) in rows.iter_mut().enumerate() {
            let in_row = || cells.iter().filter(|c| c.row == i && c.row_span == 1);
            if let Some(label) = &labels[i] {
                row.ascent = row.ascent.max(label.metrics.ascent);
                row.descent = row.descent.max(label.metrics.descent);
            }
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
        let mut spanning: Vec<&Cell<T::NodeId, Box_<T>>> = cells.iter().filter(|c| c.row_span > 1).collect();
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
        let mut spanning: Vec<&Cell<T::NodeId, Box_<T>>> = cells.iter().filter(|c| c.column_span > 1).collect();
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

        // Labels form a column of their own next to the table, aligned toward its outer edge.
        let label_width = labels
            .iter()
            .flatten()
            .map(|label| label.metrics.advance)
            .fold(0.0, f32::max);
        let has_labels = labels.iter().any(Option::is_some);
        let label_column = if has_labels { label_width + label_spacing } else { 0.0 };
        // how far the table is moved right to make room for labels on the left
        let shift = if label_on_left { label_column } else { 0.0 };

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
                line.origin = Point::new(shift + x, top + y + h);
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
        let mut row_cells: Vec<Vec<Cell<T::NodeId, Box_<T>>>> = (0..rows.len()).map(|_| Vec::new()).collect();
        for cell in cells {
            row_cells[cell.row].push(cell);
        }
        let mut labels = labels.into_iter();
        for (i, (row, cells)) in rows.iter().zip(row_cells).enumerate() {
            let label = labels.next().flatten();
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
            let mut cells: Vec<Box_<T>> = cells;
            if let Some(mut label) = label {
                // relative to the row, which is shifted right by `shift`
                label.origin.x = if label_on_left {
                    -shift
                } else {
                    width + label_spacing + label_width - label.metrics.advance
                };
                cells.push(label);
            }
            let mut row_box = LBox::boxes(cells);
            row_box.metrics = box_metrics(width, row.ascent, row.descent);
            row_box.origin = Point::new(shift, top + baseline);
            row_box.node = row.node;
            children.push(row_box);
        }

        let mut table = LBox::boxes(children);
        table.metrics = box_metrics(shift + width + (label_column - shift), -top, top + height);
        table
    }
}

/// An alignment group of a cell: the boxes of children `first..end` start at `start`, and the
/// alignment point is `left` after the start and `right` before the end.
struct Group {
    first: usize,
    end: usize,
    start: f32,
    left: f32,
    right: f32,
}

fn has_nested_row<Id>(cell: &LBox<Id>) -> bool {
    matches!(&cell.content, Content::Boxes(v) if v.len() == 1 && v[0].node.is_none())
}

/// The box holding a cell's children: the cell's own box, or the box inside it if the cell has
/// padding, a border or a margin.
fn cell_row_mut<Id>(cell: &mut LBox<Id>) -> &mut LBox<Id> {
    if !has_nested_row(cell) {
        return cell;
    }
    match &mut cell.content {
        Content::Boxes(v) => &mut v[0],
        _ => unreachable!("checked by has_nested_row"),
    }
}

impl<'t, T: MathLayoutHost> Engine<'t, T> {
    /// Alignment groups (MathML 3, 3.5.5). A `<maligngroup>` starts a group that ends at the
    /// next one; content before the first one belongs to the first group. The alignment point of
    /// a group is its `<malignmark>` if it has one, else its left edge, center or right edge
    /// according to the `groupalign` of the `<maligngroup>` (`left`, `center`, `right` or
    /// `decimalpoint`, which without a mark is the right edge). Down a column, the groups with
    /// the same index are placed so that their alignment points line up, and all cells of
    /// the column that have groups become equally wide.
    ///
    /// Markers nested deeper than the cell's children, and cells that span columns, are not
    /// aligned.
    fn align_groups(&self, cells: &mut [Cell<T::NodeId, Box_<T>>]) {
        let all_groups: Vec<Option<Vec<Group>>> =
            cells.iter_mut().map(|cell| self.cell_groups(cell)).collect();

        // for every column and group index: the largest extent left and right of the point
        let mut columns: Vec<Vec<(f32, f32)>> = Vec::new();
        for (cell, groups) in cells.iter().zip(&all_groups) {
            let Some(groups) = groups else { continue };
            if columns.len() <= cell.column {
                columns.resize(cell.column + 1, Vec::new());
            }
            let column = &mut columns[cell.column];
            if column.len() < groups.len() {
                column.resize(groups.len(), (0.0, 0.0));
            }
            for (extent, group) in column.iter_mut().zip(groups) {
                extent.0 = extent.0.max(group.left);
                extent.1 = extent.1.max(group.right);
            }
        }

        for (cell, groups) in cells.iter_mut().zip(all_groups) {
            let Some(groups) = groups else { continue };
            let column = &columns[cell.column];
            let total: f32 = column.iter().map(|(left, right)| left + right).sum();
            let nested = has_nested_row(&cell.lbox);
            let row = cell_row_mut(&mut cell.lbox);
            let delta = total - row.metrics.advance;
            let Content::Boxes(boxes) = &mut row.content else { continue };
            let mut slot_start = 0.0;
            for (group, &(left, right)) in groups.iter().zip(column) {
                let dx = slot_start + (left - group.left) - group.start;
                for child in &mut boxes[group.first..group.end] {
                    child.origin.x += dx;
                }
                slot_start += left + right;
            }
            row.metrics.advance = total;
            row.metrics.ink_left = 0.0;
            row.metrics.ink_width = total;
            if nested {
                cell.lbox.metrics.advance += delta;
                cell.lbox.metrics.ink_width += delta;
            }
        }
    }

    fn cell_groups(&self, cell: &mut Cell<T::NodeId, Box_<T>>) -> Option<Vec<Group>> {
        use MathElement::{Maligngroup, Malignmark};
        let children = self.children(cell.node);
        let markers: Vec<usize> = (0..children.len())
            .filter(|&i| self.element(children[i]) == Some(Maligngroup))
            .collect();
        if markers.is_empty() || cell.column_span != 1 {
            return None;
        }
        let row = cell_row_mut(&mut cell.lbox);
        let advance = row.metrics.advance;
        let Content::Boxes(boxes) = &row.content else { return None };
        if boxes.len() != children.len() {
            return None;
        }
        let mut groups = Vec::with_capacity(markers.len());
        for (g, &marker) in markers.iter().enumerate() {
            let end = markers.get(g + 1).copied().unwrap_or(boxes.len());
            let start = if g == 0 { 0.0 } else { boxes[marker].origin.x };
            let end_x = markers.get(g + 1).map_or(advance, |&next| boxes[next].origin.x);
            let width = end_x - start;
            let mark = (marker..end).find(|&i| self.element(children[i]) == Some(Malignmark));
            let point = match mark {
                Some(mark) => boxes[mark].origin.x - start,
                None => match self
                    .tree
                    .attribute(children[marker], "groupalign")
                    .and_then(|value| value.split_whitespace().next().map(str::to_owned))
                    .as_deref()
                {
                    Some("right" | "decimalpoint") => width,
                    Some("center") => width / 2.0,
                    _ => 0.0,
                },
            };
            groups.push(Group {
                first: if g == 0 { 0 } else { marker },
                end,
                start,
                left: point,
                right: width - point,
            });
        }
        Some(groups)
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
