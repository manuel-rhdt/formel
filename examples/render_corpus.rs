//! Renders every MathML file of a directory to SVG with a given font, for before/after diffs of
//! the font backend.
//!
//! ```sh
//! cargo run --release --example render_corpus --features mathml_parser,svg -- \
//!     tests/testfiles web/fonts/stixtwo-math.otf out/stix
//! ```

use formel::host::MathFont;
use formel::mathmlparser;
use formel::shaper::Shaper;
use formel::standalone::StyleOptions;
use formel::svg_renderer::{self, Flags};

fn main() {
    let mut args = std::env::args().skip(1);
    let (Some(dir), Some(font_path), Some(out)) = (args.next(), args.next(), args.next()) else {
        eprintln!("usage: render_corpus <dir with .xml> <font> <out dir>");
        std::process::exit(2);
    };
    let data = std::fs::read(&font_path).expect("could not read font");
    let font = Shaper::from_slice(&data, 0).expect("font has a MATH table");
    std::fs::create_dir_all(&out).unwrap();
    let units_per_em = font.units_per_em() as f32;

    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "xml"))
        .collect();
    files.sort();
    for path in files {
        let file = std::fs::File::open(&path).unwrap();
        let Ok(document) = mathmlparser::parse(std::io::BufReader::new(file)) else {
            eprintln!("skipping {}: invalid MathML", path.display());
            continue;
        };
        if document.root().is_none() {
            continue;
        }
        let options = StyleOptions {
            font_size: units_per_em,
            ..Default::default()
        };
        let typeset = formel::layout(&document, &font, options);
        let flags = Flags {
            margin: svg_renderer::DEFAULT_MARGIN_EM * units_per_em,
            font_size: Some(units_per_em),
            ..Default::default()
        };
        let svg =
            svg_renderer::render_to_string(&document, &typeset.layouts, &font, units_per_em, flags);
        let name = path.file_stem().unwrap().to_string_lossy().into_owned();
        std::fs::write(format!("{out}/{name}.svg"), svg).unwrap();
    }
}
