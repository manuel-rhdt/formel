use math_render::shaper::Shaper;

pub fn get_bytes() -> &'static [u8] {
    include_bytes!("testfiles/latinmodern-math.otf")
}

thread_local! {
    pub static TEST_FONT: Shaper<'static> =
        Shaper::from_slice(get_bytes(), 0).expect("test font has a MATH table");
}
