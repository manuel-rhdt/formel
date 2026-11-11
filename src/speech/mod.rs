//! Spoken text for formulas, built from the MathML structure, for screen readers
//! (`aria-label`) and other accessible output.
//!
//! ```text
//! <msqrt><msup><mi>b</mi><mn>2</mn></msup><mo>-</mo><mn>4</mn><mi>a</mi><mi>c</mi></msqrt>
//!   → "the square root of b squared minus 4 a c"
//! ```
//!
//! Every word comes from a [`Lexicon`]: the symbol names, function names, and phrase templates
//! are tables, not code. [`ENGLISH`] is the built-in one. The engine decides which entry
//! applies: it recognises fractions, radicals, scripts, large operators with limits, accents,
//! fences (absolute value, matrices), tables and tokens. Adjacent operands are joined by a
//! space, so implicit multiplication reads `2 a`.
//!
//! The text of any element is never empty: elements that are not spoken (`<mspace>`,
//! `<mphantom>`, invisible operators) are skipped inside a row, and spoken on their own as
//! [`Phrases::blank`], [`Phrases::space`], or their [`Lexicon::silent_names`] entry.

mod english;

use std::borrow::Cow;

use crate::host::{MathElement, MathTree, NodeKind};
use crate::standalone::MathDocument;
use crate::typesetting::unicode_math::split_family;
use crate::unicode_math::{Family, StringExtMathml};

pub use self::english::ENGLISH;

/// A table from text to the words spoken for it.
pub type Table = &'static [(&'static str, &'static str)];

/// Everything that is spoken, for one language.
///
/// Templates contain numbered placeholders, `{0}`, `{1}`, ..., which are replaced by the
/// spoken arguments named in each field's documentation.
#[derive(Debug, Clone, Copy)]
pub struct Lexicon {
    /// Operators and other symbols. Looked up for the whole content of an `<mi>` or `<mo>`
    /// first, then character by character. An empty word means the symbol is silent.
    pub symbols: Table,
    /// Symbols spoken differently in the underscript of a large operator (`x → 0` under `lim`
    /// reads "x approaches 0"). Takes precedence over `symbols` there.
    pub limit_symbols: Table,
    /// Symbols spoken differently on the left of a set-builder (`{x ∈ ℝ : …}` reads "x in
    /// ℝ").
    pub set_symbols: Table,
    /// Symbols spoken differently in the arguments of a function (`P(A ∣ B)` reads "given").
    pub argument_symbols: Table,
    /// Ordinal words for the order of a derivative (`2` → "second").
    pub ordinals: Table,
    /// Numerators spoken as words in a common fraction (`3` → "three").
    pub cardinals: Table,
    /// Denominators of common fractions: `(denominator, singular, plural)`, e.g.
    /// `("4", "quarter", "quarters")`. A fraction of two numbers from `cardinals` and this
    /// table reads "three quarters"; any other reads "11 over 2".
    pub fraction_denominators: &'static [(&'static str, &'static str, &'static str)],
    /// What to say for silent symbols when one is spoken on its own.
    pub silent_names: Table,
    /// Function names and letters (`sin` → "sine", `α` → "alpha"). Capitals of a letter listed
    /// here are spoken through [`Phrases::capital`].
    pub identifiers: Table,
    /// The adjective for a `mathvariant` (`"bold"`, `"double-struck"`). Italic and normal are
    /// not spoken.
    pub variants: Table,
    /// Superscripts with a name of their own (`2` → "squared", `′` → "prime").
    pub powers: Table,
    /// Root indices with a name of their own (`3` → "cube").
    pub root_indices: Table,
    /// Over- and underscripts spoken as an accent after their base (`^` → "hat").
    pub accents: Table,
    /// Large operators that take limits: `(operator, name, connective for the underscript)`,
    /// e.g. `("lim", "the limit", "as")`.
    pub large_operators: &'static [(&'static str, &'static str, &'static str)],
    /// Fence pairs with a meaning: `(open, close, template)`, `{0}` = the content.
    pub fences: &'static [(&'static str, &'static str, &'static str)],
    /// Fence pairs that turn a table they enclose into a kind of its own:
    /// `(open, close, kind)`, e.g. `("(", ")", "matrix")`.
    pub table_fences: &'static [(&'static str, &'static str, &'static str)],
    /// `<menclose>` notations: `(notation, template)`, `{0}` = the content.
    pub enclosures: Table,
    pub phrases: Phrases,
}

/// The phrase templates of a [`Lexicon`].
#[derive(Debug, Clone, Copy)]
pub struct Phrases {
    /// An element with nothing to say, spoken on its own (an empty row, `<mphantom>`).
    pub blank: &'static str,
    /// `<mspace>` spoken on its own.
    pub space: &'static str,
    /// `{0}` = the name of the lowercase letter, or the capital letter itself.
    pub capital: &'static str,
    /// `{0}` = the variant adjective, `{1}` = the letter.
    pub variant: &'static str,
    /// A minus sign used as a prefix.
    pub negative: &'static str,
    /// `<ms>`: `{0}` = its text.
    pub string: &'static str,
    /// `<merror>`: `{0}` = its content.
    pub error: &'static str,
    /// `<menclose>` with a notation missing from [`Lexicon::enclosures`]: `{0}` = its content.
    pub enclosed: &'static str,

    /// A common fraction: `{0}` = numerator word, `{1}` = denominator word (singular after a
    /// numerator of 1).
    pub common_fraction: &'static str,
    /// Both parts are single tokens, or a token with a power from [`Lexicon::powers`]
    /// ("1 over n squared"): `{0}` = numerator, `{1}` = denominator.
    pub fraction_simple: &'static str,
    pub fraction: &'static str,
    /// `{0}` = radicand.
    pub square_root: &'static str,
    /// `{0}` = name from [`Lexicon::root_indices`], `{1}` = radicand.
    pub named_root: &'static str,
    /// The index is a single identifier: `{0}` = index, `{1}` = radicand.
    pub simple_root: &'static str,
    pub root: &'static str,

    /// `{0}` = base, `{1}` = name from [`Lexicon::powers`].
    pub named_power: &'static str,
    /// The exponent is a single token: `{0}` = base, `{1}` = exponent.
    pub power_simple: &'static str,
    pub power: &'static str,
    /// The subscript is a single token: `{0}` = base, `{1}` = subscript.
    pub subscript_simple: &'static str,
    pub subscript: &'static str,
    /// `{0}` = base, `{1}` = subscript, `{2}` = name from [`Lexicon::powers`].
    pub sub_named_power: &'static str,
    /// `{0}` = base, `{1}` = subscript, `{2}` = superscript.
    pub sub_power: &'static str,

    /// `{0}` = operator name, `{1}` = connective, `{2}` = underscript.
    pub large_operator_under: &'static str,
    /// `{0}` = operator name, `{1}` = upper limit.
    pub large_operator_over: &'static str,
    /// `{0}` = operator name, `{1}` = lower limit, `{2}` = upper limit.
    pub large_operator_both: &'static str,
    /// `\binom{n}{k}`: `{0}` = top, `{1}` = bottom.
    pub binomial: &'static str,
    /// `{1, 2, 3}`: `{0}` = the members.
    pub set: &'static str,
    /// `{x : x > 0}`: `{0}` = the members, `{1}` = the condition.
    pub set_builder: &'static str,
    /// `{}`.
    pub empty_set: &'static str,
    /// `{` before a table: `{0}` = number of cases, `{1}` = the cases joined by `row_separator`.
    pub cases: &'static str,
    /// `{0}` = case number, `{1}` = the case.
    pub case: &'static str,
    /// A case with a condition: `{0}` = value, `{1}` = condition.
    pub case_condition: &'static str,
    /// A case whose condition starts with text (`if x > 0`, `otherwise`).
    pub case_text: &'static str,
    /// The kind of `dy/dx`.
    pub derivative: &'static str,
    /// The kind of `∂u/∂x`.
    pub partial_derivative: &'static str,
    /// `{0}` = ordinal from [`Lexicon::ordinals`], `{1}` = kind.
    pub ordinal_derivative: &'static str,
    /// `dy/dx`: `{0}` = kind, `{1}` = function, `{2}` = variable.
    pub derivative_of: &'static str,
    /// Between the variables of a mixed partial derivative ("x and y").
    pub variable_separator: &'static str,
    /// `d/dx`: `{0}` = kind, `{1}` = variable. Followed by [`Phrases::operand`] when an
    /// operand follows.
    pub derivative_operator: &'static str,
    /// Between a large operator and the expression it applies to ("the sum from 1 to n *of*
    /// x sub i"), so the listener hears where the limits end.
    pub operand: &'static str,

    /// `{0}` = base, `{1}` = name from [`Lexicon::accents`].
    pub accent: &'static str,
    /// `{0}` = base, `{1}` = underscript.
    pub under: &'static str,
    /// `{0}` = base, `{1}` = overscript.
    pub over: &'static str,
    /// `{0}` = base, `{1}` = underscript, `{2}` = overscript.
    pub under_over: &'static str,

    /// `<mmultiscripts>`: `{0}` = base, `{1}` = the scripts joined by `list_separator`.
    pub multiscripts: &'static str,
    pub list_separator: &'static str,
    pub post_subscript: &'static str,
    pub post_superscript: &'static str,
    pub pre_subscript: &'static str,
    pub pre_superscript: &'static str,

    /// `{0}` = rows, `{1}` = columns, `{2}` = kind, `{3}` = the rows joined by
    /// `row_separator`.
    pub table: &'static str,
    /// A table without rows: `{0}` = kind.
    pub empty_table: &'static str,
    /// The kind of a table without fences from [`Lexicon::table_fences`].
    pub table_kind: &'static str,
    pub row_separator: &'static str,
    pub cell_separator: &'static str,
    /// `{0}` = row number, `{1}` = the cells joined by `cell_separator`.
    pub row: &'static str,
    /// `{0}` = row number, `{1}` = label, `{2}` = the cells.
    pub labeled_row: &'static str,
}

/// The English speech for `node` of a host tree.
pub fn speak<T: MathTree>(tree: &T, node: T::NodeId) -> String {
    speak_with(tree, node, &ENGLISH)
}

/// The speech for `node` of a host tree, in the language of `lexicon`.
pub fn speak_with<T: MathTree>(tree: &T, node: T::NodeId, lexicon: &Lexicon) -> String {
    Speaker {
        source: &HostTree(tree),
        lexicon,
    }
    .speak(node, Context::default())
}

/// The English speech for `node` of an unstyled [`MathDocument`] (no font needed).
pub fn speak_document(document: &MathDocument, node: usize) -> String {
    speak_document_with(document, node, &ENGLISH)
}

/// The speech for `node` of a [`MathDocument`], in the language of `lexicon`.
pub fn speak_document_with(document: &MathDocument, node: usize, lexicon: &Lexicon) -> String {
    Speaker {
        source: document,
        lexicon,
    }
    .speak(node, Context::default())
}

/// The parts of a tree that speech reads.
trait Source {
    type Id: Copy + Eq;
    /// `None` for foreign content.
    fn element(&self, node: Self::Id) -> Option<MathElement>;
    fn children(&self, node: Self::Id) -> Vec<Self::Id>;
    /// Token text, with `mathvariant` applied.
    fn text(&self, node: Self::Id) -> Option<Cow<'_, str>>;
    fn attribute(&self, node: Self::Id, name: &str) -> Option<Cow<'_, str>>;
}

struct HostTree<'t, T>(&'t T);

impl<T: MathTree> Source for HostTree<'_, T> {
    type Id = T::NodeId;
    fn element(&self, node: T::NodeId) -> Option<MathElement> {
        match self.0.kind(node) {
            NodeKind::Math(element) => Some(element),
            NodeKind::Foreign => None,
        }
    }
    fn children(&self, node: T::NodeId) -> Vec<T::NodeId> {
        self.0.children(node).collect()
    }
    fn text(&self, node: T::NodeId) -> Option<Cow<'_, str>> {
        self.0.text(node)
    }
    fn attribute(&self, node: T::NodeId, name: &str) -> Option<Cow<'_, str>> {
        self.0.attribute(node, name)
    }
}

impl Source for MathDocument {
    type Id = usize;
    fn element(&self, node: usize) -> Option<MathElement> {
        Some(self.node(node).element)
    }
    fn children(&self, node: usize) -> Vec<usize> {
        self.node(node).children.clone()
    }
    fn text(&self, node: usize) -> Option<Cow<'_, str>> {
        let text = self.node(node).text.as_deref()?;
        match self
            .attribute(node, "mathvariant")
            .and_then(Family::from_mathvariant)
        {
            Some(family) => Some(Cow::Owned(text.adapt_to_family(Some(family)).into_owned())),
            None => Some(Cow::Borrowed(text)),
        }
    }
    fn attribute(&self, node: usize, name: &str) -> Option<Cow<'_, str>> {
        MathDocument::attribute(self, node, name).map(Cow::Borrowed)
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct Context {
    /// Words that take precedence over [`Lexicon::symbols`] in this subtree: the underscript of
    /// a large operator, the members of a set-builder, the arguments of a function.
    symbols: Option<Table>,
}

/// What a parenthesized argument follows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Callee {
    /// An invisible function application (U+2061), which says "of" itself.
    Application,
    /// A function by convention: `f`, `g`, `h`.
    NamedFunction,
    /// A capital or non-Latin letter or a function name, which may also be a factor
    /// (`A(B + C)`). A lowercase Latin letter other than `f`, `g`, `h` is not a callee.
    Function,
}

/// Letters read as functions before any parenthesized argument.
const FUNCTION_LETTERS: &[&str] = &["f", "g", "h"];

/// Fence pairs recognised in a row. Structural, not spoken: the words are in the lexicon.
const FENCE_PAIRS: &[(&str, &str)] = &[
    ("(", ")"),
    ("[", "]"),
    ("{", "}"),
    ("⟨", "⟩"),
    ("|", "|"),
    ("‖", "‖"),
    ("⌊", "⌋"),
    ("⌈", "⌉"),
];

/// Operators after which a minus sign is binary, not a prefix.
const POSTFIX: &[&str] = &[
    ")", "]", "}", "⟩", "|", "‖", "⌋", "⌉", "!", "′", "″", "‴", "%", "°",
];

const FUNCTION_APPLICATION: &str = "\u{2061}";

fn lookup(table: Table, key: &str) -> Option<&'static str> {
    table.iter().find(|(k, _)| *k == key).map(|(_, v)| *v)
}

/// Replaces `{0}`, `{1}`, ... in `template` with `args`.
fn fill(template: &str, args: &[&str]) -> String {
    let mut out =
        String::with_capacity(template.len() + args.iter().map(|a| a.len()).sum::<usize>());
    let mut rest = template;
    while let Some(start) = rest.find('{') {
        out.push_str(&rest[..start]);
        let after = &rest[start + 1..];
        match after
            .find('}')
            .and_then(|end| Some((end, after[..end].parse::<usize>().ok()?)))
        {
            Some((end, index)) => {
                out.push_str(args.get(index).copied().unwrap_or(""));
                rest = &after[end + 1..];
            }
            None => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// Strips mathematical alphanumeric styling and collapses whitespace. Returns the plain text
/// and the `mathvariant` of the first styled character.
fn plain(text: &str) -> (String, Option<&'static str>) {
    let mut family = None;
    let mut out = String::with_capacity(text.len());
    for word in text.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        for c in word.chars() {
            let (base, f) = split_family(c);
            if family.is_none() && !matches!(f, Family::Normal) {
                family = Some(f.mathvariant());
            }
            out.push(base);
        }
    }
    (out, family)
}

fn join(parts: Vec<String>, separator: &str) -> Option<String> {
    let parts: Vec<String> = parts.into_iter().filter(|p| !p.is_empty()).collect();
    (!parts.is_empty()).then(|| parts.join(separator))
}

struct Speaker<'a, S: Source> {
    source: &'a S,
    lexicon: &'a Lexicon,
}

impl<S: Source> Speaker<'_, S> {
    fn phrases(&self) -> &Phrases {
        &self.lexicon.phrases
    }

    /// The speech for `node`, never empty.
    fn speak(&self, node: S::Id, ctx: Context) -> String {
        self.spoken(node, ctx)
            .unwrap_or_else(|| self.silent_name(node))
    }

    fn speak_opt(&self, node: Option<S::Id>, ctx: Context) -> String {
        match node {
            Some(node) => self.speak(node, ctx),
            None => self.phrases().blank.to_owned(),
        }
    }

    fn silent_name(&self, node: S::Id) -> String {
        use MathElement::*;
        let element = self.source.element(node);
        if let Some(Math | Mrow | Mstyle | Mpadded | Unknown | Semantics | Mtd) | None = element {
            if let [only] = self.source.children(node)[..] {
                return self.silent_name(only);
            }
        }
        let name = match element {
            Some(MathElement::Mspace) => Some(self.phrases().space),
            Some(MathElement::Mo) => self
                .token_key(node)
                .and_then(|key| lookup(self.lexicon.silent_names, &key)),
            _ => None,
        };
        name.unwrap_or(self.phrases().blank).to_owned()
    }

    /// The speech for `node`, or `None` if it is not spoken (inside a row, it is skipped).
    fn spoken(&self, node: S::Id, ctx: Context) -> Option<String> {
        use MathElement::*;
        let children = self.source.children(node);
        let child = |i: usize| children.get(i).copied();
        let Some(element) = self.source.element(node) else {
            return self.row(&children, ctx);
        };
        let p = self.phrases();
        Some(match element {
            Mi | Mn | Mo | Mtext | Ms => return self.token(node, element, ctx),
            Math | Mrow | Mstyle | Mpadded | Unknown => return self.row(&children, ctx),
            Semantics => return child(0).and_then(|c| self.spoken(c, ctx)),
            Mspace | Mphantom | NoneElement | Mprescripts | Maligngroup | Malignmark => {
                return None;
            }
            Merror => fill(p.error, &[&self.row_or_blank(&children, ctx)]),
            Mfrac => {
                if let (Some(top), Some(bottom)) = (child(0), child(1)) {
                    if let Some(derivative) = self.derivative(top, bottom, ctx) {
                        return Some(derivative);
                    }
                    if let Some(common) = self.common_fraction(top, bottom) {
                        return Some(common);
                    }
                }
                let simple = [child(0), child(1)]
                    .iter()
                    .all(|c| c.is_some_and(|c| self.is_fraction_operand(c)));
                let template = if simple {
                    p.fraction_simple
                } else {
                    p.fraction
                };
                fill(
                    template,
                    &[
                        &self.speak_opt(child(0), ctx),
                        &self.speak_opt(child(1), ctx),
                    ],
                )
            }
            Msqrt => fill(p.square_root, &[&self.row_or_blank(&children, ctx)]),
            Mroot => self.root(child(0), child(1), ctx),
            Msub => self.scripts(child(0), child(1), None, ctx),
            Msup => self.scripts(child(0), None, child(1), ctx),
            Msubsup => self.scripts(child(0), child(1), child(2), ctx),
            Munder => self.under_over(child(0), child(1), None, ctx),
            Mover => self.under_over(child(0), None, child(1), ctx),
            Munderover => self.under_over(child(0), child(1), child(2), ctx),
            Mmultiscripts => self.multiscripts(&children, ctx),
            Menclose => {
                let content = self.row_or_blank(&children, ctx);
                let notation = self.source.attribute(node, "notation");
                let template = notation
                    .as_deref()
                    .unwrap_or("longdiv")
                    .split_whitespace()
                    .find_map(|n| lookup(self.lexicon.enclosures, n))
                    .unwrap_or(p.enclosed);
                fill(template, &[&content])
            }
            Mtable => self.table(node, p.table_kind, ctx),
            Mtr | Mlabeledtr => {
                let skip = usize::from(element == Mlabeledtr);
                let cells = children
                    .iter()
                    .skip(skip)
                    .map(|&c| self.cell(c, ctx))
                    .collect();
                return join(cells, p.cell_separator);
            }
            Mtd => return self.row(&children, ctx),
        })
    }

    fn row_or_blank(&self, children: &[S::Id], ctx: Context) -> String {
        self.row(children, ctx)
            .unwrap_or_else(|| self.phrases().blank.to_owned())
    }

    // ----- tokens -----

    fn token(&self, node: S::Id, element: MathElement, ctx: Context) -> Option<String> {
        let text = self.source.text(node)?;
        let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if text.is_empty() {
            return None;
        }
        match element {
            MathElement::Mtext => Some(text),
            MathElement::Ms => Some(fill(self.phrases().string, &[&text])),
            MathElement::Mn => Some(plain(&text).0),
            _ => self.symbol_words(&text, ctx),
        }
    }

    fn lookup_symbol(&self, key: &str, ctx: Context) -> Option<&'static str> {
        ctx.symbols
            .and_then(|table| lookup(table, key))
            .or_else(|| lookup(self.lexicon.symbols, key))
            .or_else(|| lookup(self.lexicon.identifiers, key))
    }

    fn with_variant(&self, word: String, variant: Option<&str>) -> String {
        match variant.and_then(|v| lookup(self.lexicon.variants, v)) {
            Some(adjective) => fill(self.phrases().variant, &[adjective, &word]),
            None => word,
        }
    }

    fn symbol_words(&self, text: &str, ctx: Context) -> Option<String> {
        let (plain, variant) = plain(text);
        if let Some(word) = self.lookup_symbol(&plain, ctx) {
            return (!word.is_empty()).then(|| self.with_variant(word.to_owned(), variant));
        }
        if plain.chars().count() > 1 && plain.chars().all(char::is_alphanumeric) {
            // A multi-letter name: read as a word.
            return Some(self.with_variant(plain, variant));
        }
        join(
            text.chars()
                .filter_map(|c| self.char_word(c, ctx))
                .collect(),
            " ",
        )
    }

    fn char_word(&self, c: char, ctx: Context) -> Option<String> {
        if c.is_whitespace() {
            return None;
        }
        let (base, family) = split_family(c);
        let key = base.to_string();
        let word = if let Some(word) = self.lookup_symbol(&key, ctx) {
            if word.is_empty() {
                return None;
            }
            word.to_owned()
        } else if base.is_uppercase() {
            let lower: String = base.to_lowercase().collect();
            let name = lookup(self.lexicon.identifiers, &lower).unwrap_or(&key);
            fill(self.phrases().capital, &[name])
        } else {
            key
        };
        let variant = (!matches!(family, Family::Normal)).then(|| family.mathvariant());
        Some(self.with_variant(word, variant))
    }

    // ----- structure helpers -----

    /// Elements that are never spoken inside a row.
    fn is_visible(&self, node: S::Id) -> bool {
        use MathElement::*;
        match self.source.element(node) {
            Some(Mspace | Mphantom | NoneElement | Mprescripts | Maligngroup | Malignmark) => false,
            Some(Mo) => !self
                .token_key(node)
                .is_some_and(|key| lookup(self.lexicon.symbols, &key) == Some("")),
            _ => true,
        }
    }

    fn visible(&self, nodes: &[S::Id]) -> Vec<S::Id> {
        nodes
            .iter()
            .copied()
            .filter(|&n| self.is_visible(n))
            .collect()
    }

    /// Looks through rows with a single visible child.
    fn unwrap(&self, mut node: S::Id) -> S::Id {
        use MathElement::*;
        while let Some(Math | Mrow | Mstyle | Mpadded | Unknown | Semantics) =
            self.source.element(node)
        {
            let children = self.source.children(node);
            let inner = if self.source.element(node) == Some(Semantics) {
                children.first().copied()
            } else {
                match self.visible(&children)[..] {
                    [only] => Some(only),
                    _ => None,
                }
            };
            match inner {
                Some(inner) => node = inner,
                None => break,
            }
        }
        node
    }

    /// A single token (possibly wrapped in rows).
    fn is_simple(&self, node: S::Id) -> bool {
        let node = self.unwrap(node);
        self.source.element(node).is_some_and(MathElement::is_token)
    }

    /// A part of a fraction that needs no "numerator ... denominator" around it: a token, or a
    /// token with a named power. "n squared" ends by itself; "x to the n" would not ("x to the
    /// n over 2" could be x to the n/2), so other powers don't qualify.
    fn is_fraction_operand(&self, node: S::Id) -> bool {
        if self.is_simple(node) {
            return true;
        }
        let node = self.unwrap(node);
        if self.source.element(node) != Some(MathElement::Msup) {
            return false;
        }
        match self.source.children(node)[..] {
            [base, exponent] => {
                self.is_simple(base)
                    && self
                        .flat_key(exponent)
                        .is_some_and(|k| lookup(self.lexicon.powers, &k).is_some())
            }
            _ => false,
        }
    }

    /// A large operator, with or without limits (`∑`, `lim` under `x → 0`, `∫` with scripts).
    fn is_large_operator(&self, node: S::Id) -> bool {
        use MathElement::*;
        let node = self.unwrap(node);
        match self.source.element(node) {
            Some(Munder | Mover | Munderover | Msub | Msup | Msubsup) => self
                .large_operator(self.source.children(node).first().copied())
                .is_some(),
            Some(Mi | Mo) => self.large_operator(Some(node)).is_some(),
            Some(Mfrac) => self.is_derivative_operator(node),
            _ => false,
        }
    }

    /// Whether `node`, right after a large operator, is what the operator applies to: not an
    /// operator or relation (`∑ = …`), except an opening fence. An invisible function
    /// application is not an operand either; it says "of" itself.
    fn is_operand(&self, node: S::Id) -> bool {
        match self.source.element(node) {
            Some(MathElement::Mo) => self
                .token_key(node)
                .is_some_and(|k| FENCE_PAIRS.iter().any(|(open, _)| *open == k)),
            _ => true,
        }
    }

    /// The plain text of a single token (possibly wrapped in rows), for table lookups.
    fn token_key(&self, node: S::Id) -> Option<String> {
        let node = self.unwrap(node);
        if !self.source.element(node).is_some_and(MathElement::is_token) {
            return None;
        }
        let text = self.source.text(node)?;
        Some(plain(&text).0)
    }

    /// The plain text of a token, or of a row of tokens (`−1` written as `<mo>−</mo><mn>1</mn>`),
    /// for table lookups.
    fn flat_key(&self, node: S::Id) -> Option<String> {
        let node = self.unwrap(node);
        if let Some(key) = self.token_key(node) {
            return Some(key);
        }
        if self.source.element(node) != Some(MathElement::Mrow) {
            return None;
        }
        self.visible(&self.source.children(node))
            .into_iter()
            .map(|child| self.token_key(child))
            .collect()
    }

    /// The fence text of an `<mo>` child of a row.
    fn fence_key(&self, node: S::Id) -> Option<String> {
        (self.source.element(node) == Some(MathElement::Mo))
            .then(|| self.token_key(node))
            .flatten()
    }

    // ----- rows and fences -----

    fn row(&self, children: &[S::Id], ctx: Context) -> Option<String> {
        let mut parts = Vec::new();
        let mut prev: Option<S::Id> = None;
        let mut i = 0;
        while i < children.len() {
            let child = children[i];
            if let Some((j, speech)) = self.fenced_run(children, i, prev, ctx) {
                parts.push(speech);
                prev = Some(children[j]);
                i = j + 1;
                continue;
            }
            if let Some(speech) = self.after_function(child, prev, ctx) {
                parts.push(speech);
            } else if self.is_prefix_minus(child, prev, &children[i + 1..]) {
                parts.push(self.phrases().negative.to_owned());
            } else if let Some(speech) = self.spoken(child, ctx) {
                parts.push(speech);
                let next = children[i + 1..].iter().find(|&&n| self.is_visible(n));
                if self.is_large_operator(child) && next.is_some_and(|&n| self.is_operand(n)) {
                    parts.push(self.phrases().operand.to_owned());
                }
            }
            if self.is_visible(child) {
                prev = Some(child);
            }
            i += 1;
        }
        join(parts, " ")
    }

    /// A fenced group starting at `children[i]` that is spoken as a whole: the index of its
    /// last node and its speech.
    fn fenced_run(
        &self,
        children: &[S::Id],
        i: usize,
        prev: Option<S::Id>,
        ctx: Context,
    ) -> Option<(usize, String)> {
        let open = self.fence_key(children[i])?;
        let close = FENCE_PAIRS.iter().find(|(o, _)| *o == open)?.1;
        let mut depth = 0;
        let mut j = i + 1;
        let end = loop {
            let Some(&node) = children.get(j) else {
                // No closing fence: `{` before a table is a list of cases.
                return self
                    .cases(&open, &children[i + 1..], ctx)
                    .map(|speech| (children.len() - 1, speech));
            };
            match self.fence_key(node).as_deref() {
                Some(k) if k == close && depth == 0 => break j,
                Some(k) if k == close => depth -= 1,
                Some(k) if k == open => depth += 1,
                _ => {}
            }
            j += 1;
        };
        let speech = self.fenced(&open, close, &children[i + 1..end], prev, ctx)?;
        Some((end, speech))
    }

    fn fenced(
        &self,
        open: &str,
        close: &str,
        inner: &[S::Id],
        prev: Option<S::Id>,
        ctx: Context,
    ) -> Option<String> {
        let p = self.phrases();
        let visible = self.visible(inner);
        if let [only] = visible[..] {
            let only = self.unwrap(only);
            match self.source.element(only) {
                Some(MathElement::Mtable) => {
                    if let Some(&(_, _, kind)) = self
                        .lexicon
                        .table_fences
                        .iter()
                        .find(|(o, c, _)| *o == open && *c == close)
                    {
                        return Some(self.table(only, kind, ctx));
                    }
                }
                Some(MathElement::Mfrac) if (open, close) == ("(", ")") && self.is_stack(only) => {
                    let parts = self.source.children(only);
                    let part = |i: usize| self.speak_opt(parts.get(i).copied(), ctx);
                    return Some(fill(p.binomial, &[&part(0), &part(1)]));
                }
                _ => {}
            }
        }
        if let Some(&(_, _, template)) = self
            .lexicon
            .fences
            .iter()
            .find(|(o, c, _)| *o == open && *c == close)
        {
            return Some(fill(template, &[&self.row_or_blank(inner, ctx)]));
        }
        if (open, close) == ("{", "}") {
            return Some(self.set(inner, ctx));
        }
        if (open, close) == ("(", ")") {
            return self.call(prev, inner, ctx);
        }
        None
    }

    /// Parentheses right after something that takes an argument.
    ///
    /// - after a function application or a function (`sin`, a single letter, `f^{-1}`), a
    ///   single term drops its parentheses: "f of x", "sine of 2 x";
    /// - `f`, `g` and `h` are read as functions whatever the argument: "f of open paren x plus
    ///   1 close paren" (a single other letter may be a factor: `a(b + c)`);
    /// - inside the arguments of a function, `∣` reads "given" ([`Lexicon::argument_symbols`]).
    fn call(&self, prev: Option<S::Id>, inner: &[S::Id], ctx: Context) -> Option<String> {
        let p = self.phrases();
        let callee = self.callee(prev?)?;
        let args = Context {
            symbols: Some(self.lexicon.argument_symbols),
        };
        let applied = callee == Callee::Application;
        let of = |speech: String| {
            if applied {
                speech
            } else {
                format!("{} {speech}", p.operand)
            }
        };
        if self.is_term(inner) {
            return Some(of(self.row_or_blank(inner, args)));
        }
        let parenthesized = || {
            let words = |key: &str| self.symbol_words(key, ctx).unwrap_or_default();
            join(
                vec![words("("), self.row_or_blank(inner, args), words(")")],
                " ",
            )
            .unwrap_or_default()
        };
        match callee {
            Callee::Application | Callee::NamedFunction => Some(of(parenthesized())),
            Callee::Function => Some(parenthesized()),
        }
    }

    /// How `prev` takes a parenthesized argument after it, if it does.
    fn callee(&self, prev: S::Id) -> Option<Callee> {
        use MathElement::*;
        if self.fence_key(prev).as_deref() == Some(FUNCTION_APPLICATION) {
            return Some(Callee::Application);
        }
        let mut node = self.unwrap(prev);
        // `f^{-1}`, `sin^2`, `f_n`
        if let Some(Msup | Msub | Msubsup) = self.source.element(node) {
            node = self.unwrap(*self.source.children(node).first()?);
        }
        if self.source.element(node) != Some(Mi) {
            return None;
        }
        let key = self.token_key(node)?;
        let mut chars = key.chars();
        let single = chars.next().filter(|_| chars.next().is_none());
        if FUNCTION_LETTERS.contains(&key.as_str()) {
            Some(Callee::NamedFunction)
        } else if single.is_some_and(|c| !c.is_ascii_lowercase())
            || (single.is_none() && lookup(self.lexicon.identifiers, &key).is_some())
        {
            // `F(b)`, `P(A)`, `φ(x)`, `sin(x)`; not `b(…)`, usually a product.
            Some(Callee::Function)
        } else {
            None
        }
    }

    /// A single term: no operators other than function applications ("2 x", "n log n").
    fn is_term(&self, nodes: &[S::Id]) -> bool {
        let visible = self.visible(nodes);
        !visible.is_empty()
            && visible.iter().all(|&n| {
                self.source.element(n) != Some(MathElement::Mo)
                    || self.fence_key(n).as_deref() == Some(FUNCTION_APPLICATION)
            })
    }

    /// `<mfrac linethickness="0">`.
    fn is_stack(&self, node: S::Id) -> bool {
        self.source
            .attribute(node, "linethickness")
            .and_then(|t| {
                let t = t.trim();
                let number = t.trim_end_matches(|c: char| c.is_ascii_alphabetic() || c == '%');
                number.parse::<f32>().ok()
            })
            .is_some_and(|t| t == 0.0)
    }

    /// `{ ... }`: a set, or with `:` or `∣` a set-builder.
    fn set(&self, inner: &[S::Id], ctx: Context) -> String {
        let p = self.phrases();
        let split = inner.iter().position(|&n| {
            self.source.element(n) == Some(MathElement::Mo)
                && matches!(self.fence_key(n).as_deref(), Some(":" | "∣"))
        });
        match split {
            Some(s) => {
                let members = Context {
                    symbols: Some(self.lexicon.set_symbols),
                };
                fill(
                    p.set_builder,
                    &[
                        &self.row_or_blank(&inner[..s], members),
                        &self.row_or_blank(&inner[s + 1..], ctx),
                    ],
                )
            }
            None => match self.row(inner, ctx) {
                Some(members) => fill(p.set, &[&members]),
                None => p.empty_set.to_owned(),
            },
        }
    }

    /// `{` followed by nothing but a table: "2 cases; case 1: x, if x is positive; ...".
    fn cases(&self, open: &str, rest: &[S::Id], ctx: Context) -> Option<String> {
        if open != "{" {
            return None;
        }
        let [table] = self.visible(rest)[..] else {
            return None;
        };
        let table = self.unwrap(table);
        if self.source.element(table) != Some(MathElement::Mtable) {
            return None;
        }
        let p = self.phrases();
        let rows = self.visible(&self.source.children(table));
        let cases: Vec<String> = rows
            .iter()
            .enumerate()
            .map(|(index, &row)| {
                let cells = match self.source.element(row) {
                    Some(MathElement::Mtr) => self.source.children(row),
                    _ => vec![row],
                };
                let spoken = |i: usize| cells.get(i).map(|&c| self.cell(c, ctx));
                let case = match (spoken(0), spoken(1)) {
                    (Some(value), Some(condition)) => {
                        // `\text{if } x > 0` and `\text{otherwise}` bring their own word.
                        let template = if self.starts_with_text(cells[1]) {
                            p.case_text
                        } else {
                            p.case_condition
                        };
                        fill(template, &[&value, &condition])
                    }
                    (Some(value), None) => value,
                    _ => p.blank.to_owned(),
                };
                fill(p.case, &[&(index + 1).to_string(), &case])
            })
            .collect();
        Some(fill(
            p.cases,
            &[&rows.len().to_string(), &cases.join(p.row_separator)],
        ))
    }

    fn starts_with_text(&self, node: S::Id) -> bool {
        let mut node = node;
        loop {
            match self.source.element(node) {
                Some(MathElement::Mtext) => return true,
                Some(MathElement::Mtd | MathElement::Mrow | MathElement::Mstyle) => {
                    match self.visible(&self.source.children(node)).first() {
                        Some(&first) => node = first,
                        None => return false,
                    }
                }
                _ => return false,
            }
        }
    }

    /// A row child that is itself a parenthesized row, right after something that takes an
    /// argument: `f⁡(x)` written as `<mi>f</mi><mo>⁡</mo><mrow><mo>(</mo>…</mrow>`.
    fn after_function(&self, child: S::Id, prev: Option<S::Id>, ctx: Context) -> Option<String> {
        let prev = prev?;
        if !matches!(self.source.element(child), Some(MathElement::Mrow)) {
            return None;
        }
        let children = self.visible(&self.source.children(child));
        let (first, rest) = children.split_first()?;
        let (last, inner) = rest.split_last()?;
        if (
            self.fence_key(*first)?.as_str(),
            self.fence_key(*last)?.as_str(),
        ) != ("(", ")")
        {
            return None;
        }
        self.call(Some(prev), inner, ctx)
    }

    /// `1/2` → "one half", `3/4` → "three quarters", for numbers in the lexicon's tables.
    fn common_fraction(&self, numerator: S::Id, denominator: S::Id) -> Option<String> {
        let number = |node: S::Id| {
            let node = self.unwrap(node);
            (self.source.element(node) == Some(MathElement::Mn))
                .then(|| self.token_key(node))
                .flatten()
        };
        let (top, bottom) = (number(numerator)?, number(denominator)?);
        let count = lookup(self.lexicon.cardinals, &top)?;
        let &(_, singular, plural) = self
            .lexicon
            .fraction_denominators
            .iter()
            .find(|(d, _, _)| *d == bottom)?;
        let name = if top == "1" { singular } else { plural };
        Some(fill(self.phrases().common_fraction, &[count, name]))
    }

    // ----- derivatives -----

    /// `d` or `∂`, possibly with an order (`∂²`): (is partial, order).
    fn differential(&self, node: S::Id) -> Option<(bool, Option<String>)> {
        let node = self.unwrap(node);
        let (base, order) = match self.source.element(node) {
            Some(MathElement::Msup) => match self.source.children(node)[..] {
                [base, order] => (base, Some(self.token_key(order)?)),
                _ => return None,
            },
            _ => (node, None),
        };
        match self.token_key(base)?.as_str() {
            "d" | "ⅆ" => Some((false, order)),
            "∂" => Some((true, order)),
            _ => None,
        }
    }

    /// The parts of `d^n y / d x^n` and `∂²f / ∂x ∂y`: (partial, order, function, variables).
    #[allow(clippy::type_complexity)]
    fn derivative_parts(
        &self,
        numerator: S::Id,
        denominator: S::Id,
    ) -> Option<(bool, Option<String>, Vec<S::Id>, Vec<S::Id>)> {
        let row = |node: S::Id| {
            let node = self.unwrap(node);
            match self.source.element(node) {
                Some(MathElement::Mrow) => self.visible(&self.source.children(node)),
                _ => vec![node],
            }
        };
        let top = row(numerator);
        let (&d_top, function) = top.split_first()?;
        let (partial, order) = self.differential(d_top)?;
        // The denominator: pairs of a differential and a variable, `dx^2` carrying an order.
        let bottom = row(denominator);
        if bottom.is_empty() || bottom.len() % 2 != 0 {
            return None;
        }
        let mut variables = Vec::new();
        let mut powers = Vec::new();
        for pair in bottom.chunks(2) {
            let (partial_bottom, None) = self.differential(pair[0])? else {
                return None;
            };
            if partial != partial_bottom {
                return None;
            }
            let variable = self.unwrap(pair[1]);
            let (variable, power) = match self.source.element(variable) {
                Some(MathElement::Msup) => match self.source.children(variable)[..] {
                    [base, power] => (base, Some(self.token_key(power)?)),
                    _ => return None,
                },
                _ => (variable, None),
            };
            if !self.is_simple(variable) {
                return None;
            }
            variables.push(variable);
            powers.push(power);
        }
        // The orders must agree: `d²/dx²`, or `∂²/∂x∂y` with one variable per order.
        let consistent = match (&order, &powers[..]) {
            (order, [power]) => order == power,
            (Some(order), _) => {
                powers.iter().all(Option::is_none) && *order == variables.len().to_string()
            }
            (None, _) => false,
        };
        consistent.then(|| (partial, order, function.to_vec(), variables))
    }

    /// `dy/dx` or `d/dx`, as speech, or `None` for any other fraction.
    fn derivative(&self, numerator: S::Id, denominator: S::Id, ctx: Context) -> Option<String> {
        let p = self.phrases();
        let (partial, order, function, variables) =
            self.derivative_parts(numerator, denominator)?;
        let mut kind = if partial {
            p.partial_derivative
        } else {
            p.derivative
        }
        .to_owned();
        if let Some(order) = order {
            let ordinal = lookup(self.lexicon.ordinals, &order)?;
            kind = fill(p.ordinal_derivative, &[ordinal, &kind]);
        }
        let variables: Vec<String> = variables.iter().map(|&v| self.speak(v, ctx)).collect();
        let variable = variables.join(p.variable_separator);
        Some(match self.row(&function, ctx) {
            Some(function) => fill(p.derivative_of, &[&kind, &function, &variable]),
            None => fill(p.derivative_operator, &[&kind, &variable]),
        })
    }

    /// `d/dx`: a derivative with nothing to differentiate in its numerator.
    fn is_derivative_operator(&self, node: S::Id) -> bool {
        let node = self.unwrap(node);
        if self.source.element(node) != Some(MathElement::Mfrac) {
            return false;
        }
        match self.source.children(node)[..] {
            [top, bottom] => self
                .derivative_parts(top, bottom)
                .is_some_and(|(_, _, function, _)| function.is_empty()),
            _ => false,
        }
    }

    fn is_prefix_minus(&self, child: S::Id, prev: Option<S::Id>, rest: &[S::Id]) -> bool {
        let is_minus = self.source.element(child) == Some(MathElement::Mo)
            && matches!(self.token_key(child).as_deref(), Some("-" | "−"));
        let operand_follows = rest.iter().any(|&n| self.is_visible(n));
        let prev_is_operator = match prev {
            None => true,
            Some(p) => {
                self.source.element(p) == Some(MathElement::Mo)
                    && !self
                        .token_key(p)
                        .is_some_and(|k| POSTFIX.contains(&k.as_str()))
            }
        };
        is_minus && operand_follows && prev_is_operator
    }

    // ----- radicals, scripts -----

    fn root(&self, base: Option<S::Id>, index: Option<S::Id>, ctx: Context) -> String {
        let p = self.phrases();
        let radicand = self.speak_opt(base, ctx);
        let key = index.and_then(|i| self.token_key(i));
        if let Some(name) = key
            .as_deref()
            .and_then(|k| lookup(self.lexicon.root_indices, k))
        {
            return fill(p.named_root, &[name, &radicand]);
        }
        let index_is_identifier = index
            .map(|i| self.unwrap(i))
            .is_some_and(|i| self.source.element(i) == Some(MathElement::Mi));
        let template = if index_is_identifier {
            p.simple_root
        } else {
            p.root
        };
        fill(template, &[&self.speak_opt(index, ctx), &radicand])
    }

    fn large_operator(&self, base: Option<S::Id>) -> Option<(&'static str, &'static str)> {
        let key = self.token_key(base?)?;
        self.lexicon
            .large_operators
            .iter()
            .find(|(op, _, _)| *op == key)
            .map(|&(_, name, connective)| (name, connective))
    }

    fn limits(
        &self,
        (name, connective): (&str, &str),
        under: Option<S::Id>,
        over: Option<S::Id>,
        ctx: Context,
    ) -> String {
        let p = self.phrases();
        let under = under.map(|u| {
            self.speak(
                u,
                Context {
                    symbols: Some(self.lexicon.limit_symbols),
                },
            )
        });
        let over = over.map(|o| self.speak(o, ctx));
        match (under, over) {
            (Some(u), Some(o)) => fill(p.large_operator_both, &[name, &u, &o]),
            (Some(u), None) => fill(p.large_operator_under, &[name, connective, &u]),
            (None, Some(o)) => fill(p.large_operator_over, &[name, &o]),
            (None, None) => name.to_owned(),
        }
    }

    fn scripts(
        &self,
        base: Option<S::Id>,
        sub: Option<S::Id>,
        sup: Option<S::Id>,
        ctx: Context,
    ) -> String {
        if let Some(op) = self.large_operator(base) {
            return self.limits(op, sub, sup, ctx);
        }
        let p = self.phrases();
        let base_speech = self.speak_opt(base, ctx);
        let power = sup
            .and_then(|s| self.flat_key(s))
            .and_then(|k| lookup(self.lexicon.powers, &k));
        let sup_simple = sup.is_some_and(|s| self.is_simple(s));
        match (sub, sup) {
            (Some(sub), None) => {
                let template = if self.is_simple(sub) {
                    p.subscript_simple
                } else {
                    p.subscript
                };
                fill(template, &[&base_speech, &self.speak(sub, ctx)])
            }
            (None, Some(sup)) => match power {
                Some(name) => fill(p.named_power, &[&base_speech, name]),
                None => {
                    let template = if sup_simple { p.power_simple } else { p.power };
                    fill(template, &[&base_speech, &self.speak(sup, ctx)])
                }
            },
            (Some(sub), Some(sup)) => {
                let sub = self.speak(sub, ctx);
                match power {
                    Some(name) => fill(p.sub_named_power, &[&base_speech, &sub, name]),
                    None => fill(p.sub_power, &[&base_speech, &sub, &self.speak(sup, ctx)]),
                }
            }
            (None, None) => base_speech,
        }
    }

    fn under_over(
        &self,
        base: Option<S::Id>,
        under: Option<S::Id>,
        over: Option<S::Id>,
        ctx: Context,
    ) -> String {
        if let Some(op) = self.large_operator(base) {
            return self.limits(op, under, over, ctx);
        }
        let p = self.phrases();
        let accent = |n: Option<S::Id>| {
            n.and_then(|n| self.token_key(n))
                .and_then(|k| lookup(self.lexicon.accents, &k))
        };
        let base_speech = self.speak_opt(base, ctx);
        match (under, over) {
            (Some(u), None) => match accent(Some(u)) {
                Some(name) => fill(p.accent, &[&base_speech, name]),
                None => fill(p.under, &[&base_speech, &self.speak(u, ctx)]),
            },
            (None, Some(o)) => match accent(Some(o)) {
                Some(name) => fill(p.accent, &[&base_speech, name]),
                None => fill(p.over, &[&base_speech, &self.speak(o, ctx)]),
            },
            (Some(u), Some(o)) => fill(
                p.under_over,
                &[&base_speech, &self.speak(u, ctx), &self.speak(o, ctx)],
            ),
            (None, None) => base_speech,
        }
    }

    fn multiscripts(&self, children: &[S::Id], ctx: Context) -> String {
        let p = self.phrases();
        let base = self.speak_opt(children.first().copied(), ctx);
        let rest = children.get(1..).unwrap_or(&[]);
        let split = rest
            .iter()
            .position(|&c| self.source.element(c) == Some(MathElement::Mprescripts));
        let (post, pre) = match split {
            Some(s) => (&rest[..s], &rest[s + 1..]),
            None => (rest, &[][..]),
        };
        let mut items = Vec::new();
        let mut add = |scripts: &[S::Id], sub_template: &str, sup_template: &str| {
            for pair in scripts.chunks(2) {
                for (&script, template) in pair.iter().zip([sub_template, sup_template]) {
                    if self.source.element(script) != Some(MathElement::NoneElement) {
                        items.push(fill(template, &[&self.speak(script, ctx)]));
                    }
                }
            }
        };
        add(pre, p.pre_subscript, p.pre_superscript);
        add(post, p.post_subscript, p.post_superscript);
        match join(items, p.list_separator) {
            Some(list) => fill(p.multiscripts, &[&base, &list]),
            None => base,
        }
    }

    // ----- tables -----

    fn cell(&self, cell: S::Id, ctx: Context) -> String {
        if self.source.element(cell) == Some(MathElement::Mtd) {
            self.row_or_blank(&self.source.children(cell), ctx)
        } else {
            self.speak(cell, ctx)
        }
    }

    fn table(&self, table: S::Id, kind: &str, ctx: Context) -> String {
        let p = self.phrases();
        let rows = self.visible(&self.source.children(table));
        if rows.is_empty() {
            return fill(p.empty_table, &[kind]);
        }
        let mut columns = 0;
        let spoken_rows: Vec<String> = rows
            .iter()
            .enumerate()
            .map(|(index, &row)| {
                let number = (index + 1).to_string();
                let element = self.source.element(row);
                let children = self.source.children(row);
                let (label, cells) = match element {
                    Some(MathElement::Mtr) => (None, children),
                    Some(MathElement::Mlabeledtr) => match children.split_first() {
                        Some((label, cells)) => (Some(*label), cells.to_vec()),
                        None => (None, children),
                    },
                    _ => (None, vec![row]),
                };
                columns = columns.max(cells.len());
                let cells = cells.iter().map(|&c| self.cell(c, ctx)).collect::<Vec<_>>();
                let cells = join(cells, p.cell_separator).unwrap_or_else(|| p.blank.to_owned());
                match label {
                    Some(label) => fill(p.labeled_row, &[&number, &self.cell(label, ctx), &cells]),
                    None => fill(p.row, &[&number, &cells]),
                }
            })
            .collect();
        fill(
            p.table,
            &[
                &rows.len().to_string(),
                &columns.to_string(),
                kind,
                &spoken_rows.join(p.row_separator),
            ],
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fill_replaces_numbered_placeholders() {
        assert_eq!(
            fill("the {0} by {1} {2}; end {2}", &["2", "3", "matrix"]),
            "the 2 by 3 matrix; end matrix"
        );
        assert_eq!(fill("{x} {0}", &["a"]), "{x} a");
    }

    /// Every template only refers to the arguments its documentation promises.
    #[test]
    fn english_templates_use_known_placeholders() {
        let p = ENGLISH.phrases;
        let templates: &[(&str, usize)] = &[
            (p.blank, 0),
            (p.space, 0),
            (p.capital, 1),
            (p.variant, 2),
            (p.negative, 0),
            (p.string, 1),
            (p.error, 1),
            (p.enclosed, 1),
            (p.fraction_simple, 2),
            (p.fraction, 2),
            (p.square_root, 1),
            (p.named_root, 2),
            (p.simple_root, 2),
            (p.root, 2),
            (p.named_power, 2),
            (p.power_simple, 2),
            (p.power, 2),
            (p.subscript_simple, 2),
            (p.subscript, 2),
            (p.sub_named_power, 3),
            (p.sub_power, 3),
            (p.large_operator_under, 3),
            (p.large_operator_over, 2),
            (p.large_operator_both, 3),
            (p.operand, 0),
            (p.common_fraction, 2),
            (p.binomial, 2),
            (p.set, 1),
            (p.set_builder, 2),
            (p.empty_set, 0),
            (p.cases, 2),
            (p.case, 2),
            (p.case_condition, 2),
            (p.case_text, 2),
            (p.derivative, 0),
            (p.partial_derivative, 0),
            (p.ordinal_derivative, 2),
            (p.derivative_of, 3),
            (p.derivative_operator, 2),
            (p.accent, 2),
            (p.under, 2),
            (p.over, 2),
            (p.under_over, 3),
            (p.multiscripts, 2),
            (p.post_subscript, 1),
            (p.post_superscript, 1),
            (p.pre_subscript, 1),
            (p.pre_superscript, 1),
            (p.table, 4),
            (p.empty_table, 1),
            (p.table_kind, 0),
            (p.row, 2),
            (p.labeled_row, 3),
        ];
        let fences = ENGLISH.fences.iter().map(|f| f.2);
        let enclosures = ENGLISH.enclosures.iter().map(|e| e.1);
        for (template, arity) in templates
            .iter()
            .copied()
            .chain(fences.chain(enclosures).map(|t| (t, 1)))
        {
            for i in 0..arity {
                assert!(
                    template.contains(&format!("{{{i}}}")),
                    "{template:?} lacks {{{i}}}"
                );
            }
            assert!(
                !template.contains(&format!("{{{arity}}}")),
                "{template:?} uses {{{arity}}}"
            );
        }
    }

    #[test]
    fn english_tables_have_no_duplicate_keys() {
        for table in [
            ENGLISH.symbols,
            ENGLISH.identifiers,
            ENGLISH.powers,
            ENGLISH.accents,
            ENGLISH.root_indices,
            ENGLISH.variants,
            ENGLISH.enclosures,
        ] {
            let mut keys: Vec<_> = table.iter().map(|(k, _)| *k).collect();
            keys.sort();
            let len = keys.len();
            keys.dedup();
            assert_eq!(len, keys.len(), "duplicate key in a table");
        }
    }
}
