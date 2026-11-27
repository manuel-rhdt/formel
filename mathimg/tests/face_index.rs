//! A math font inside a collection must be read from the face the caller asked for. (The old
//! `create_shaper` always used face 0, so Cambria Math in `cambria.ttc` loaded the wrong font.)

use std::process::Command;

const FONT: &[u8] = include_bytes!("../../tests/testfiles/latinmodern-math.otf");

/// A two-face collection: face 0 is Latin Modern Math with its MATH table renamed, so that it is not
/// a math font; face 1 is the intact font.
fn collection() -> Vec<u8> {
    let num_tables = u16::from_be_bytes([FONT[4], FONT[5]]) as usize;
    let directory_len = 12 + 16 * num_tables;
    // 'ttcf', version 1.0, two faces, their offsets
    let header_len = 12 + 4 * 2;
    let bases = [header_len, header_len + FONT.len()];

    let mut out = Vec::new();
    out.extend_from_slice(b"ttcf");
    out.extend_from_slice(&0x0001_0000u32.to_be_bytes());
    out.extend_from_slice(&2u32.to_be_bytes());
    for base in bases {
        out.extend_from_slice(&(base as u32).to_be_bytes());
    }
    for (face, base) in bases.into_iter().enumerate() {
        let mut font = FONT.to_vec();
        for i in 0..num_tables {
            let record = 12 + 16 * i;
            // table offsets in a collection are from the start of the file
            let offset = u32::from_be_bytes(font[record + 8..record + 12].try_into().unwrap());
            font[record + 8..record + 12].copy_from_slice(&(offset + base as u32).to_be_bytes());
            if face == 0 && &font[record..record + 4] == b"MATH" {
                font[record..record + 4].copy_from_slice(b"MATX");
            }
        }
        assert!(font.len() > directory_len);
        out.extend_from_slice(&font);
    }
    out
}

#[test]
fn shaper_reads_the_requested_face() {
    let data = collection();
    assert!(
        formel::shaper::Shaper::from_slice(&data, 0).is_none(),
        "face 0 has no MATH table"
    );
    let shaper = formel::shaper::Shaper::from_slice(&data, 1).expect("face 1 is a math font");
    assert_eq!(shaper.face_index(), 1);
    assert_eq!(shaper.font_data(), &data[..]);
}

#[test]
fn mathimg_renders_from_a_nonzero_face() {
    let dir = std::path::Path::new(env!("CARGO_TARGET_TMPDIR"));
    let font = dir.join("two-faces.ttc");
    std::fs::write(&font, collection()).unwrap();
    let input = dir.join("input.xml");
    std::fs::write(&input, "<math><mfrac><mi>x</mi><mn>2</mn></mfrac></math>").unwrap();
    let mathimg = env!("CARGO_BIN_EXE_mathimg");

    // face 0 is not a math font: an error, not a rendering with the wrong face
    let output = dir.join("face0.svg");
    let status = Command::new(mathimg)
        .args(["-f", font.to_str().unwrap(), "--face-index=0"])
        .args([input.to_str().unwrap(), output.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(!status.status.success());
    assert!(String::from_utf8_lossy(&status.stderr).contains("MATH"));

    let output = dir.join("face1.svg");
    let status = Command::new(mathimg)
        .args(["-f", font.to_str().unwrap(), "--face-index=1"])
        .args([input.to_str().unwrap(), output.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(status.success());
    let svg = std::fs::read_to_string(&output).unwrap();
    assert!(svg.starts_with("<svg") && svg.contains("<path"), "{svg}");

    // the same formula rendered from the plain font is identical
    let plain = dir.join("plain.svg");
    let font = dir.join("plain.otf");
    std::fs::write(&font, FONT).unwrap();
    assert!(
        Command::new(mathimg)
            .args(["-f", font.to_str().unwrap()])
            .args([input.to_str().unwrap(), plain.to_str().unwrap()])
            .status()
            .unwrap()
            .success()
    );
    assert_eq!(svg, std::fs::read_to_string(&plain).unwrap());
}
