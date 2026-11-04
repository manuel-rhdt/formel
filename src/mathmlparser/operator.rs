use bitflags::bitflags;

bitflags! {
    /// Boolean operator properties, as stored in the operator dictionary.
    pub struct Flags: u8 {
        const SYMMETRIC         = 0b00000001;
        const FENCE             = 0b00000010;
        const STRETCHY          = 0b00000100;
        const SEPARATOR         = 0b00001000;
        const ACCENT            = 0b00010000;
        const LARGEOP           = 0b00100000;
        const MOVABLE_LIMITS    = 0b01000000;
    }
}

impl Default for Flags {
    fn default() -> Flags {
        Flags::empty()
    }
}

/// The form of an operator: its position relative to its operands.
#[derive(PartialEq, Eq, Copy, Clone, Debug, Ord, PartialOrd, Hash, Default)]
pub enum Form {
    Prefix,
    #[default]
    Infix,
    Postfix,
}

impl std::str::FromStr for Form {
    type Err = String;
    fn from_str(s: &str) -> Result<Form, String> {
        match s.trim() {
            "prefix" => Ok(Form::Prefix),
            "infix" => Ok(Form::Infix),
            "postfix" => Ok(Form::Postfix),
            other => Err(format!("unknown operator form {:?}", other)),
        }
    }
}
