//! Operator properties, resolved at layout time from the operator dictionary and the host's
//! overrides.

use super::Engine;
use crate::host::{Form, MathElement, MathLayoutHost, NodeKind};
use crate::mathmlparser::Flags;
use crate::mathmlparser::operator_dict;

/// The resolved properties of an `<mo>`.
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub(crate) struct OperatorProperties {
    pub stretchy: bool,
    pub symmetric: bool,
    pub largeop: bool,
    pub movablelimits: bool,
    /// px
    pub lspace: f32,
    /// px
    pub rspace: f32,
    pub minsize: Option<f32>,
    pub maxsize: Option<f32>,
}

impl<'t, T: MathLayoutHost> Engine<'t, T> {
    pub(crate) fn element(&self, node: T::NodeId) -> Option<MathElement> {
        match self.tree.kind(node) {
            NodeKind::Math(element) => Some(element),
            NodeKind::Foreign => None,
        }
    }

    /// The properties of the `<mo>` `op`, which is in the given default `form`.
    pub(crate) fn operator_properties(&self, op: T::NodeId, form: Form) -> OperatorProperties {
        let overrides = self.tree.operator_overrides(op);
        let form = overrides.form.unwrap_or(form);
        let character = self.tree.text(op).and_then(|text| {
            let mut chars = text.chars();
            match (chars.next(), chars.next()) {
                (Some(c), None) => Some(c),
                _ => None,
            }
        });
        let entry = character
            .and_then(|c| operator_dict::find_entry(c, form))
            .unwrap_or_default();
        let em = self.tree.style(op).font_size;
        let flags = entry.flags;
        OperatorProperties {
            stretchy: overrides.stretchy.unwrap_or(flags.contains(Flags::STRETCHY)),
            symmetric: overrides.symmetric.unwrap_or(flags.contains(Flags::SYMMETRIC)),
            largeop: overrides.largeop.unwrap_or(flags.contains(Flags::LARGEOP)),
            movablelimits: overrides
                .movablelimits
                .unwrap_or(flags.contains(Flags::MOVABLE_LIMITS)),
            lspace: overrides
                .lspace
                .unwrap_or(entry.lspace as f32 / 18.0 * em),
            rspace: overrides
                .rspace
                .unwrap_or(entry.rspace as f32 / 18.0 * em),
            minsize: overrides.minsize,
            maxsize: overrides.maxsize,
        }
    }

    /// The properties of the core operator of `node`, if `node` is an embellished operator.
    pub(crate) fn embellished_properties(
        &self,
        node: T::NodeId,
        form: Form,
    ) -> Option<OperatorProperties> {
        self.core_operator(node)
            .map(|op| self.operator_properties(op, form))
    }

    /// The `<mo>` at the core of `node` if it is an embellished operator (MathML Core 3.2.4.1).
    pub(crate) fn core_operator(&self, node: T::NodeId) -> Option<T::NodeId> {
        use MathElement::*;
        match self.element(node)? {
            Mo => Some(node),
            Msub | Msup | Msubsup | Munder | Mover | Munderover | Mmultiscripts | Mfrac
            | Semantics => self
                .tree
                .children(node)
                .next()
                .and_then(|first| self.core_operator(first)),
            Mrow | Mstyle | Mphantom | Mpadded => self
                .embellished_row_child(node)
                .and_then(|child| self.core_operator(child)),
            _ => None,
        }
    }

    /// For an mrow-like element whose only non-space-like child is an embellished operator,
    /// returns that child.
    pub(crate) fn embellished_row_child(&self, node: T::NodeId) -> Option<T::NodeId> {
        let mut candidates = self
            .tree
            .children(node)
            .filter(|&child| !self.is_space_like(child));
        let only = candidates.next()?;
        if candidates.next().is_some() {
            return None;
        }
        self.core_operator(only).map(|_| only)
    }

    /// Space-like elements don't count when deciding an operator's form (MathML Core
    /// 3.2.5.1).
    pub(crate) fn is_space_like(&self, node: T::NodeId) -> bool {
        use MathElement::*;
        match self.element(node) {
            Some(Mtext | Mspace) => true,
            Some(Mrow | Mstyle | Mphantom | Mpadded) => {
                self.tree.children(node).all(|child| self.is_space_like(child))
            }
            _ => false,
        }
    }
}
