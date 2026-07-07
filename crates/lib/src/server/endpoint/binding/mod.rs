mod handler;
mod source;

pub use handler::{Handler, HandlerError, status_of, with_status};
pub use source::{Source, ValueStream};

use std::sync::Arc;

pub enum Binding {
    Native(Arc<dyn Handler>),
    Stream(Arc<dyn Source>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::Value;

    #[test]
    fn native_binding_dispatches_through_handler_trait_object() {
        let binding = Binding::Native(Arc::new(|args: &[Value]| match args {
            [Value::I64(a), Value::I64(b)] => Ok(Value::I64(a + b)),
            _ => Err("expected two i64 arguments".into()),
        }));

        let Binding::Native(handler) = &binding else {
            unreachable!()
        };
        let result = handler.call(&[Value::I64(2), Value::I64(3)]).unwrap();

        assert_eq!(result, Value::I64(5));
    }

    #[test]
    fn native_binding_propagates_handler_errors() {
        let binding = Binding::Native(Arc::new(|_: &[Value]| -> Result<Value, HandlerError> {
            Err("boom".into())
        }));

        let Binding::Native(handler) = &binding else {
            unreachable!()
        };
        assert!(handler.call(&[]).is_err());
    }

    #[test]
    fn stream_binding_yields_values_through_source_trait_object() {
        let binding = Binding::Stream(Arc::new(
            |_: &[Value]| -> Result<ValueStream, HandlerError> {
                Ok(Box::new((0..3).map(|i| Ok(Value::I64(i)))))
            },
        ));

        let Binding::Stream(source) = &binding else {
            unreachable!()
        };
        let items: Vec<_> = source.subscribe(&[]).unwrap().map(Result::unwrap).collect();

        assert_eq!(items, vec![Value::I64(0), Value::I64(1), Value::I64(2)]);
    }
}
