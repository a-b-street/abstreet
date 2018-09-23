// Call the log crate, but pre-set the target.

macro_rules! debug {
    ( $( $x:expr ),* ) => {
        {
            extern crate log;
            log!(target: "map", log::Level::Debug, $( $x, )* );
        }
    }
}

macro_rules! info {
    ( $( $x:expr ),* ) => {
        {
            extern crate log;
            log!(target: "map", log::Level::Info, $( $x, )* );
        }
    }
}

macro_rules! warn {
    ( $( $x:expr ),* ) => {
        {
            extern crate log;
            log!(target: "map", log::Level::Warn, $( $x, )* );
        }
    }
}

macro_rules! error {
    ( $( $x:expr ),* ) => {
        {
            extern crate log;
            log!(target: "map", log::Level::Error, $( $x, )* );
        }
    }
}
