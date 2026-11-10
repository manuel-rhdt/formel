


use std::borrow::Cow;
use std::fs::File;
use std::io;
use std::io::BufReader;
use std::path::{Path, PathBuf};



use math_render::mathmlparser;
use math_render::svg_renderer;
use math_render::host::MathFont;
use math_render::shaper::Shaper;
use math_render::standalone::StyleOptions;

use fontconfig::{list_fonts, Pattern};


use docopt::Docopt;

const USAGE: &'static str = "
Usage: mathimg [options] <input> <output>
       mathimg list-fonts [--verbose]

Subcommands:
    list-fonts  Lists all available math fonts on the system.

Options:
    -o FORMAT --output-format=FORMAT  The output format to use. [default: svg]
    -f FONT --font=FONT               Name of the font to use.
    --show-ink-bounds                 Fill the boxes of token elements.
    --show-logical-bounds             Outline the box of every element.
    --verbose                         Show additional information
    ";

#[derive(Debug, serde::Deserialize)]
struct Args {
    arg_input: String,
    arg_output: String,
    flag_output_format: Option<Format>,
    cmd_list_fonts: bool,
    flag_font: String,
    flag_verbose: bool,
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

#[derive(Debug)]
struct Font {
    name: String,
    path: PathBuf,
    face_index: u32,
}

fn find_math_fonts() -> Vec<Font> {
    let pat = Pattern::new();
    let fontset = list_fonts(&pat);

    (&fontset)
        .iter()
        .filter_map(|pattern| {
            pattern.get_string("capability").and_then(|cap| {
                if cap.contains("otlayout:math") {
                    Some(Font {
                        name: pattern.name().unwrap().into(),
                        path: pattern.filename().unwrap().into(),
                        face_index: pattern.face_index().unwrap() as u32,
                    })
                } else {
                    None
                }
            })
        })
        .filter(has_math_data)
        .collect()
}

/// checks if a math table exists in the font
fn has_math_data(font: &Font) -> bool {
    let Ok(buffer) = std::fs::read(&font.path) else {
        return false;
    };
    Shaper::from_slice(&buffer, font.face_index as u32).is_some()
}

fn create_shaper(font_bytes: &[u8]) -> Shaper<'_> {
    Shaper::from_slice(font_bytes, 0).expect("font must have a MATH table")
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

    if args.cmd_list_fonts {
        let vec = find_math_fonts();
        if vec.len() == 0 {
            panic!("Found no math fonts.");
        }

        for font in &vec {
            print!("{}", font.name);
            if args.flag_verbose {
                print!(": {:?}", font.path);
            }
            print!("\n");
        }
        return;
    }

    let font_path = if args.flag_font.is_empty() {
        PathBuf::from(
            find_math_fonts()
                .get(0)
                .expect("Could not find suitable math font on system.")
                .path
                .clone(),
        )
    } else {
        match PathBuf::from(args.flag_font.clone()).canonicalize() {
            Ok(path) => path,
            Err(err) => {
                println!("Error opening {:?}", args.flag_font);
                panic!("{}", err);
            }
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

    let font_bytes = std::fs::read(&font_path).expect("could not read font file");

    let shaper = create_shaper(&font_bytes);

    let document = list.as_ref().unwrap();
    // A font size of one em per font unit makes px equal font units.
    let units_per_em = shaper.units_per_em() as f32;
    let options = StyleOptions {
        font_size: units_per_em,
        ..Default::default()
    };
    let typeset = math_render::layout(document, &shaper, options);
    match args.flag_output_format {
        Some(Format::Svg) => {
            let flags = svg_renderer::Flags {
                show_ink_bounds: args.flag_show_ink_bounds,
                show_logical_bounds: args.flag_show_logical_bounds,
            };

            svg_renderer::render(
                document,
                &typeset.layouts,
                shaper.face(),
                units_per_em,
                flags,
                &out_path,
            )
        }
        _ => panic!(),
    }
}
