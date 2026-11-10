//! Standalone use without a host engine: a minimal MathML document with a built-in user agent
//! stylesheet.
//!
//! [`MathDocument`] is a plain element tree (the XML parser in
//! [`mathmlparser`](crate::mathmlparser) produces one). [`MathDocument::style`] computes the
//! CSS that a browser would compute from the MathML Core user agent stylesheet. The result,
//! [`StyledDocument`], implements [`MathTree`], so it is laid out by the same engine as any
//! host tree.

mod ua_style;

use std::borrow::Cow;

use crate::host::{
    LayoutCollector, MathComputedStyle, MathElement, MathFont, MathStyle, MathTree, NodeKind,
    layout_math,
};

/// Index of a node in a [`MathDocument`].
pub type NodeId = usize;

/// An element of a [`MathDocument`].
#[derive(Debug, Clone, PartialEq)]
pub struct DocNode {
    /// The local name, e.g. `"mfrac"`.
    pub name: String,
    pub element: MathElement,
    pub attributes: Vec<(String, String)>,
    /// The text content of a token element.
    pub text: Option<String>,
    pub children: Vec<NodeId>,
}

/// A MathML element tree.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MathDocument {
    nodes: Vec<DocNode>,
    root: Option<NodeId>,
}

impl MathDocument {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds an element with the given children. The last added node becomes the root unless
    /// [`set_root`](Self::set_root) is called.
    pub fn add_element(
        &mut self,
        name: &str,
        attributes: Vec<(String, String)>,
        children: Vec<NodeId>,
    ) -> NodeId {
        self.push(DocNode {
            name: name.to_owned(),
            element: MathElement::from_local_name(name),
            attributes,
            text: None,
            children,
        })
    }

    /// Adds a token element (`mi`, `mo`, ...) with text content.
    pub fn add_token(
        &mut self,
        name: &str,
        attributes: Vec<(String, String)>,
        text: &str,
    ) -> NodeId {
        self.push(DocNode {
            name: name.to_owned(),
            element: MathElement::from_local_name(name),
            attributes,
            text: Some(text.to_owned()),
            children: Vec::new(),
        })
    }

    fn push(&mut self, node: DocNode) -> NodeId {
        self.nodes.push(node);
        let id = self.nodes.len() - 1;
        self.root = Some(id);
        id
    }

    pub fn set_root(&mut self, root: NodeId) {
        self.root = Some(root);
    }

    pub fn root(&self) -> Option<NodeId> {
        self.root
    }

    pub fn node(&self, id: NodeId) -> &DocNode {
        &self.nodes[id]
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn attribute(&self, id: NodeId, name: &str) -> Option<&str> {
        self.nodes[id]
            .attributes
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    }

    /// The parent of every node (`None` for nodes outside the tree and the root).
    pub fn parents(&self) -> Vec<Option<NodeId>> {
        let mut parents = vec![None; self.nodes.len()];
        for (id, node) in self.nodes.iter().enumerate() {
            for &child in &node.children {
                parents[child] = Some(id);
            }
        }
        parents
    }

    /// Computes the style of every node with the built-in user agent stylesheet, using `font`
    /// for all text.
    pub fn style<F: MathFont + Clone>(
        &self,
        font: F,
        options: StyleOptions,
    ) -> StyledDocument<'_, F> {
        let (styles, texts) = ua_style::resolve(self, &font, options);
        StyledDocument {
            document: self,
            font,
            styles,
            texts,
        }
    }
}

/// Settings for the root of a [`MathDocument`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StyleOptions {
    /// `font-size` of the root in px. Using the font's units per em gives results in font
    /// units.
    pub font_size: f32,
    /// `math-style` of the root, unless a `display` attribute on `<math>` says otherwise.
    pub math_style: MathStyle,
}

impl Default for StyleOptions {
    fn default() -> Self {
        StyleOptions {
            font_size: 16.0,
            math_style: MathStyle::Normal,
        }
    }
}

/// A [`MathDocument`] with computed styles: a [`MathTree`] ready for layout.
pub struct StyledDocument<'d, F> {
    document: &'d MathDocument,
    font: F,
    styles: Vec<MathComputedStyle>,
    texts: Vec<Option<String>>,
}

impl<'d, F> StyledDocument<'d, F> {
    pub fn document(&self) -> &'d MathDocument {
        self.document
    }
}

impl<'d, F: MathFont + Clone> MathTree for StyledDocument<'d, F> {
    type NodeId = NodeId;
    type Font = F;

    fn kind(&self, node: NodeId) -> NodeKind {
        NodeKind::Math(self.document.nodes[node].element)
    }

    fn children(&self, node: NodeId) -> impl Iterator<Item = NodeId> {
        self.document.nodes[node].children.iter().copied()
    }

    fn child_count(&self, node: NodeId) -> usize {
        self.document.nodes[node].children.len()
    }

    fn style(&self, node: NodeId) -> MathComputedStyle {
        self.styles[node]
    }

    fn font(&self, _node: NodeId) -> F {
        self.font.clone()
    }

    fn text(&self, node: NodeId) -> Option<Cow<'_, str>> {
        self.texts[node].as_deref().map(Cow::Borrowed)
    }

    fn attribute(&self, node: NodeId, name: &str) -> Option<Cow<'_, str>> {
        if let Some(value) = self.document.attribute(node, name) {
            return Some(Cow::Borrowed(value));
        }
        // `style="text-align: ..."` on a cell stands in for a missing `columnalign`.
        if name == "columnalign" && self.document.node(node).element == MathElement::Mtd {
            let inline = self.document.attribute(node, "style")?;
            let align = ua_style::inline_style_property(inline, "text-align")?;
            return match align {
                "left" | "start" => Some(Cow::Borrowed("left")),
                "right" | "end" => Some(Cow::Borrowed("right")),
                "center" => Some(Cow::Borrowed("center")),
                _ => None,
            };
        }
        None
    }
}

/// Styles and lays out `document` (from its root) with a single font.
///
/// # Panics
/// If the document is empty.
pub fn layout<F: MathFont + Clone>(
    document: &MathDocument,
    font: F,
    options: StyleOptions,
) -> LayoutCollector<StyledDocument<'_, F>> {
    let root = document.root().expect("the document has a root");
    let mut collector = LayoutCollector::new(document.style(font, options));
    layout_math(&mut collector, root);
    collector
}
