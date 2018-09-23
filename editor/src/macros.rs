// Call the log crate, but pre-set the target.

macro_rules! debug {
    ( $( $x:expr ),* ) => {
        {
            extern crate log;
            log!(target: "UI", log::Level::Debug, $( $x, )* );
        }
    }
}

macro_rules! info {
    ( $( $x:expr ),* ) => {
        {
            extern crate log;
            log!(target: "UI", log::Level::Info, $( $x, )* );
        }
    }
}

macro_rules! warn {
    ( $( $x:expr ),* ) => {
        {
            extern crate log;
            log!(target: "UI", log::Level::Warn, $( $x, )* );
        }
    }
}

macro_rules! error {
    ( $( $x:expr ),* ) => {
        {
            extern crate log;
            log!(target: "UI", log::Level::Error, $( $x, )* );
        }
    }
}
