//! Renders a MathML file to SVG with the bundled Latin Modern Math font.
//!
//! ```sh
//! cargo run --example render_svg --features mathml_parser,svg -- tests/testfiles/frac.xml out.svg
//! ```

use math_render::host::MathFont;
use math_render::mathmlparser;
use math_render::shaper::Shaper;
use math_render::standalone::StyleOptions;
use math_render::svg_renderer::{self, Flags};

const FONT: &[u8] = include_bytes!("../tests/testfiles/latinmodern-math.otf");

fn main() {
    let mut args = std::env::args().skip(1);
    let (Some(input), Some(output)) = (args.next(), args.next()) else {
        eprintln!("usage: render_svg <input.xml> <output.svg>");
        std::process::exit(2);
    };

    let file = std::fs::File::open(&input).expect("could not open input");
    let document = mathmlparser::parse(std::io::BufReader::new(file)).expect("invalid MathML");

    let font = Shaper::from_slice(FONT, 0).expect("font has a MATH table");
    // A font size of one em per font unit makes px equal font units.
    let units_per_em = font.units_per_em() as f32;
    let options = StyleOptions {
        font_size: units_per_em,
        ..Default::default()
    };
    let typeset = math_render::layout(&document, &font, options);

    svg_renderer::render(
        &document,
        &typeset.layouts,
        font.face(),
        units_per_em,
        Flags::default(),
        output,
    );
}
