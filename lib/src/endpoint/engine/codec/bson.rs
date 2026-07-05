use serde::{Deserialize, Serialize};

use crate::endpoint::HandlerError;
use crate::schema::Value;

#[derive(Serialize)]
struct ArgsFrame<'a> {
	args: &'a [Value]
}

#[cfg_attr(test, derive(Serialize))]
#[derive(Deserialize)]
struct ResultFrame {
	ok: Option<Value>,
	err: Option<String>
}

pub fn encode_args(args: &[Value]) -> Result<Vec<u8>, HandlerError> {
	Ok(bson::serialize_to_vec(&ArgsFrame { args })?)
}

pub fn decode_result(bytes: &[u8]) -> Result<Value, HandlerError> {
	let frame: ResultFrame = bson::deserialize_from_slice(bytes)?;
	match (frame.ok, frame.err) {
		(Some(value), None) => Ok(value),
		(None, Some(message)) => Err(message.into()),
		_ => Err("wasm result frame must contain exactly one of `ok` or `err`".into())
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn args_frame_roundtrips_and_result_frames_decode() {
		let args = vec![Value::I64(2), Value::Str("x".into())];
		let bytes = encode_args(&args).unwrap();
		let doc: bson::Document = bson::deserialize_from_slice(&bytes).unwrap();
		assert!(doc.contains_key("args"));

		let ok = bson::serialize_to_vec(&ResultFrame { ok: Some(Value::I64(5)), err: None }).unwrap();
		assert_eq!(decode_result(&ok).unwrap(), Value::I64(5));

		let err = bson::serialize_to_vec(&ResultFrame { ok: None, err: Some("boom".into()) }).unwrap();
		assert_eq!(decode_result(&err).unwrap_err().to_string(), "boom");
	}
}
