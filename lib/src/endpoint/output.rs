/* adapters from handler return types to what the engine executes, so
attributed functions can return plain values, Results, or iterators */

use super::{HandlerError, ValueStream};
use crate::schema::convert::IntoValue;
use crate::schema::{Blob, Date, Value};

pub trait IntoHandlerOutput {
    type Ok: IntoValue;
    fn into_handler_output(self) -> Result<Value, HandlerError>;
}

// implemented per type constructor (not as a blanket over IntoValue) so the
// Result impl below stays coherent
macro_rules! infallible_outputs {
    ($($ty:ty),* $(,)?) => {
        $(
            impl IntoHandlerOutput for $ty {
                type Ok = $ty;
                fn into_handler_output(self) -> Result<Value, HandlerError> {
                    Ok(self.into_value())
                }
            }
        )*
    };
}

infallible_outputs!(bool, i32, u32, i64, u64, f32, f64, String, Blob, Date);

impl<T: IntoValue> IntoHandlerOutput for Vec<T> {
    type Ok = Vec<T>;
    fn into_handler_output(self) -> Result<Value, HandlerError> {
        Ok(self.into_value())
    }
}

impl<K: IntoValue, V: IntoValue> IntoHandlerOutput for std::collections::BTreeMap<K, V> {
    type Ok = std::collections::BTreeMap<K, V>;
    fn into_handler_output(self) -> Result<Value, HandlerError> {
        Ok(self.into_value())
    }
}

impl<K: IntoValue, V: IntoValue> IntoHandlerOutput for std::collections::HashMap<K, V> {
    type Ok = std::collections::HashMap<K, V>;
    fn into_handler_output(self) -> Result<Value, HandlerError> {
        Ok(self.into_value())
    }
}

impl<T: IntoValue, E: Into<HandlerError>> IntoHandlerOutput for Result<T, E> {
    type Ok = T;
    fn into_handler_output(self) -> Result<Value, HandlerError> {
        self.map(IntoValue::into_value).map_err(Into::into)
    }
}

/// The return contract of streaming (`sse`/`live`) handlers:
/// `Result<impl Iterator<Item = T>, HandlerError>`.
pub trait StreamOutput {
    type Item: IntoValue;
    fn into_value_stream(self) -> Result<ValueStream, HandlerError>;
}

impl<I, T, E> StreamOutput for Result<I, E>
where
    I: Iterator<Item = T> + Send + 'static,
    T: IntoValue,
    E: Into<HandlerError>,
{
    type Item = T;
    fn into_value_stream(self) -> Result<ValueStream, HandlerError> {
        let items = self.map_err(Into::into)?;
        Ok(Box::new(items.map(|item| Ok(item.into_value()))))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_and_result_returns_become_outputs() {
        assert_eq!(42i64.into_handler_output().unwrap(), Value::I64(42));
        assert_eq!(
            Ok::<_, HandlerError>("hi".to_string())
                .into_handler_output()
                .unwrap(),
            Value::Str("hi".into())
        );
        assert!(
            Err::<i64, HandlerError>("boom".into())
                .into_handler_output()
                .is_err()
        );
    }

    #[test]
    fn iterators_become_value_streams() {
        let stream = Ok::<_, HandlerError>((0..3).map(|i| i as i64))
            .into_value_stream()
            .unwrap();
        let items: Vec<_> = stream.map(Result::unwrap).collect();
        assert_eq!(items, vec![Value::I64(0), Value::I64(1), Value::I64(2)]);
    }
}
