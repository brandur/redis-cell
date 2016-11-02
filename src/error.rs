use std::error;
use std::fmt;
use std::string;

#[derive(Debug)]
pub enum ThrottleError {
    Generic(GenericError),
    String(string::FromUtf8Error),
}

impl ThrottleError {
    pub fn generic(message: &str) -> ThrottleError {
        ThrottleError::Generic(GenericError::new(message))
    }
}

#[derive(Debug)]
pub struct GenericError {
    message: String,
}

impl GenericError {
    pub fn new(message: &str) -> GenericError {
        GenericError { message: String::from(message) }
    }
}

impl<'a> fmt::Display for GenericError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Store error: {}", self.message)
    }
}

impl<'a> error::Error for GenericError {
    fn description(&self) -> &str {
        self.message.as_str()
    }

    fn cause(&self) -> Option<&error::Error> {
        None
    }
}
