use std::collections::HashSet;
use std::sync::Arc;

use super::error::EngineError;
use super::executor::{Executor, WasmHandler};
use crate::endpoint::{Access, Binding, Endpoint, Signature};

pub struct PreparedEndpoint {
	pub name: String,
	pub signature: Signature,
	pub access: Access,
	pub executor: Executor
}

pub fn prepare(endpoints: Vec<Endpoint>) -> Result<Vec<Arc<PreparedEndpoint>>, EngineError> {
	let mut routes = HashSet::new();
	let mut prepared = Vec::with_capacity(endpoints.len());

	for endpoint in endpoints {
		// Sse and Live both mount as GET routes, so they share the GET slot
		let (method, url) = match &endpoint.access {
			Access::Rest { method, url } => (method.to_string(), url.clone()),
			Access::Live { url } | Access::Sse { url } => ("GET".to_string(), url.clone())
		};
		if !routes.insert((method.clone(), url.clone())) {
			return Err(EngineError::Conflict(format!(
				"endpoint {:?} duplicates route {method} {url}",
				endpoint.name
			)));
		}

		let streaming_access = matches!(endpoint.access, Access::Live { .. } | Access::Sse { .. });
		let executor = match endpoint.binding {
			Binding::Native(handler) => Executor::Native(handler),
			Binding::Stream(source) => {
				if !streaming_access {
					return Err(EngineError::Conflict(format!(
						"endpoint {:?} has a streaming binding but non-streaming access",
						endpoint.name
					)));
				}
				Executor::Stream(source)
			}
			Binding::Wasm { bytes, export } => {
				if streaming_access {
					return Err(EngineError::Conflict(format!(
						"endpoint {:?}: wasm bindings do not support streaming access yet",
						endpoint.name
					)));
				}
				Executor::Wasm(Arc::new(WasmHandler::new(&bytes, &export).map_err(EngineError::Wasm)?))
			}
		};

		prepared.push(Arc::new(PreparedEndpoint {
			name: endpoint.name,
			signature: endpoint.signature,
			access: endpoint.access,
			executor
		}));
	}

	Ok(prepared)
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::schema::{Primitive, Schema, Value};
	use http::Method;

	fn rest(name: &str, url: &str) -> Endpoint {
		Endpoint {
			name: name.into(),
			signature: Signature { params: vec![], output: Schema::Primitive(Primitive::I64) },
			access: Access::Rest { method: Method::GET, url: url.into() },
			binding: Binding::Native(Arc::new(|_: &[Value]| Ok(Value::I64(0))))
		}
	}

	#[test]
	fn accepts_distinct_routes() {
		assert!(prepare(vec![rest("a", "/a"), rest("b", "/b")]).is_ok());
	}

	#[test]
	fn rejects_duplicate_routes() {
		assert!(matches!(
			prepare(vec![rest("a", "/a"), rest("b", "/a")]),
			Err(EngineError::Conflict(_))
		));
	}

	#[test]
	fn rejects_sse_colliding_with_rest_get() {
		let sse = Endpoint {
			name: "s".into(),
			signature: Signature { params: vec![], output: Schema::Primitive(Primitive::I64) },
			access: Access::Sse { url: "/a".into() },
			binding: Binding::Native(Arc::new(|_: &[Value]| Ok(Value::I64(0))))
		};
		assert!(matches!(prepare(vec![rest("a", "/a"), sse]), Err(EngineError::Conflict(_))));
	}

	#[test]
	fn rejects_stream_binding_on_rest_access() {
		let endpoint = Endpoint {
			name: "bad".into(),
			signature: Signature { params: vec![], output: Schema::Primitive(Primitive::I64) },
			access: Access::Rest { method: Method::GET, url: "/bad".into() },
			binding: Binding::Stream(Arc::new(
				|_: &[Value]| -> Result<crate::endpoint::ValueStream, crate::endpoint::HandlerError> {
					Ok(Box::new(std::iter::empty()))
				}
			))
		};
		assert!(matches!(prepare(vec![endpoint]), Err(EngineError::Conflict(_))));
	}

	#[test]
	fn rejects_wasm_binding_on_streaming_access() {
		let endpoint = Endpoint {
			name: "bad".into(),
			signature: Signature { params: vec![], output: Schema::Primitive(Primitive::I64) },
			access: Access::Sse { url: "/bad".into() },
			binding: Binding::Wasm { bytes: vec![], export: "run".into() }
		};
		assert!(matches!(prepare(vec![endpoint]), Err(EngineError::Conflict(_))));
	}
}
