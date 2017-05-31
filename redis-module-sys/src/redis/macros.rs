#[macro_export]
macro_rules! redis_error {
    ($message:expr) => {
        CellError::generic($message)
    };
    ($message:expr, $($arg:tt)*) => {
        CellError::generic(format!($message, $($arg)+).as_str())
    }
}

#[macro_export]
macro_rules! redis_log_debug {
    ($logger:expr, $target:expr) => {
        if cfg!(debug_assertions) {
            $logger.log_debug($target)
        }
    };
    ($logger:expr, $target:expr, $($arg:tt)*) => {
        if cfg!(debug_assertions) {
            $logger.log_debug(format!($target, $($arg)+).as_str())
        }
    }
}