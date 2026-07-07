/* per-request context delivered to every handler alongside the decoded
args: what the request carried besides its parameters. today that is the
bearer token; handler parameters resolved from it (like an authenticated
User) implement FromContext instead of FromValue */

use std::collections::HashMap;

use super::binding::HandlerError;

#[derive(Clone, Debug, Default)]
pub struct Context {
    token: Option<String>,
}

impl Context {
    pub fn with_token(token: impl Into<String>) -> Context {
        Context {
            token: Some(token.into()),
        }
    }

    /// The request's bearer token, if any.
    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    /// Reads `Authorization: Bearer <token>`, falling back to a `?token=`
    /// query parameter — browser EventSource/WebSocket cannot set headers.
    pub(crate) fn extract(
        headers: &http::HeaderMap,
        query: &HashMap<String, String>,
    ) -> Context {
        let bearer = headers
            .get(http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .map(str::to_string);
        Context {
            token: bearer.or_else(|| query.get("token").cloned()),
        }
    }
}

/// Handler parameters that come from the request context rather than the
/// wire. The endpoint macros route any `User`/`Option<User>` parameter here;
/// such parameters never appear in the endpoint's schema.
pub trait FromContext: Sized {
    fn from_context(ctx: &Context) -> Result<Self, HandlerError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn headers(pairs: &[(&str, &str)]) -> http::HeaderMap {
        let mut map = http::HeaderMap::new();
        for (name, value) in pairs {
            map.insert(
                http::HeaderName::try_from(*name).unwrap(),
                http::HeaderValue::try_from(*value).unwrap(),
            );
        }
        map
    }

    #[test]
    fn extracts_bearer_tokens_from_the_authorization_header() {
        let ctx = Context::extract(&headers(&[("authorization", "Bearer abc")]), &HashMap::new());
        assert_eq!(ctx.token(), Some("abc"));
    }

    #[test]
    fn falls_back_to_the_token_query_parameter() {
        let query = HashMap::from([("token".to_string(), "xyz".to_string())]);
        let ctx = Context::extract(&headers(&[]), &query);
        assert_eq!(ctx.token(), Some("xyz"));
    }

    #[test]
    fn header_wins_over_query_and_absence_is_none() {
        let query = HashMap::from([("token".to_string(), "xyz".to_string())]);
        let ctx = Context::extract(&headers(&[("authorization", "Bearer abc")]), &query);
        assert_eq!(ctx.token(), Some("abc"));

        assert_eq!(Context::extract(&headers(&[]), &HashMap::new()).token(), None);
    }

    #[test]
    fn non_bearer_authorization_is_ignored() {
        let ctx = Context::extract(&headers(&[("authorization", "Basic dXNlcg==")]), &HashMap::new());
        assert_eq!(ctx.token(), None);
    }
}
