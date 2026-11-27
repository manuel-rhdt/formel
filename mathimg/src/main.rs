


use std::borrow::Cow;
use std::fs::File;
use std::io;
use std::io::BufReader;
use std::path::{Path, PathBuf};



use formel::mathmlparser;
use formel::svg_renderer;
use formel::host::MathFont;
use formel::shaper::Shaper;
use formel::standalone::StyleOptions;

use docopt::Docopt;

const USAGE: &'static str = "
Usage: mathimg [options] --font=FONT <input> <output>

Renders MathML to an image. <input> is a MathML file, or - for standard input; <output> is a file
or a directory (the file is then named after the input).

Options:
    -o FORMAT --output-format=FORMAT  The output format to use. [default: svg]
    -f FONT --font=FONT               Path of a font file with an OpenType MATH table.
    --face-index=N                    Index of the font within a font collection (.ttc/.otc),
                                      for example Cambria Math in cambria.ttc. [default: 0]
    --show-ink-bounds                 Fill the boxes of token elements.
    --show-logical-bounds             Outline the box of every element.
    ";

#[derive(Debug, serde::Deserialize)]
struct Args {
    arg_input: String,
    arg_output: String,
    flag_output_format: Option<Format>,
    flag_font: String,
    flag_face_index: u32,
    flag_show_ink_bounds: bool,
    flag_show_logical_bounds: bool,
}

#[derive(serde::Deserialize, Debug, Copy, Clone)]
enum Format {
    Svg,
}

impl Format {
    fn extension(self) -> &'static str {
        match self {
            Format::Svg => ".svg",
        }
    }
}

fn main() {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());

    let (list, output_name) = if args.arg_input == "-" {
        let stdin = io::stdin();
        let handle = stdin.lock();
        (Some(mathmlparser::parse(handle).unwrap()), "output".into())
    } else if args.arg_input != "" {
        let path = match PathBuf::from(args.arg_input.clone()).canonicalize() {
            Ok(path) => path,
            Err(err) => {
                println!("Error opening {:?}", args.arg_input);
                panic!("{}", err);
            }
        };
        let file = File::open(&path).unwrap();
        let name = path
            .file_stem()
            .or_else(|| path.file_name())
            .expect("input file has no name");
        (
            Some(mathmlparser::parse(BufReader::new(file)).expect("could not parse file")),
            Cow::from(name.to_string_lossy().into_owned()),
        )
    } else {
        (None, "".into())
    };

    let font_path = match PathBuf::from(args.flag_font.clone()).canonicalize() {
        Ok(path) => path,
        Err(err) => {
            eprintln!("Error opening {:?}: {err}", args.flag_font);
            std::process::exit(1);
        }
    };

    let mut out_path = Cow::from(Path::new(&args.arg_output));
    if out_path.is_dir() {
        let extension = args
            .flag_output_format
            .map(|format| format.extension())
            .unwrap_or("");
        out_path.to_mut().push(output_name.into_owned() + extension);
    }

    let font_bytes = match std::fs::read(&font_path) {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!("could not read {:?}: {err}", font_path);
            std::process::exit(1);
        }
    };

    // The face index matters for collections: Shaper::from_slice would otherwise read the first font.
    let Some(shaper) = Shaper::from_vec(font_bytes, args.flag_face_index) else {
        eprintln!(
            "{:?} (face {}) is not a font with an OpenType MATH table",
            font_path, args.flag_face_index
        );
        std::process::exit(1);
    };

    let document = list.as_ref().unwrap();
    // A font size of one em per font unit makes px equal font units.
    let units_per_em = shaper.units_per_em() as f32;
    let options = StyleOptions {
        font_size: units_per_em,
        ..Default::default()
    };
    let typeset = formel::layout(document, &shaper, options);
    match args.flag_output_format {
        Some(Format::Svg) => {
            let flags = svg_renderer::Flags {
                show_ink_bounds: args.flag_show_ink_bounds,
                show_logical_bounds: args.flag_show_logical_bounds,
                margin: svg_renderer::DEFAULT_MARGIN_EM * units_per_em,
                font_size: Some(units_per_em),
            };

            svg_renderer::render(
                document,
                &typeset.layouts,
                &shaper,
                units_per_em,
                flags,
                &out_path,
            )
        }
        _ => panic!(),
    }
}
