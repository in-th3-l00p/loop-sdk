use std::error::Error;
use std::sync::Arc;

use crate::schema::Value;

pub type HandlerError = Box<dyn Error + Send + Sync>;

pub trait Handler: Send + Sync {
	fn call(&self, args: &[Value]) -> Result<Value, HandlerError>;
}

impl<F> Handler for F
where
	F: Fn(&[Value]) -> Result<Value, HandlerError> + Send + Sync
{
	fn call(&self, args: &[Value]) -> Result<Value, HandlerError> {
		self(args)
	}
}

pub enum Binding {
	Native(Arc<dyn Handler>),
	Wasm { bytes: Vec<u8>, export: String }
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn native_binding_dispatches_through_handler_trait_object() {
		let binding = Binding::Native(Arc::new(|args: &[Value]| match args {
			[Value::I64(a), Value::I64(b)] => Ok(Value::I64(a + b)),
			_ => Err("expected two i64 arguments".into())
		}));

		let Binding::Native(handler) = &binding else { unreachable!() };
		let result = handler.call(&[Value::I64(2), Value::I64(3)]).unwrap();

		assert_eq!(result, Value::I64(5));
	}

	#[test]
	fn native_binding_propagates_handler_errors() {
		let binding =
			Binding::Native(Arc::new(|_: &[Value]| -> Result<Value, HandlerError> { Err("boom".into()) }));

		let Binding::Native(handler) = &binding else { unreachable!() };
		assert!(handler.call(&[]).is_err());
	}

	#[test]
	fn wasm_binding_holds_module_bytes_and_export_name() {
		let binding = Binding::Wasm { bytes: vec![0, 97, 115, 109], export: "add".to_string() };

		let Binding::Wasm { bytes, export } = &binding else { unreachable!() };
		assert_eq!(export, "add");
		assert!(!bytes.is_empty());
	}
}
