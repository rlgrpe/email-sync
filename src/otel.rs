//! Internal helpers for OpenTelemetry span status reporting.
//!
//! Active only with the `observability` feature.

use crate::error::Error;

/// Sets the current span status based on the result.
///
/// - `Ok` -> `Status::Ok`
/// - `Err` -> `Status::error(...)` with the error message
pub(crate) fn set_span_status(result: &Result<impl Sized, Error>) {
    #[cfg(feature = "observability")]
    {
        use opentelemetry::trace::Status;
        use tracing_opentelemetry::OpenTelemetrySpanExt;

        match result {
            Ok(_) => tracing::Span::current().set_status(Status::Ok),
            Err(e) => tracing::Span::current().set_status(Status::error(e.to_string())),
        }
    }

    #[cfg(not(feature = "observability"))]
    let _ = result;
}
