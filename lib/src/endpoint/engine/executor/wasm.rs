use std::sync::Arc;

use wasmtime::{Instance, InstancePre, Linker, Module, Store};
use wasmtime_wasi::WasiCtxBuilder;
use wasmtime_wasi::p1::WasiP1Ctx;

use super::super::codec;
use crate::endpoint::HandlerError;
use crate::schema::Value;

pub struct WasmHandler {
	engine: wasmtime::Engine,
	instance_pre: InstancePre<WasiP1Ctx>,
	export: String
}

fn store(engine: &wasmtime::Engine) -> Store<WasiP1Ctx> {
	Store::new(engine, WasiCtxBuilder::new().inherit_stdout().inherit_stderr().build_p1())
}

// wasip1 reactor modules expose their initializers through `_initialize`
fn instantiate(pre: &InstancePre<WasiP1Ctx>, store: &mut Store<WasiP1Ctx>) -> wasmtime::Result<Instance> {
	let instance = pre.instantiate(&mut *store)?;
	if let Ok(init) = instance.get_typed_func::<(), ()>(&mut *store, "_initialize") {
		init.call(store, ())?;
	}
	Ok(instance)
}

impl WasmHandler {
	pub fn new(bytes: &[u8], export: &str) -> Result<Self, String> {
		let engine = wasmtime::Engine::default();
		let module = Module::new(&engine, bytes).map_err(|e| e.to_string())?;
		let mut linker = Linker::new(&engine);
		wasmtime_wasi::p1::add_to_linker_sync(&mut linker, |ctx| ctx).map_err(|e| e.to_string())?;
		let instance_pre = linker.instantiate_pre(&module).map_err(|e| e.to_string())?;

		let handler = Self { engine, instance_pre, export: export.into() };
		handler.check_exports()?;
		Ok(handler)
	}

	fn check_exports(&self) -> Result<(), String> {
		let mut store = store(&self.engine);
		let instance = instantiate(&self.instance_pre, &mut store).map_err(|e| e.to_string())?;
		instance
			.get_memory(&mut store, "memory")
			.ok_or("wasm module must export `memory`".to_string())?;
		instance
			.get_typed_func::<i32, i32>(&mut store, "loop_alloc")
			.map_err(|e| format!("wasm module must export `loop_alloc(i32) -> i32`: {e}"))?;
		instance
			.get_typed_func::<(i32, i32), i64>(&mut store, &self.export)
			.map_err(|e| format!("wasm module must export `{}(i32, i32) -> i64`: {}", self.export, e))?;
		Ok(())
	}

	pub async fn call(self: &Arc<Self>, args: Vec<Value>) -> Result<Value, HandlerError> {
		let handler = self.clone();
		tokio::task::spawn_blocking(move || handler.call_blocking(&args))
			.await
			.map_err(|e| -> HandlerError { format!("wasm call panicked: {e}").into() })?
	}

	fn call_blocking(&self, args: &[Value]) -> Result<Value, HandlerError> {
		let input = codec::bson::encode_args(args)?;

		let mut store = store(&self.engine);
		let instance = instantiate(&self.instance_pre, &mut store)?;
		let memory = instance.get_memory(&mut store, "memory").ok_or("missing `memory` export")?;
		let alloc = instance.get_typed_func::<i32, i32>(&mut store, "loop_alloc")?;
		let call = instance.get_typed_func::<(i32, i32), i64>(&mut store, &self.export)?;

		let len = i32::try_from(input.len()).map_err(|_| "args frame too large for wasm memory")?;
		let ptr = alloc.call(&mut store, len)?;
		memory.write(&mut store, ptr as usize, &input)?;

		let packed = call.call(&mut store, (ptr, len))?;
		let out_ptr = (packed >> 32) as u32 as usize;
		let out_len = packed as u32 as usize;

		let mut output = vec![0u8; out_len];
		memory.read(&store, out_ptr, &mut output)?;
		codec::bson::decode_result(&output)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use serde::Serialize;

	#[derive(Serialize)]
	struct OkFrame {
		ok: Value
	}

	#[derive(Serialize)]
	struct ErrFrame {
		err: String
	}

	// wasmtime's `wat` feature lets Module::new accept WAT text directly
	fn module_returning(frame_bytes: &[u8]) -> Vec<u8> {
		let escaped: String = frame_bytes.iter().map(|b| format!("\\{b:02x}")).collect();
		let packed = ((4096i64) << 32) | frame_bytes.len() as i64;
		format!(
			r#"(module
				(memory (export "memory") 1)
				(global $next (mut i32) (i32.const 8192))
				(func (export "loop_alloc") (param i32) (result i32)
					global.get $next
					global.get $next
					local.get 0
					i32.add
					global.set $next)
				(func (export "run") (param i32 i32) (result i64)
					i64.const {packed})
				(data (i32.const 4096) "{escaped}"))"#
		)
		.into_bytes()
	}

	#[tokio::test]
	async fn calls_wasm_export_and_decodes_ok_frame() {
		let frame = bson::serialize_to_vec(&OkFrame { ok: Value::I64(5) }).unwrap();
		let handler = Arc::new(WasmHandler::new(&module_returning(&frame), "run").unwrap());

		assert_eq!(handler.call(vec![Value::I64(2), Value::I64(3)]).await.unwrap(), Value::I64(5));
	}

	#[tokio::test]
	async fn surfaces_wasm_err_frame_as_handler_error() {
		let frame = bson::serialize_to_vec(&ErrFrame { err: "boom".into() }).unwrap();
		let handler = Arc::new(WasmHandler::new(&module_returning(&frame), "run").unwrap());

		assert_eq!(handler.call(vec![]).await.unwrap_err().to_string(), "boom");
	}

	#[test]
	fn rejects_module_missing_required_exports() {
		assert!(WasmHandler::new(b"(module)", "run").is_err());
	}
}
