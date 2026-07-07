use crate::schema::Value;

use super::super::Context;
use super::handler::HandlerError;

pub type ValueStream = Box<dyn Iterator<Item = Result<Value, HandlerError>> + Send>;

pub trait Source: Send + Sync {
    fn subscribe(&self, ctx: &Context, args: &[Value]) -> Result<ValueStream, HandlerError>;
}

impl<F> Source for F
where
    F: Fn(&Context, &[Value]) -> Result<ValueStream, HandlerError> + Send + Sync,
{
    fn subscribe(&self, ctx: &Context, args: &[Value]) -> Result<ValueStream, HandlerError> {
        self(ctx, args)
    }
}
