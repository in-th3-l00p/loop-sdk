/* runtime abi encoding/decoding behind the #[contract] macro: the macro
embeds canonical signature strings at compile time, this module turns them
into calldata and back via alloy's dyn-abi. hidden from the public surface —
generated code is the only intended caller */

use alloy::dyn_abi::{DynSolType, DynSolValue};
use alloy::primitives::keccak256;

use super::error::EthError;
use super::types::{Address, U256, Wei};
use crate::schema::Blob;

/// A single abi value in transit. Deliberately smaller than the abi type
/// system: bit widths and fixed sizes come from the signature string, not
/// from the value.
#[doc(hidden)]
#[derive(Debug, Clone, PartialEq)]
pub enum AbiParam {
    Unit,
    Address(Address),
    Uint(U256),
    Int(i64),
    Bool(bool),
    Str(String),
    Bytes(Vec<u8>),
    List(Vec<AbiParam>),
}

/// Conversion between Rust parameter/return types and [`AbiParam`],
/// implemented for every type the #[contract] macro maps abi types onto.
#[doc(hidden)]
pub trait AbiType: Sized {
    fn into_abi(self) -> AbiParam;
    fn from_abi(param: AbiParam) -> Result<Self, EthError>;
}

fn expected<T>(kind: &str, got: &AbiParam) -> Result<T, EthError> {
    Err(EthError::Abi(format!("expected {kind}, got {got:?}")))
}

impl AbiType for () {
    fn into_abi(self) -> AbiParam {
        AbiParam::Unit
    }
    fn from_abi(_: AbiParam) -> Result<Self, EthError> {
        Ok(())
    }
}

impl AbiType for Address {
    fn into_abi(self) -> AbiParam {
        AbiParam::Address(self)
    }
    fn from_abi(param: AbiParam) -> Result<Self, EthError> {
        match param {
            AbiParam::Address(a) => Ok(a),
            other => expected("address", &other),
        }
    }
}

impl AbiType for U256 {
    fn into_abi(self) -> AbiParam {
        AbiParam::Uint(self)
    }
    fn from_abi(param: AbiParam) -> Result<Self, EthError> {
        match param {
            AbiParam::Uint(u) => Ok(u),
            other => expected("uint", &other),
        }
    }
}

impl AbiType for Wei {
    fn into_abi(self) -> AbiParam {
        AbiParam::Uint(self.as_u256())
    }
    fn from_abi(param: AbiParam) -> Result<Self, EthError> {
        U256::from_abi(param).map(Wei::from)
    }
}

impl AbiType for bool {
    fn into_abi(self) -> AbiParam {
        AbiParam::Bool(self)
    }
    fn from_abi(param: AbiParam) -> Result<Self, EthError> {
        match param {
            AbiParam::Bool(b) => Ok(b),
            other => expected("bool", &other),
        }
    }
}

impl AbiType for String {
    fn into_abi(self) -> AbiParam {
        AbiParam::Str(self)
    }
    fn from_abi(param: AbiParam) -> Result<Self, EthError> {
        match param {
            AbiParam::Str(s) => Ok(s),
            other => expected("string", &other),
        }
    }
}

impl AbiType for Blob {
    fn into_abi(self) -> AbiParam {
        AbiParam::Bytes(self.0)
    }
    fn from_abi(param: AbiParam) -> Result<Self, EthError> {
        match param {
            AbiParam::Bytes(bytes) => Ok(Blob(bytes)),
            other => expected("bytes", &other),
        }
    }
}

macro_rules! uint_abi_types {
    ($($ty:ty),* $(,)?) => {
        $(
            impl AbiType for $ty {
                fn into_abi(self) -> AbiParam {
                    AbiParam::Uint(U256::from(u64::from(self)))
                }
                fn from_abi(param: AbiParam) -> Result<Self, EthError> {
                    let AbiParam::Uint(u) = param else {
                        return expected("uint", &param);
                    };
                    u.to_u64()
                        .and_then(|v| <$ty>::try_from(v).ok())
                        .ok_or_else(|| {
                            EthError::Abi(format!(
                                "{u} does not fit in {}", stringify!($ty)
                            ))
                        })
                }
            }
        )*
    };
}

uint_abi_types!(u8, u16, u32, u64);

macro_rules! int_abi_types {
    ($($ty:ty),* $(,)?) => {
        $(
            impl AbiType for $ty {
                fn into_abi(self) -> AbiParam {
                    AbiParam::Int(i64::from(self))
                }
                fn from_abi(param: AbiParam) -> Result<Self, EthError> {
                    let AbiParam::Int(i) = param else {
                        return expected("int", &param);
                    };
                    <$ty>::try_from(i).map_err(|_| {
                        EthError::Abi(format!("{i} does not fit in {}", stringify!($ty)))
                    })
                }
            }
        )*
    };
}

int_abi_types!(i32, i64);

impl<T: AbiType> AbiType for Vec<T> {
    fn into_abi(self) -> AbiParam {
        AbiParam::List(self.into_iter().map(AbiType::into_abi).collect())
    }
    fn from_abi(param: AbiParam) -> Result<Self, EthError> {
        match param {
            AbiParam::List(items) => items.into_iter().map(T::from_abi).collect(),
            other => expected("array", &other),
        }
    }
}

/// The four-byte function selector for a canonical signature like
/// `"transfer(address,uint256)"`.
pub fn selector(signature: &str) -> [u8; 4] {
    let hash = keccak256(signature.as_bytes());
    [hash[0], hash[1], hash[2], hash[3]]
}

/// The topic0 of an event with a canonical signature like
/// `"Transfer(address,address,uint256)"`.
pub fn event_topic(signature: &str) -> [u8; 32] {
    keccak256(signature.as_bytes()).0
}

fn param_types(signature: &str) -> Result<Vec<DynSolType>, EthError> {
    let open = signature
        .find('(')
        .ok_or_else(|| EthError::Abi(format!("malformed signature {signature:?}")))?;
    let types = DynSolType::parse(&signature[open..])
        .map_err(|e| EthError::Abi(format!("malformed signature {signature:?}: {e}")))?;
    match types {
        DynSolType::Tuple(types) => Ok(types),
        single => Ok(vec![single]),
    }
}

/// Encodes a full calldata payload: selector + abi-encoded arguments, with
/// the argument shapes taken from the signature.
pub fn encode_call(signature: &str, args: Vec<AbiParam>) -> Result<Vec<u8>, EthError> {
    let types = param_types(signature)?;
    if types.len() != args.len() {
        return Err(EthError::Abi(format!(
            "{signature} takes {} arguments, got {}",
            types.len(),
            args.len()
        )));
    }
    let values = args
        .into_iter()
        .zip(&types)
        .map(|(arg, ty)| to_sol_value(arg, ty))
        .collect::<Result<Vec<_>, _>>()?;

    let mut data = selector(signature).to_vec();
    data.extend(DynSolValue::Tuple(values).abi_encode_params());
    Ok(data)
}

/// Decodes the return data of a call whose single output has the given
/// canonical type.
pub fn decode_return(sol_type: &str, data: &[u8]) -> Result<AbiParam, EthError> {
    let ty = DynSolType::parse(&format!("({sol_type})"))
        .map_err(|e| EthError::Abi(format!("malformed return type {sol_type:?}: {e}")))?;
    let value = ty
        .abi_decode_sequence(data)
        .map_err(|e| EthError::Abi(format!("return data does not match {sol_type:?}: {e}")))?;
    match value {
        DynSolValue::Tuple(mut values) if values.len() == 1 => from_sol_value(values.remove(0)),
        _ => Err(EthError::Abi(format!(
            "return data does not match {sol_type:?}"
        ))),
    }
}

/// Decodes one log into params in field declaration order. `fields` pairs
/// each field's canonical type with whether it is indexed; indexed fields
/// come from the topics, the rest from the data section.
pub fn decode_log(
    fields: &[(&str, bool)],
    topics: &[[u8; 32]],
    data: &[u8],
) -> Result<Vec<AbiParam>, EthError> {
    let mut topics = topics.iter().skip(1); // topic0 is the event signature
    let unindexed_types = fields
        .iter()
        .filter(|(_, indexed)| !indexed)
        .map(|(ty, _)| {
            DynSolType::parse(ty).map_err(|e| EthError::Abi(format!("malformed type {ty:?}: {e}")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut unindexed = DynSolType::Tuple(unindexed_types)
        .abi_decode_sequence(data)
        .map_err(|e| EthError::Abi(format!("log data does not match event fields: {e}")))?
        .as_tuple()
        .map(<[DynSolValue]>::to_vec)
        .unwrap_or_default()
        .into_iter();

    fields
        .iter()
        .map(|(ty_str, indexed)| {
            if *indexed {
                let topic = topics
                    .next()
                    .ok_or_else(|| EthError::Abi("log is missing an indexed topic".into()))?;
                let ty = DynSolType::parse(ty_str)
                    .map_err(|e| EthError::Abi(format!("malformed type {ty_str:?}: {e}")))?;
                if !matches!(
                    ty,
                    DynSolType::Address
                        | DynSolType::Bool
                        | DynSolType::Uint(_)
                        | DynSolType::Int(_)
                        | DynSolType::FixedBytes(_)
                ) {
                    return Err(EthError::Unsupported(format!(
                        "indexed {ty_str} fields only carry a hash on-chain and cannot be decoded"
                    )));
                }
                let value = ty.abi_decode(topic).map_err(|e| {
                    EthError::Abi(format!("indexed topic does not match {ty_str:?}: {e}"))
                })?;
                from_sol_value(value)
            } else {
                unindexed
                    .next()
                    .map(from_sol_value)
                    .unwrap_or_else(|| Err(EthError::Abi("log data field count mismatch".into())))
            }
        })
        .collect()
}

/// Shapes an [`AbiParam`] into the concrete sol value the signature expects,
/// so bit widths and fixed sizes always come from the abi.
fn to_sol_value(param: AbiParam, ty: &DynSolType) -> Result<DynSolValue, EthError> {
    match (param, ty) {
        (AbiParam::Address(a), DynSolType::Address) => Ok(DynSolValue::Address(a.0)),
        (AbiParam::Uint(u), DynSolType::Uint(bits)) => Ok(DynSolValue::Uint(u.0, *bits)),
        (AbiParam::Int(i), DynSolType::Int(bits)) => Ok(DynSolValue::Int(
            alloy::primitives::I256::try_from(i)
                .map_err(|e| EthError::Abi(format!("int conversion failed: {e}")))?,
            *bits,
        )),
        (AbiParam::Bool(b), DynSolType::Bool) => Ok(DynSolValue::Bool(b)),
        (AbiParam::Str(s), DynSolType::String) => Ok(DynSolValue::String(s)),
        (AbiParam::Bytes(bytes), DynSolType::Bytes) => Ok(DynSolValue::Bytes(bytes)),
        (AbiParam::Bytes(bytes), DynSolType::FixedBytes(size)) => {
            if bytes.len() != *size {
                return Err(EthError::Abi(format!(
                    "bytes{size} takes exactly {size} bytes, got {}",
                    bytes.len()
                )));
            }
            let mut word = alloy::primitives::B256::ZERO;
            word[..*size].copy_from_slice(&bytes);
            Ok(DynSolValue::FixedBytes(word, *size))
        }
        (AbiParam::List(items), DynSolType::Array(inner)) => Ok(DynSolValue::Array(
            items
                .into_iter()
                .map(|item| to_sol_value(item, inner))
                .collect::<Result<_, _>>()?,
        )),
        (AbiParam::List(items), DynSolType::FixedArray(inner, size)) => {
            if items.len() != *size {
                return Err(EthError::Abi(format!(
                    "fixed array takes exactly {size} items, got {}",
                    items.len()
                )));
            }
            Ok(DynSolValue::FixedArray(
                items
                    .into_iter()
                    .map(|item| to_sol_value(item, inner))
                    .collect::<Result<_, _>>()?,
            ))
        }
        (param, ty) => Err(EthError::Abi(format!("cannot encode {param:?} as {ty}"))),
    }
}

fn from_sol_value(value: DynSolValue) -> Result<AbiParam, EthError> {
    match value {
        DynSolValue::Address(a) => Ok(AbiParam::Address(Address(a))),
        DynSolValue::Uint(u, _) => Ok(AbiParam::Uint(U256(u))),
        DynSolValue::Int(i, _) => i64::try_from(i)
            .map(AbiParam::Int)
            .map_err(|_| EthError::Abi(format!("{i} does not fit in i64"))),
        DynSolValue::Bool(b) => Ok(AbiParam::Bool(b)),
        DynSolValue::String(s) => Ok(AbiParam::Str(s)),
        DynSolValue::Bytes(bytes) => Ok(AbiParam::Bytes(bytes)),
        DynSolValue::FixedBytes(word, size) => Ok(AbiParam::Bytes(word[..size].to_vec())),
        DynSolValue::Array(items) | DynSolValue::FixedArray(items) => Ok(AbiParam::List(
            items
                .into_iter()
                .map(from_sol_value)
                .collect::<Result<_, _>>()?,
        )),
        other => Err(EthError::Unsupported(format!(
            "cannot decode abi value {other:?}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(hex: &str) -> Address {
        Address::parse(hex).unwrap()
    }

    #[test]
    fn selectors_match_known_vectors() {
        assert_eq!(selector("balanceOf(address)"), [0x70, 0xa0, 0x82, 0x31]);
        assert_eq!(
            selector("transfer(address,uint256)"),
            [0xa9, 0x05, 0x9c, 0xbb]
        );
    }

    #[test]
    fn event_topics_match_known_vectors() {
        // the canonical erc-20 Transfer topic0
        let topic = event_topic("Transfer(address,address,uint256)");
        assert_eq!(
            alloy::primitives::hex::encode(topic),
            "ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef"
        );
    }

    #[test]
    fn encode_call_produces_selector_and_padded_args() {
        let holder = addr("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045");
        let data = encode_call("balanceOf(address)", vec![holder.into_abi()]).unwrap();
        assert_eq!(data.len(), 4 + 32);
        assert_eq!(&data[..4], &[0x70, 0xa0, 0x82, 0x31]);
        assert_eq!(&data[4..16], &[0u8; 12]); // left-padded address
        assert_eq!(
            alloy::primitives::hex::encode(&data[16..]),
            "d8da6bf26964af9d7eed9e03e53415d37aa96045"
        );
    }

    #[test]
    fn encode_call_checks_arity() {
        assert!(encode_call("balanceOf(address)", vec![]).is_err());
    }

    #[test]
    fn returns_roundtrip_through_decode() {
        let encoded = encode_call("f(uint256)", vec![U256::from(1000u64).into_abi()]).unwrap();
        let decoded = decode_return("uint256", &encoded[4..]).unwrap();
        assert_eq!(decoded, AbiParam::Uint(U256::from(1000u64)));

        let encoded = encode_call("f(string)", vec!["USDC".to_string().into_abi()]).unwrap();
        let decoded = decode_return("string", &encoded[4..]).unwrap();
        assert_eq!(decoded, AbiParam::Str("USDC".into()));
    }

    #[test]
    fn dynamic_and_fixed_arrays_encode_differently() {
        let items = vec![U256::from(1u64).into_abi(), U256::from(2u64).into_abi()];
        let dynamic = encode_call("f(uint256[])", vec![AbiParam::List(items.clone())]).unwrap();
        let fixed = encode_call("f(uint256[2])", vec![AbiParam::List(items)]).unwrap();
        assert!(dynamic.len() > fixed.len()); // offset + length prefix
        assert_eq!(fixed.len(), 4 + 64);
    }

    #[test]
    fn logs_decode_indexed_and_data_fields_in_order() {
        let from = addr("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045");
        let to = addr("0x0000000000000000000000000000000000000001");

        let mut from_topic = [0u8; 32];
        from_topic[12..].copy_from_slice(from.0.as_slice());
        let mut to_topic = [0u8; 32];
        to_topic[12..].copy_from_slice(to.0.as_slice());
        let amount =
            encode_call("f(uint256)", vec![U256::from(500u64).into_abi()]).unwrap()[4..].to_vec();

        let fields = [("address", true), ("address", true), ("uint256", false)];
        let topics = [
            event_topic("Transfer(address,address,uint256)"),
            from_topic,
            to_topic,
        ];
        let params = decode_log(&fields, &topics, &amount).unwrap();
        assert_eq!(
            params,
            vec![
                AbiParam::Address(from),
                AbiParam::Address(to),
                AbiParam::Uint(U256::from(500u64)),
            ]
        );
    }

    #[test]
    fn indexed_dynamic_fields_are_rejected() {
        let fields = [("string", true)];
        let topics = [[0u8; 32], [0u8; 32]];
        assert!(matches!(
            decode_log(&fields, &topics, &[]),
            Err(EthError::Unsupported(_))
        ));
    }
}
