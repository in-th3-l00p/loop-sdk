use crate::schema::Value;

use super::handler::HandlerError;

pub type ValueStream = Box<dyn Iterator<Item = Result<Value, HandlerError>> + Send>;

pub trait Source: Send + Sync {
    fn subscribe(&self, args: &[Value]) -> Result<ValueStream, HandlerError>;
}

impl<F> Source for F
where
    F: Fn(&[Value]) -> Result<ValueStream, HandlerError> + Send + Sync,
{
    fn subscribe(&self, args: &[Value]) -> Result<ValueStream, HandlerError> {
        self(args)
    }
}
