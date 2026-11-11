//! Speech: one golden test per construct, plus a property test that no element is silent.

use formel::speech::{speak, speak_document};
use formel::standalone::{MathDocument, NodeId, StyleOptions};
use quickcheck::{Arbitrary, Gen, QuickCheck};

mod util;
use crate::util::TEST_FONT;

/// Every node reachable from the root.
fn all_nodes(doc: &MathDocument) -> Vec<NodeId> {
    let mut nodes = Vec::new();
    let mut stack: Vec<NodeId> = doc.root().into_iter().collect();
    while let Some(node) = stack.pop() {
        nodes.push(node);
        stack.extend(&doc.node(node).children);
    }
    nodes
}

/// A non-empty, trimmed string with single spaces.
fn assert_well_formed(speech: &str, context: &dyn std::fmt::Debug) {
    assert!(!speech.trim().is_empty(), "empty speech for {context:?}");
    assert_eq!(
        speech,
        speech.trim(),
        "untrimmed speech {speech:?} for {context:?}"
    );
    assert!(
        !speech.contains("  "),
        "double space in {speech:?} for {context:?}"
    );
}

#[cfg(feature = "mathml_parser")]
mod golden {
    use super::*;
    use formel::mathmlparser;

    fn say(xml: &str) -> String {
        let doc = mathmlparser::parse(xml.as_bytes()).expect("valid MathML");
        speak_document(&doc, doc.root().unwrap())
    }

    /// The host-tree path, where `<mi>` text arrives as mathematical italic and `-` as `−`.
    fn say_styled(xml: &str) -> String {
        let doc = mathmlparser::parse(xml.as_bytes()).expect("valid MathML");
        TEST_FONT.with(|font| {
            speak(
                &doc.style(font, StyleOptions::default()),
                doc.root().unwrap(),
            )
        })
    }

    const QUADRATIC_ROOT: &str = "<msqrt><msup><mi>b</mi><mn>2</mn></msup><mo>-</mo>\
        <mn>4</mn><mi>a</mi><mi>c</mi></msqrt>";

    #[test]
    fn square_root() {
        assert_eq!(
            say(QUADRATIC_ROOT),
            "the square root of b squared minus 4 a c"
        );
    }

    #[test]
    fn styled_tree_speaks_like_the_document() {
        assert_eq!(say_styled(QUADRATIC_ROOT), say(QUADRATIC_ROOT));
        assert_eq!(
            say_styled("<mi>A</mi><mo>-</mo><mi>x</mi>"),
            "capital A minus x"
        );
    }

    #[test]
    fn implicit_multiplication() {
        assert_eq!(say("<mn>2</mn><mi>a</mi>"), "2 a");
        assert_eq!(say("<mn>2</mn><mo>&#x2062;</mo><mi>a</mi>"), "2 a");
    }

    #[test]
    fn operators() {
        assert_eq!(
            say("<mi>a</mi><mo>+</mo><mi>b</mi><mo>=</mo><mi>c</mi>"),
            "a plus b equals c"
        );
        assert_eq!(
            say("<mi>x</mi><mo>≤</mo><mn>1</mn>"),
            "x is less than or equal to 1"
        );
        assert_eq!(say("<mi>a</mi><mo>±</mo><mi>b</mi>"), "a plus or minus b");
    }

    #[test]
    fn prefix_minus() {
        assert_eq!(say("<mo>-</mo><mi>b</mi>"), "negative b");
        assert_eq!(
            say("<mi>x</mi><mo>=</mo><mo>-</mo><mn>1</mn>"),
            "x equals negative 1"
        );
        assert_eq!(say("<mi>a</mi><mo>-</mo><mi>b</mi>"), "a minus b");
        assert_eq!(
            say("<mo>(</mo><mi>a</mi><mo>)</mo><mo>-</mo><mi>b</mi>"),
            "open paren a close paren minus b"
        );
    }

    #[test]
    fn identifiers() {
        assert_eq!(say("<mi>α</mi><mi>Ω</mi>"), "alpha capital omega");
        assert_eq!(say("<mi>X</mi>"), "capital X");
        assert_eq!(say("<mi mathvariant='bold'>v</mi>"), "bold v");
        assert_eq!(say("<mi>ℝ</mi>"), "double-struck capital R");
        assert_eq!(say("<mi>∞</mi>"), "infinity");
    }

    #[test]
    fn functions() {
        assert_eq!(say("<mi>sin</mi><mi>x</mi>"), "sine x");
        assert_eq!(say("<mi>sin</mi><mo>&#x2061;</mo><mi>x</mi>"), "sine of x");
        assert_eq!(
            say("<mi>ln</mi><mo>&#x2061;</mo><mi>x</mi>"),
            "natural log of x"
        );
        assert_eq!(
            say("<mi>f</mi><mo>&#x2061;</mo><mrow><mo>(</mo><mi>x</mi><mo>)</mo></mrow>"),
            "f of x"
        );
        assert_eq!(
            say("<mi>f</mi><mo>&#x2061;</mo><mo>(</mo><mi>x</mi><mo>)</mo>"),
            "f of x"
        );
        assert_eq!(
            say(
                "<mi>sin</mi><mo>&#x2061;</mo><mrow><mo>(</mo><mi>x</mi><mo>+</mo><mi>y</mi><mo>)</mo></mrow>"
            ),
            "sine of open paren x plus y close paren"
        );
    }

    #[test]
    fn fractions() {
        assert_eq!(say("<mfrac><mi>a</mi><mi>b</mi></mfrac>"), "a over b");
        assert_eq!(
            say("<mfrac><mrow><mi>a</mi><mo>+</mo><mn>1</mn></mrow><mn>2</mn></mfrac>"),
            "the fraction with numerator a plus 1 and denominator 2"
        );
    }

    #[test]
    fn quadratic_formula() {
        let xml = "<mi>x</mi><mo>=</mo><mfrac><mrow><mo>-</mo><mi>b</mi><mo>±</mo>\
            <msqrt><msup><mi>b</mi><mn>2</mn></msup><mo>-</mo><mn>4</mn><mi>a</mi><mi>c</mi></msqrt>\
            </mrow><mrow><mn>2</mn><mi>a</mi></mrow></mfrac>";
        assert_eq!(
            say(xml),
            "x equals the fraction with numerator negative b plus or minus the square root of \
             b squared minus 4 a c and denominator 2 a"
        );
    }

    #[test]
    fn roots() {
        assert_eq!(
            say("<mroot><mi>x</mi><mn>3</mn></mroot>"),
            "the cube root of x"
        );
        assert_eq!(
            say("<mroot><mi>x</mi><mi>n</mi></mroot>"),
            "the n-th root of x"
        );
        assert_eq!(
            say("<mroot><mi>x</mi><mrow><mi>n</mi><mo>+</mo><mn>1</mn></mrow></mroot>"),
            "the root with index n plus 1 of x"
        );
    }

    #[test]
    fn superscripts() {
        assert_eq!(say("<msup><mi>x</mi><mn>3</mn></msup>"), "x cubed");
        assert_eq!(say("<msup><mi>e</mi><mi>x</mi></msup>"), "e to the x");
        assert_eq!(
            say(
                "<msup><mi>e</mi><mrow><mi>x</mi><mo>+</mo><mn>1</mn></mrow></msup><mo>-</mo><mn>1</mn>"
            ),
            "e to the power x plus 1, end exponent minus 1"
        );
        assert_eq!(say("<msup><mi>f</mi><mo>′</mo></msup>"), "f prime");
        assert_eq!(
            say("<msup><mi>sin</mi><mn>2</mn></msup><mi>x</mi>"),
            "sine squared x"
        );
    }

    #[test]
    fn subscripts() {
        assert_eq!(say("<msub><mi>x</mi><mi>i</mi></msub>"), "x sub i");
        assert_eq!(
            say("<msub><mi>a</mi><mrow><mi>i</mi><mi>j</mi></mrow></msub>"),
            "a sub i j, end sub"
        );
        assert_eq!(
            say("<msubsup><mi>x</mi><mi>i</mi><mn>2</mn></msubsup>"),
            "x sub i squared"
        );
        assert_eq!(
            say("<msubsup><mi>x</mi><mi>i</mi><mi>n</mi></msubsup>"),
            "x sub i, to the n"
        );
    }

    #[test]
    fn large_operators() {
        assert_eq!(
            say(
                "<munderover><mo>∑</mo><mrow><mi>i</mi><mo>=</mo><mn>1</mn></mrow><mi>n</mi></munderover><msub><mi>x</mi><mi>i</mi></msub>"
            ),
            "the sum from i equals 1 to n x sub i"
        );
        assert_eq!(
            say("<msubsup><mo>∫</mo><mn>0</mn><mn>1</mn></msubsup><mi>f</mi>"),
            "the integral from 0 to 1 f"
        );
        assert_eq!(
            say(
                "<munder><mo>lim</mo><mrow><mi>x</mi><mo>→</mo><mn>0</mn></mrow></munder><mo>&#x2061;</mo><mfrac><mn>1</mn><mi>x</mi></mfrac>"
            ),
            "the limit as x approaches 0 of 1 over x"
        );
        assert_eq!(
            say("<munder><mi>max</mi><mi>x</mi></munder>"),
            "the maximum over x"
        );
        assert_eq!(say("<mo>∑</mo><mi>x</mi>"), "the sum x");
        // outside a limit, an arrow is an arrow
        assert_eq!(
            say("<mi>f</mi><mo>:</mo><mi>A</mi><mo>→</mo><mi>B</mi>"),
            "f colon capital A right arrow capital B"
        );
    }

    #[test]
    fn accents_and_under_over() {
        assert_eq!(
            say("<mover accent='true'><mi>x</mi><mo>^</mo></mover>"),
            "x hat"
        );
        assert_eq!(
            say("<mover accent='true'><mi>x</mi><mo>&#x0302;</mo></mover>"),
            "x hat"
        );
        assert_eq!(say("<mover><mi>v</mi><mo>→</mo></mover>"), "v vector");
        assert_eq!(say("<mover><mi>x</mi><mo>¯</mo></mover>"), "x bar");
        assert_eq!(
            say("<mover><mi>A</mi><mi>B</mi></mover>"),
            "capital A with capital B above"
        );
        assert_eq!(
            say("<munder><mi>x</mi><mi>y</mi></munder>"),
            "x with y below"
        );
        assert_eq!(
            say("<munderover><mi>x</mi><mi>a</mi><mi>b</mi></munderover>"),
            "x with a below and b above"
        );
    }

    #[test]
    fn multiscripts() {
        assert_eq!(
            say(
                "<mmultiscripts><mi>T</mi><mi>c</mi><none/><none/><mi>d</mi><mprescripts/><mi>a</mi><none/></mmultiscripts>"
            ),
            "capital T with presubscript a, subscript c, superscript d"
        );
    }

    #[test]
    fn fences() {
        assert_eq!(
            say("<mo>|</mo><mi>x</mi><mo>|</mo>"),
            "the absolute value of x"
        );
        assert_eq!(
            say("<mo>⌊</mo><mi>x</mi><mo>+</mo><mn>1</mn><mo>⌋</mo>"),
            "the floor of x plus 1"
        );
        assert_eq!(say("<mo>‖</mo><mi>v</mi><mo>‖</mo>"), "the norm of v");
        assert_eq!(
            say("<mo>(</mo><mi>a</mi><mo>+</mo><mi>b</mi><mo>)</mo><mi>c</mi>"),
            "open paren a plus b close paren c"
        );
        assert_eq!(
            say("<mfenced><mi>a</mi><mi>b</mi></mfenced>"),
            "open paren a comma b close paren"
        );
    }

    const TABLE: &str = "<mtable><mtr><mtd><mi>a</mi></mtd><mtd><mi>b</mi></mtd></mtr>\
        <mtr><mtd><mi>c</mi></mtd><mtd><mi>d</mi></mtd></mtr></mtable>";

    #[test]
    fn tables() {
        assert_eq!(
            say(TABLE),
            "the 2 by 2 table; row 1: a, b; row 2: c, d; end table"
        );
        assert_eq!(
            say(&format!("<mrow><mo>(</mo>{TABLE}<mo>)</mo></mrow>")),
            "the 2 by 2 matrix; row 1: a, b; row 2: c, d; end matrix"
        );
        assert_eq!(
            say(&format!("<mo>[</mo>{TABLE}<mo>]</mo>")),
            "the 2 by 2 matrix; row 1: a, b; row 2: c, d; end matrix"
        );
        assert_eq!(
            say(&format!("<mo>|</mo>{TABLE}<mo>|</mo>")),
            "the 2 by 2 determinant; row 1: a, b; row 2: c, d; end determinant"
        );
        assert_eq!(
            say(
                "<mtable><mlabeledtr><mtd><mtext>(1)</mtext></mtd><mtd><mi>x</mi></mtd></mlabeledtr></mtable>"
            ),
            "the 1 by 1 table; row 1, labeled (1): x; end table"
        );
        assert_eq!(
            say("<mtable><mtr><mtd></mtd><mtd><mn>1</mn></mtd></mtr></mtable>"),
            "the 1 by 2 table; row 1: blank, 1; end table"
        );
    }

    #[test]
    fn enclose() {
        assert_eq!(
            say("<menclose notation='box'><mi>x</mi></menclose>"),
            "x in a box"
        );
        assert_eq!(
            say("<menclose notation='updiagonalstrike'><mi>x</mi></menclose>"),
            "x crossed out"
        );
        assert_eq!(
            say("<menclose notation='bogus'><mi>x</mi></menclose>"),
            "x enclosed"
        );
    }

    #[test]
    fn text_and_strings() {
        assert_eq!(say("<mi>x</mi><mtext>if</mtext><mi>y</mi>"), "x if y");
        assert_eq!(say("<ms>abc</ms>"), "the string abc");
        assert_eq!(say("<merror><mi>x</mi></merror>"), "error: x");
    }

    #[test]
    fn silent_elements() {
        assert_eq!(
            say("<mi>a</mi><mspace width='1em'/><mphantom><mi>z</mi></mphantom><mi>b</mi>"),
            "a b"
        );
        assert_eq!(say("<mspace width='1em'/>"), "space");
        assert_eq!(say("<mrow/>"), "blank");
        assert_eq!(say("<mo>&#x2062;</mo>"), "invisible times");
        assert_eq!(
            say("<semantics><mi>x</mi><annotation>x</annotation></semantics>"),
            "x"
        );
    }

    /// Every node of every test file speaks, on both paths.
    #[test]
    fn test_files_have_no_empty_speech() {
        let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/testfiles");
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|e| e != "xml") {
                continue;
            }
            let doc =
                mathmlparser::parse(std::fs::read_to_string(&path).unwrap().as_bytes()).unwrap();
            TEST_FONT.with(|font| {
                let styled = doc.style(font, StyleOptions::default());
                for node in all_nodes(&doc) {
                    assert_well_formed(&speak_document(&doc, node), &(&path, node));
                    assert_well_formed(&speak(&styled, node), &(&path, node));
                }
            });
        }
    }
}

// ----- property test -----

const ELEMENTS: &[&str] = &[
    "math",
    "mrow",
    "mfrac",
    "msqrt",
    "mroot",
    "msub",
    "msup",
    "msubsup",
    "munder",
    "mover",
    "munderover",
    "mmultiscripts",
    "mprescripts",
    "none",
    "mspace",
    "mpadded",
    "menclose",
    "mphantom",
    "maligngroup",
    "malignmark",
    "mstyle",
    "merror",
    "semantics",
    "mtable",
    "mtr",
    "mlabeledtr",
    "mtd",
    "unknownelement",
];
const TOKENS: &[&str] = &["mi", "mn", "mo", "mtext", "ms"];
const TEXTS: &[&str] = &[
    "", " ", "x", "X", "2", "sin", "lim", "∑", "∫", "-", "−", "(", ")", "|", "⌊", "⌋", "→",
    "\u{2061}", "\u{2062}", "\u{2063}", "\u{2064}", "𝑥", "𝐀", "ℝ", "α", "Γ", "^", "′", "abc", "☃",
    "\u{200B}", "a b",
];
const ATTRIBUTES: &[(&str, &str)] = &[
    ("notation", "box"),
    ("notation", ""),
    ("notation", "nonsense"),
    ("mathvariant", "bold"),
    ("mathvariant", "double-struck"),
    ("accent", "true"),
];

/// A random MathML tree: any element, any number of children (including the wrong number for
/// the element), any text.
#[derive(Debug, Clone)]
enum Tree {
    Token(
        &'static str,
        &'static str,
        Option<(&'static str, &'static str)>,
    ),
    Element(
        &'static str,
        Option<(&'static str, &'static str)>,
        Vec<Tree>,
    ),
}

impl Arbitrary for Tree {
    fn arbitrary(g: &mut Gen) -> Tree {
        fn build(g: &mut Gen, depth: usize) -> Tree {
            let attribute = if bool::arbitrary(g) {
                Some(*g.choose(ATTRIBUTES).unwrap())
            } else {
                None
            };
            if depth == 0 || u8::arbitrary(g) % 3 == 0 {
                return Tree::Token(
                    g.choose(TOKENS).unwrap(),
                    g.choose(TEXTS).unwrap(),
                    attribute,
                );
            }
            let count = usize::arbitrary(g) % 5;
            let children = (0..count).map(|_| build(g, depth - 1)).collect();
            Tree::Element(g.choose(ELEMENTS).unwrap(), attribute, children)
        }
        build(g, 4)
    }

    fn shrink(&self) -> Box<dyn Iterator<Item = Tree>> {
        match self {
            Tree::Token(..) => Box::new(std::iter::empty()),
            Tree::Element(name, attribute, children) => {
                let (name, attribute) = (*name, *attribute);
                let subtrees = children.clone().into_iter();
                let fewer = children
                    .clone()
                    .shrink()
                    .map(move |c| Tree::Element(name, attribute, c));
                Box::new(subtrees.chain(fewer))
            }
        }
    }
}

impl Tree {
    fn add_to(&self, doc: &mut MathDocument) -> NodeId {
        let attributes = |a: &Option<(&str, &str)>| {
            a.iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect()
        };
        match self {
            Tree::Token(name, text, attribute) => doc.add_token(name, attributes(attribute), text),
            Tree::Element(name, attribute, children) => {
                let children = children.iter().map(|c| c.add_to(doc)).collect();
                doc.add_element(name, attributes(attribute), children)
            }
        }
    }
}

#[test]
fn no_element_speaks_an_empty_string() {
    fn property(tree: Tree) -> bool {
        let mut doc = MathDocument::new();
        let root = tree.add_to(&mut doc);
        doc.set_root(root);
        TEST_FONT.with(|font| {
            let styled = doc.style(font, StyleOptions::default());
            for node in all_nodes(&doc) {
                let context = (&tree, node, doc.node(node).element);
                assert_well_formed(&speak_document(&doc, node), &context);
                assert_well_formed(&speak(&styled, node), &context);
            }
        });
        true
    }
    QuickCheck::new()
        .tests(2000)
        .quickcheck(property as fn(Tree) -> bool);
}

#[test]
fn every_element_kind_speaks_on_its_own() {
    for name in ELEMENTS.iter().chain(TOKENS) {
        let mut doc = MathDocument::new();
        let node = if TOKENS.contains(name) {
            doc.add_token(name, vec![], "")
        } else {
            doc.add_element(name, vec![], vec![])
        };
        let element = doc.node(node).element;
        assert_well_formed(&speak_document(&doc, node), &(name, element));
    }
}

#[cfg(all(feature = "svg", feature = "mathml_parser"))]
mod svg {
    use super::*;
    use formel::host::MathFont;
    use formel::mathmlparser;
    use formel::svg_renderer::{self, A11y, Flags};

    fn render(xml: &str, a11y: A11y<'_>) -> String {
        let doc = mathmlparser::parse(xml.as_bytes()).expect("valid MathML");
        TEST_FONT.with(|font| {
            let units_per_em = font.units_per_em() as f32;
            let options = StyleOptions {
                font_size: units_per_em,
                ..Default::default()
            };
            let result = formel::layout(&doc, font, options);
            svg_renderer::render_to_string_with(
                &doc,
                &result.layouts,
                font.face(),
                units_per_em,
                Flags::default(),
                a11y,
            )
        })
    }

    /// The `<title>` must be the first child of `<svg>` for assistive technology.
    fn title(svg: &str) -> Option<&str> {
        let open = svg.find('>')? + 1;
        let rest = svg[open..].trim_start().strip_prefix("<title>")?;
        Some(&rest[..rest.find("</title>")?])
    }

    #[test]
    fn spoken_by_default() {
        let svg = render("<msqrt><mi>x</mi></msqrt>", A11y::default());
        assert!(svg.contains(r#"role="img""#), "{svg}");
        assert_eq!(title(&svg), Some("the square root of x"));
    }

    #[test]
    fn author_alttext_wins_over_speech() {
        let svg = render(
            r#"<math alttext=" root of  x "><msqrt><mi>x</mi></msqrt></math>"#,
            A11y::SPOKEN,
        );
        assert_eq!(title(&svg), Some("root of x"));
        let svg = render(r#"<math alttext=" "><mi>x</mi></math>"#, A11y::SPOKEN);
        assert_eq!(title(&svg), Some("x"));
    }

    #[test]
    fn explicit_title_and_none() {
        let svg = render("<mi>x</mi>", A11y::titled("ex"));
        assert_eq!(title(&svg), Some("ex"));
        let svg = render("<mi>x</mi>", A11y::NONE);
        assert!(!svg.contains("<title>") && !svg.contains("role="), "{svg}");
    }

    #[test]
    fn plain_render_is_named() {
        let doc = mathmlparser::parse("<mfrac><mi>a</mi><mi>b</mi></mfrac>".as_bytes()).unwrap();
        let svg = TEST_FONT.with(|font| {
            let result = formel::layout(&doc, font, StyleOptions::default());
            svg_renderer::render_to_string(
                &doc,
                &result.layouts,
                font.face(),
                font.units_per_em() as f32,
                Flags::default(),
            )
        });
        assert_eq!(title(&svg), Some("a over b"));
    }

    #[test]
    fn title_is_escaped() {
        let svg = render("<mtext>a &lt; b &amp; c</mtext>", A11y::SPOKEN);
        assert_eq!(title(&svg), Some("a &lt; b &amp; c"));
    }
}
