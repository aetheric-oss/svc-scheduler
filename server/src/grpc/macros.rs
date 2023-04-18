//! log macro's for gRPC logging

/// Writes a debug! message to the app::grpc logger
#[macro_export]
macro_rules! grpc_debug {
    ($($arg:tt)+) => {
        log::debug!(target: "app::grpc", $($arg)+)
    };
}

/// Writes an info! message to the app::grpc logger
#[macro_export]
macro_rules! grpc_info {
    ($($arg:tt)+) => {
        log::info!(target: "app::grpc", $($arg)+)
    };
}

/// Writes an warn! message to the app::grpc logger
#[macro_export]
macro_rules! grpc_warn {
    ($($arg:tt)+) => {
        log::warn!(target: "app::grpc", $($arg)+)
    };
}

/// Writes an error! message to the app::grpc logger
#[macro_export]
macro_rules! grpc_error {
    ($($arg:tt)+) => {
        log::error!(target: "app::grpc", $($arg)+)
    };
}
