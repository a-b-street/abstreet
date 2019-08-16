use std::{error, fmt};

#[derive(PartialEq)]
pub struct Error {
    message: String,
    context: Vec<String>,
}

impl Error {
    pub fn new(message: String) -> Error {
        Error {
            message,
            context: Vec::new(),
        }
    }

    pub fn context(mut self, msg: String) -> Error {
        self.context.push(msg);
        self
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.message)?;
        for c in &self.context {
            writeln!(f, "\n  - {}", c)?;
        }
        Ok(())
    }
}

impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // Do the same thing as the Display trait
        write!(f, "{}", self)
    }
}

impl error::Error for Error {}

impl std::convert::From<std::num::ParseFloatError> for Error {
    fn from(err: std::num::ParseFloatError) -> Self {
        Error::new(err.to_string())
    }
}
