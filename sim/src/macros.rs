// Call the log crate, but pre-set the target.

macro_rules! debug {
    ( $( $x:expr ),* ) => {
        {
            use log::log;
            log!(target: "sim", log::Level::Debug, $( $x, )* );
        }
    }
}

macro_rules! info {
    ( $( $x:expr ),* ) => {
        {
            use log::log;
            log!(target: "sim", log::Level::Info, $( $x, )* );
        }
    }
}

macro_rules! warn {
    ( $( $x:expr ),* ) => {
        {
            use log::log;
            log!(target: "sim", log::Level::Warn, $( $x, )* );
        }
    }
}

macro_rules! error {
    ( $( $x:expr ),* ) => {
        {
            use log::log;
            log!(target: "sim", log::Level::Error, $( $x, )* );
        }
    }
}
