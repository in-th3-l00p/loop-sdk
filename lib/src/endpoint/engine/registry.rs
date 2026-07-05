use std::collections::HashSet;
use std::sync::Arc;

use tokio::sync::mpsc;

use super::error::EngineError;
use super::executor::{Executor, WasmHandler};
use crate::endpoint::{Access, Binding, Endpoint, Signature};
use crate::schema::Value;

pub struct RegisteredEndpoint {
    pub name: String,
    pub signature: Signature,
    pub access: Access,
    executor: Executor,
}

impl RegisteredEndpoint {
    pub async fn call(&self, args: Vec<Value>) -> Result<Value, EngineError> {
        self.validate_args(&args)?;
        let output = self
            .executor
            .call(args)
            .await
            .map_err(|e| EngineError::Handler(e.to_string()))?;
        self.signature
            .output
            .validate(&output)
            .map_err(EngineError::Output)?;
        Ok(output)
    }

    pub async fn stream(
        self: &Arc<Self>,
        args: Vec<Value>,
    ) -> Result<mpsc::Receiver<Result<Value, EngineError>>, EngineError> {
        self.validate_args(&args)?;
        let mut source = self
            .executor
            .stream(args)
            .await
            .map_err(|e| EngineError::Handler(e.to_string()))?;

        let endpoint = self.clone();
        let (tx, rx) = mpsc::channel(16);
        tokio::spawn(async move {
            while let Some(item) = source.recv().await {
                let item = match item {
                    Ok(value) => endpoint
                        .signature
                        .output
                        .validate(&value)
                        .map(|()| value)
                        .map_err(EngineError::Output),
                    Err(e) => Err(EngineError::Handler(e.to_string())),
                };
                if tx.send(item).await.is_err() {
                    break;
                }
            }
        });

        Ok(rx)
    }

    fn validate_args(&self, args: &[Value]) -> Result<(), EngineError> {
        if args.len() != self.signature.params.len() {
            return Err(EngineError::Decode(format!(
                "expected {} arguments, got {}",
                self.signature.params.len(),
                args.len()
            )));
        }
        for (param, arg) in self.signature.params.iter().zip(args) {
            param.schema.validate(arg).map_err(EngineError::Input)?;
        }
        Ok(())
    }
}

pub fn prepare(endpoints: Vec<Endpoint>) -> Result<Vec<Arc<RegisteredEndpoint>>, EngineError> {
    let mut routes = HashSet::new();
    let mut prepared = Vec::with_capacity(endpoints.len());

    for endpoint in endpoints {
        // Sse and Live both mount as GET routes, so they share the GET slot
        let (method, url) = match &endpoint.access {
            Access::Rest { method, url } => (method.to_string(), url.clone()),
            Access::Live { url } | Access::Sse { url } => ("GET".to_string(), url.clone()),
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
                Executor::Wasm(Arc::new(
                    WasmHandler::new(&bytes, &export).map_err(EngineError::Wasm)?,
                ))
            }
        };

        prepared.push(Arc::new(RegisteredEndpoint {
            name: endpoint.name,
            signature: endpoint.signature,
            access: endpoint.access,
            executor,
        }));
    }

    Ok(prepared)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::schema::{Primitive, Schema};
    use http::Method;

    fn rest(name: &str, url: &str) -> Endpoint {
        Endpoint {
            name: name.into(),
            signature: Signature {
                params: vec![],
                output: Schema::Primitive(Primitive::I64),
            },
            access: Access::Rest {
                method: Method::GET,
                url: url.into(),
            },
            binding: Binding::Native(Arc::new(|_: &[Value]| Ok(Value::I64(0)))),
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
            signature: Signature {
                params: vec![],
                output: Schema::Primitive(Primitive::I64),
            },
            access: Access::Sse { url: "/a".into() },
            binding: Binding::Native(Arc::new(|_: &[Value]| Ok(Value::I64(0)))),
        };
        assert!(matches!(
            prepare(vec![rest("a", "/a"), sse]),
            Err(EngineError::Conflict(_))
        ));
    }

    #[test]
    fn rejects_stream_binding_on_rest_access() {
        let endpoint =
            Endpoint {
                name: "bad".into(),
                signature: Signature {
                    params: vec![],
                    output: Schema::Primitive(Primitive::I64),
                },
                access: Access::Rest {
                    method: Method::GET,
                    url: "/bad".into(),
                },
                binding:
                    Binding::Stream(
                        Arc::new(
                            |_: &[Value]| -> Result<
                                crate::endpoint::ValueStream,
                                crate::endpoint::HandlerError,
                            > { Ok(Box::new(std::iter::empty())) },
                        ),
                    ),
            };
        assert!(matches!(
            prepare(vec![endpoint]),
            Err(EngineError::Conflict(_))
        ));
    }

    #[test]
    fn rejects_wasm_binding_on_streaming_access() {
        let endpoint = Endpoint {
            name: "bad".into(),
            signature: Signature {
                params: vec![],
                output: Schema::Primitive(Primitive::I64),
            },
            access: Access::Sse { url: "/bad".into() },
            binding: Binding::Wasm {
                bytes: vec![],
                export: "run".into(),
            },
        };
        assert!(matches!(
            prepare(vec![endpoint]),
            Err(EngineError::Conflict(_))
        ));
    }

    #[tokio::test]
    async fn call_validates_inputs_and_outputs() {
        let endpoints = prepare(vec![Endpoint {
            name: "id".into(),
            signature: Signature {
                params: vec![crate::endpoint::Parameter {
                    name: "n".into(),
                    schema: Schema::Primitive(Primitive::I64),
                }],
                output: Schema::Primitive(Primitive::I64),
            },
            access: Access::Rest {
                method: Method::POST,
                url: "/id".into(),
            },
            binding: Binding::Native(Arc::new(|args: &[Value]| Ok(args[0].clone()))),
        }])
        .unwrap();
        let endpoint = &endpoints[0];

        assert_eq!(
            endpoint.call(vec![Value::I64(7)]).await.unwrap(),
            Value::I64(7)
        );
        assert!(matches!(
            endpoint.call(vec![Value::Bool(true)]).await,
            Err(EngineError::Input(_))
        ));
        assert!(matches!(
            endpoint.call(vec![]).await,
            Err(EngineError::Decode(_))
        ));
    }
}
