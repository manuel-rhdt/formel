//! The traits through which mlayout reads the host's document tree.

use std::borrow::Cow;
use std::fmt::Debug;
use std::hash::Hash;

use super::font::MathFont;
use super::output::{ForeignMetrics, NodeLayout};
use super::style::{Form, MathComputedStyle, OperatorOverrides, parse_math_length};

/// The MathML elements mlayout knows how to lay out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MathElement {
    Math,
    Mrow,
    Mfrac,
    Msqrt,
    Mroot,
    Msub,
    Msup,
    Msubsup,
    Munder,
    Mover,
    Munderover,
    Mmultiscripts,
    Mprescripts,
    /// The `<none>` element (an empty script of `<mmultiscripts>`).
    NoneElement,
    Mi,
    Mn,
    Mo,
    Mtext,
    Ms,
    Mspace,
    Mpadded,
    /// `<menclose>`: draws the lines named by its `notation` attribute around its content.
    Menclose,
    Mphantom,
    /// `<maligngroup>`: starts an alignment group in a table cell (a direct child of `<mtd>`).
    Maligngroup,
    /// `<malignmark>`: the alignment point of the group it is in.
    Malignmark,
    Mstyle,
    Merror,
    Semantics,
    Mtable,
    Mtr,
    /// A table row whose first child is a label, placed beside the table (`side`).
    Mlabeledtr,
    Mtd,
    /// Any other element in the MathML namespace. Laid out like `<mrow>`.
    Unknown,
}

impl MathElement {
    /// Maps a MathML local name (e.g. `"mfrac"`) to an element.
    pub fn from_local_name(name: &str) -> MathElement {
        use MathElement::*;
        match name {
            "math" => Math,
            "mrow" => Mrow,
            "mfrac" => Mfrac,
            "msqrt" => Msqrt,
            "mroot" => Mroot,
            "msub" => Msub,
            "msup" => Msup,
            "msubsup" => Msubsup,
            "munder" => Munder,
            "mover" => Mover,
            "munderover" => Munderover,
            "mmultiscripts" => Mmultiscripts,
            "mprescripts" => Mprescripts,
            "none" => NoneElement,
            "mi" => Mi,
            "mn" => Mn,
            "mo" => Mo,
            "mtext" => Mtext,
            "ms" => Ms,
            "mspace" => Mspace,
            "mpadded" => Mpadded,
            "menclose" => Menclose,
            "mphantom" => Mphantom,
            "maligngroup" => Maligngroup,
            "malignmark" => Malignmark,
            "mstyle" => Mstyle,
            "merror" => Merror,
            "semantics" => Semantics,
            "mtable" => Mtable,
            "mtr" => Mtr,
            "mlabeledtr" => Mlabeledtr,
            "mtd" => Mtd,
            _ => Unknown,
        }
    }

    /// Token elements contain text instead of child elements.
    pub fn is_token(self) -> bool {
        use MathElement::*;
        matches!(self, Mi | Mn | Mo | Mtext | Ms)
    }
}

/// What kind of box a node is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeKind {
    Math(MathElement),
    /// Content the host lays out itself: non-MathML content (e.g. an HTML element inside
    /// `<mtext>` or `<semantics>`), or MathML that the host wants to handle with its own
    /// algorithms. For example, a host with CSS table layout may report `<mtable>` as foreign,
    /// lay it out as a table, call [`layout_math`](super::layout_math) on each `<mtd>`, and
    /// center the table on the math axis (see [`axis_height`](super::axis_height)).
    /// mlayout then places the result through [`MathLayoutHost::layout_foreign`].
    Foreign,
}

/// Read access to the host's tree: structure, computed style and fonts.
///
/// Rendered children only: [`children`](MathTree::children) must skip `display: none`
/// elements, comments and whitespace-only text. Token elements (`mi`, `mn`, `mo`, `mtext`,
/// `ms`) are leaves; their content comes from [`text`](MathTree::text).
pub trait MathTree {
    /// A cheap handle that identifies a node (an index, an entity, a pointer).
    type NodeId: Copy + Eq + Hash + Debug;
    /// A cheap handle to a font (e.g. `&F`, `Rc<F>`).
    type Font: MathFont + Clone;

    fn kind(&self, node: Self::NodeId) -> NodeKind;

    /// The rendered child elements of `node`, in order.
    fn children(&self, node: Self::NodeId) -> impl Iterator<Item = Self::NodeId>;

    fn child_count(&self, node: Self::NodeId) -> usize {
        self.children(node).count()
    }

    fn style(&self, node: Self::NodeId) -> MathComputedStyle;

    /// The first available math font of `node`'s computed `font-family`.
    fn font(&self, node: Self::NodeId) -> Self::Font;

    /// The text content of a token element, after CSS `text-transform` (including
    /// `math-auto` italic mapping for single-character `<mi>`) has been applied.
    fn text(&self, node: Self::NodeId) -> Option<Cow<'_, str>>;

    /// A MathML attribute of `node`, e.g. `linethickness` or `accent`.
    fn attribute(&self, node: Self::NodeId, name: &str) -> Option<Cow<'_, str>>;

    /// Operator properties that override the operator dictionary for an `<mo>`.
    ///
    /// By default this reads the MathML attributes `form`, `stretchy`, `symmetric`,
    /// `largeop`, `movablelimits`, `lspace`, `rspace`, `minsize` and `maxsize`.
    /// Override it to map these from CSS instead.
    fn operator_overrides(&self, node: Self::NodeId) -> OperatorOverrides {
        default_operator_overrides(self, node)
    }
}

/// The default implementation of [`MathTree::operator_overrides`], for hosts that override it
/// but still want the attribute-based values for some properties.
pub fn default_operator_overrides<T: MathTree + ?Sized>(
    tree: &T,
    node: T::NodeId,
) -> OperatorOverrides {
    let flag = |name| match tree.attribute(node, name).as_deref().map(str::trim) {
        Some("true") => Some(true),
        Some("false") => Some(false),
        _ => None,
    };
    let style = tree.style(node);
    let length = |name| {
        tree.attribute(node, name)
            .and_then(|value| parse_math_length(&value, &style))
    };
    let form = match tree.attribute(node, "form").as_deref().map(str::trim) {
        Some("prefix") => Some(Form::Prefix),
        Some("infix") => Some(Form::Infix),
        Some("postfix") => Some(Form::Postfix),
        _ => None,
    };
    OperatorOverrides {
        form,
        stretchy: flag("stretchy"),
        symmetric: flag("symmetric"),
        largeop: flag("largeop"),
        movablelimits: flag("movablelimits"),
        lspace: length("lspace"),
        rspace: length("rspace"),
        minsize: length("minsize"),
        maxsize: length("maxsize"),
    }
}

/// A host that can lay out foreign content and receive the results.
pub trait MathLayoutHost: MathTree {
    /// Lays out a [`NodeKind::Foreign`] node and returns its margin-box metrics. This may be
    /// called more than once for the same node.
    fn layout_foreign(&mut self, node: Self::NodeId) -> ForeignMetrics {
        let _ = node;
        ForeignMetrics::default()
    }

    /// Receives the final layout of a node. Called once per laid-out node, children before
    /// their parent.
    fn set_layout(&mut self, node: Self::NodeId, layout: NodeLayout);
}

impl<T: MathTree + ?Sized> MathTree for &T {
    type NodeId = T::NodeId;
    type Font = T::Font;

    fn kind(&self, node: Self::NodeId) -> NodeKind {
        (**self).kind(node)
    }
    fn children(&self, node: Self::NodeId) -> impl Iterator<Item = Self::NodeId> {
        (**self).children(node)
    }
    fn child_count(&self, node: Self::NodeId) -> usize {
        (**self).child_count(node)
    }
    fn style(&self, node: Self::NodeId) -> MathComputedStyle {
        (**self).style(node)
    }
    fn font(&self, node: Self::NodeId) -> Self::Font {
        (**self).font(node)
    }
    fn text(&self, node: Self::NodeId) -> Option<Cow<'_, str>> {
        (**self).text(node)
    }
    fn attribute(&self, node: Self::NodeId, name: &str) -> Option<Cow<'_, str>> {
        (**self).attribute(node, name)
    }
    fn operator_overrides(&self, node: Self::NodeId) -> OperatorOverrides {
        (**self).operator_overrides(node)
    }
}
