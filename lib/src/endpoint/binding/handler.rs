use std::error::Error;

use crate::schema::Value;

pub type HandlerError = Box<dyn Error + Send + Sync>;

pub trait Handler: Send + Sync {
    fn call(&self, args: &[Value]) -> Result<Value, HandlerError>;
}

impl<F> Handler for F
where
    F: Fn(&[Value]) -> Result<Value, HandlerError> + Send + Sync,
{
    fn call(&self, args: &[Value]) -> Result<Value, HandlerError> {
        self(args)
    }
}
