use std::error::Error;
use std::fmt;

use http::StatusCode;

use super::super::Context;
use crate::schema::Value;

pub type HandlerError = Box<dyn Error + Send + Sync>;

/// Wraps any error as a `HandlerError` carrying the HTTP status a REST
/// response (or the initial SSE/Live handshake) should use instead of the
/// default 500.
pub fn with_status(status: StatusCode, source: impl Into<HandlerError>) -> HandlerError {
    Box::new(Status {
        status,
        source: source.into(),
    })
}

pub fn status_of(error: &HandlerError) -> Option<StatusCode> {
    error.downcast_ref::<Status>().map(|s| s.status)
}

struct Status {
    status: StatusCode,
    source: HandlerError,
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.source, f)
    }
}

impl fmt::Debug for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.source, f)
    }
}

impl Error for Status {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(self.source.as_ref())
    }
}

pub trait Handler: Send + Sync {
    fn call(&self, ctx: &Context, args: &[Value]) -> Result<Value, HandlerError>;
}

impl<F> Handler for F
where
    F: Fn(&Context, &[Value]) -> Result<Value, HandlerError> + Send + Sync,
{
    fn call(&self, ctx: &Context, args: &[Value]) -> Result<Value, HandlerError> {
        self(ctx, args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_status_is_recovered_by_status_of() {
        let error = with_status(StatusCode::NOT_FOUND, "missing");
        assert_eq!(status_of(&error), Some(StatusCode::NOT_FOUND));
        assert_eq!(error.to_string(), "missing");
    }

    #[test]
    fn plain_errors_have_no_status() {
        let error: HandlerError = "boom".into();
        assert_eq!(status_of(&error), None);
    }
}
