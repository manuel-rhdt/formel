use std::fmt;

pub type Result<T> = std::result::Result<T, ParsingError>;

#[derive(Debug)]
pub struct ParsingError {
    /// Byte offset into the input, if known.
    pub position: Option<u64>,
    pub error_type: ErrorType,
}

impl ParsingError {
    /// An error at a byte offset into the input.
    pub fn at(position: u64, error_type: ErrorType) -> ParsingError {
        ParsingError {
            position: Some(position),
            error_type,
        }
    }
}

#[derive(Debug)]
pub enum ErrorType {
    UnknownElement(String),
    UnexpectedEndOfInput,
    WrongEndElement(String),
    OtherError(String),
    Utf8Error(std::str::Utf8Error),
    #[cfg(feature = "mathml_parser")]
    XmlError(quick_xml::Error),
}

impl fmt::Display for ParsingError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.error_type {
            ErrorType::Utf8Error(err) => write!(f, "{}", err),
            ErrorType::UnknownElement(ref name) => write!(f, "Unknown Element: \"{}\"", name),
            ErrorType::UnexpectedEndOfInput => write!(f, "Unexpected end of input."),
            ErrorType::WrongEndElement(ref name) => write!(
                f,
                "Unexpected end element \"<{}>\" without corresponding start element.",
                name
            ),
            ErrorType::OtherError(ref string) => write!(f, "Error: {}", string),
            #[cfg(feature = "mathml_parser")]
            ErrorType::XmlError(ref error) => write!(f, "XML error: {}", error),
        }
    }
}
impl std::error::Error for ParsingError {
    fn cause(&self) -> Option<&dyn std::error::Error> {
        match self.error_type {
            #[cfg(feature = "mathml_parser")]
            ErrorType::XmlError(ref error) => Some(error),
            _ => None,
        }
    }
}
impl<'a> ::std::convert::From<&'a str> for ParsingError {
    fn from(string: &str) -> ParsingError {
        ParsingError {
            position: None,
            error_type: ErrorType::OtherError(string.to_owned()),
        }
    }
}
impl ::std::convert::From<String> for ParsingError {
    fn from(string: String) -> ParsingError {
        ParsingError {
            position: None,
            error_type: ErrorType::OtherError(string),
        }
    }
}
#[cfg(feature = "mathml_parser")]
impl ::std::convert::From<quick_xml::Error> for ParsingError {
    fn from(error: quick_xml::Error) -> ParsingError {
        ParsingError {
            position: None,
            error_type: ErrorType::XmlError(error),
        }
    }
}
impl ::std::convert::From<std::str::Utf8Error> for ParsingError {
    fn from(error: std::str::Utf8Error) -> ParsingError {
        ParsingError {
            position: None,
            error_type: ErrorType::Utf8Error(error),
        }
    }
}
