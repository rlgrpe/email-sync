//! Internal helpers for OpenTelemetry span status reporting.
//!
//! Active only with the `tracing` feature.

/// Marks the current span as successful.
pub(crate) fn set_span_ok() {
    #[cfg(feature = "tracing")]
    {
        use opentelemetry::trace::Status;
        use tracing_opentelemetry::OpenTelemetrySpanExt;

        tracing::Span::current().set_status(Status::Ok);
    }

    #[cfg(not(feature = "tracing"))]
    {}
}

/// Marks the current span as failed.
pub(crate) fn set_span_error(err: &dyn std::fmt::Display) {
    #[cfg(feature = "tracing")]
    {
        use opentelemetry::trace::Status;
        use tracing_opentelemetry::OpenTelemetrySpanExt;

        tracing::Span::current().set_status(Status::error(err.to_string()));
    }

    #[cfg(not(feature = "tracing"))]
    let _ = err;
}

/// Marks the current span as a client span for `OTel` backends.
pub(crate) fn set_client_kind() {
    #[cfg(feature = "tracing")]
    {
        use tracing_opentelemetry::OpenTelemetrySpanExt;

        tracing::Span::current().set_attribute("otel.kind", "client");
    }

    #[cfg(not(feature = "tracing"))]
    {}
}
