/* chain primitives that cross the wire as hex strings, validated by pattern
constraints before handler code runs */

use std::fmt;
use std::str::FromStr;

use super::error::EthError;
use crate::schema::{AsSchema, Constraint, FromValue, IntoValue, Primitive, Schema, Value};
use crate::server::endpoint::{HandlerError, IntoHandlerOutput};

const ADDRESS_PATTERN: &str = "^0x[0-9a-fA-F]{40}$";
const QUANTITY_PATTERN: &str = "^0x[0-9a-fA-F]{1,64}$";

fn hex_str_schema(pattern: &str) -> Schema {
    Schema::Constrained(
        Box::new(Schema::Primitive(Primitive::Str)),
        vec![Constraint::Pattern(pattern.to_string())],
    )
}

/// A 20-byte account or contract address. Displays EIP-55 checksummed;
/// parsing verifies the checksum only when the input is mixed-case.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Address(pub(crate) alloy::primitives::Address);

impl Address {
    pub const ZERO: Address = Address(alloy::primitives::Address::ZERO);

    pub fn parse(s: &str) -> Result<Address, EthError> {
        let Some(hex) = s.strip_prefix("0x") else {
            return Err(EthError::Parse(format!("address must start with 0x, got {s:?}")));
        };
        if hex.len() != 40 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(EthError::Parse(format!(
                "address must be 40 hex characters, got {s:?}"
            )));
        }
        let has_upper = hex.bytes().any(|b| b.is_ascii_uppercase());
        let has_lower = hex.bytes().any(|b| b.is_ascii_lowercase());
        if has_upper && has_lower {
            alloy::primitives::Address::parse_checksummed(s, None)
                .map(Address)
                .map_err(|_| {
                    EthError::Parse(format!("address has an invalid EIP-55 checksum: {s}"))
                })
        } else {
            hex.parse()
                .map(Address)
                .map_err(|e| EthError::Parse(format!("invalid address {s:?}: {e}")))
        }
    }
}

impl FromStr for Address {
    type Err = EthError;
    fn from_str(s: &str) -> Result<Address, EthError> {
        Address::parse(s)
    }
}

impl fmt::Display for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0.to_checksum(None))
    }
}

/// Accepted by generated contract constructors so both a parsed [`Address`]
/// and a literal `"0x…"` work. A malformed literal is a programmer error and
/// panics; runtime input should already arrive as an `Address`.
pub trait IntoAddress {
    fn into_address(self) -> Address;
}

impl IntoAddress for Address {
    fn into_address(self) -> Address {
        self
    }
}

impl IntoAddress for &str {
    fn into_address(self) -> Address {
        Address::parse(self).unwrap_or_else(|e| panic!("{e}"))
    }
}

impl IntoAddress for String {
    fn into_address(self) -> Address {
        self.as_str().into_address()
    }
}

/// An unsigned 256-bit integer, the native word of the EVM. Crosses the wire
/// as a 0x-prefixed hex string.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct U256(pub(crate) alloy::primitives::U256);

impl U256 {
    pub const ZERO: U256 = U256(alloy::primitives::U256::ZERO);

    pub fn from_dec(s: &str) -> Result<U256, EthError> {
        alloy::primitives::U256::from_str_radix(s, 10)
            .map(U256)
            .map_err(|e| EthError::Parse(format!("invalid decimal quantity {s:?}: {e}")))
    }

    pub fn from_hex(s: &str) -> Result<U256, EthError> {
        let hex = s.strip_prefix("0x").unwrap_or(s);
        if hex.is_empty() || hex.len() > 64 {
            return Err(EthError::Parse(format!(
                "hex quantity must be 1-64 hex characters, got {s:?}"
            )));
        }
        alloy::primitives::U256::from_str_radix(hex, 16)
            .map(U256)
            .map_err(|e| EthError::Parse(format!("invalid hex quantity {s:?}: {e}")))
    }

    pub fn to_u64(self) -> Option<u64> {
        u64::try_from(self.0).ok()
    }

    /// Decimal rendering, for humans; the wire and [`Display`](fmt::Display)
    /// form stays hex.
    pub fn to_dec(self) -> String {
        format!("{}", self.0)
    }

    pub fn checked_add(self, rhs: U256) -> Option<U256> {
        self.0.checked_add(rhs.0).map(U256)
    }

    pub fn checked_sub(self, rhs: U256) -> Option<U256> {
        self.0.checked_sub(rhs.0).map(U256)
    }

    pub fn checked_mul(self, rhs: U256) -> Option<U256> {
        self.0.checked_mul(rhs.0).map(U256)
    }

    pub fn checked_div(self, rhs: U256) -> Option<U256> {
        self.0.checked_div(rhs.0).map(U256)
    }
}

impl From<u64> for U256 {
    fn from(v: u64) -> U256 {
        U256(alloy::primitives::U256::from(v))
    }
}

impl From<u128> for U256 {
    fn from(v: u128) -> U256 {
        U256(alloy::primitives::U256::from(v))
    }
}

/// Accepts `0x`-prefixed hex or plain decimal.
impl FromStr for U256 {
    type Err = EthError;
    fn from_str(s: &str) -> Result<U256, EthError> {
        if s.starts_with("0x") {
            U256::from_hex(s)
        } else {
            U256::from_dec(s)
        }
    }
}

impl fmt::Display for U256 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "0x{:x}", self.0)
    }
}

macro_rules! u256_ops {
    ($($trait:ident::$method:ident),* $(,)?) => {
        $(
            impl std::ops::$trait for U256 {
                type Output = U256;
                fn $method(self, rhs: U256) -> U256 {
                    U256(std::ops::$trait::$method(self.0, rhs.0))
                }
            }
        )*
    };
}

u256_ops!(Add::add, Sub::sub, Mul::mul, Div::div, Rem::rem);

/// An amount of ether, denominated in wei. Same wire shape as [`U256`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Wei(pub U256);

impl Wei {
    pub const ZERO: Wei = Wei(U256::ZERO);

    pub fn from_wei(v: impl Into<U256>) -> Wei {
        Wei(v.into())
    }

    pub fn from_gwei(v: u64) -> Wei {
        Wei(U256::from(v) * U256::from(1_000_000_000u64))
    }

    /// Whole ether only, so no float rounding can corrupt an amount.
    pub fn from_eth(whole: u64) -> Wei {
        Wei(U256::from(whole) * U256::from(1_000_000_000_000_000_000u64))
    }

    pub fn as_u256(self) -> U256 {
        self.0
    }
}

impl From<U256> for Wei {
    fn from(v: U256) -> Wei {
        Wei(v)
    }
}

impl From<Wei> for U256 {
    fn from(v: Wei) -> U256 {
        v.0
    }
}

impl fmt::Display for Wei {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl AsSchema for Address {
    fn schema() -> Schema {
        hex_str_schema(ADDRESS_PATTERN)
    }
}

impl IntoValue for Address {
    fn into_value(self) -> Value {
        Value::Str(self.to_string())
    }
}

impl FromValue for Address {
    fn from_value(value: Value) -> Result<Self, HandlerError> {
        match value {
            Value::Str(s) => Address::parse(&s).map_err(|e| e.to_string().into()),
            other => Err(format!("expected an address string, got {}", other.kind()).into()),
        }
    }
}

impl AsSchema for U256 {
    fn schema() -> Schema {
        hex_str_schema(QUANTITY_PATTERN)
    }
}

impl IntoValue for U256 {
    fn into_value(self) -> Value {
        Value::Str(self.to_string())
    }
}

impl FromValue for U256 {
    fn from_value(value: Value) -> Result<Self, HandlerError> {
        match value {
            Value::Str(s) => U256::from_hex(&s).map_err(|e| e.to_string().into()),
            other => Err(format!("expected a hex quantity string, got {}", other.kind()).into()),
        }
    }
}

impl AsSchema for Wei {
    fn schema() -> Schema {
        U256::schema()
    }
}

impl IntoValue for Wei {
    fn into_value(self) -> Value {
        self.0.into_value()
    }
}

impl FromValue for Wei {
    fn from_value(value: Value) -> Result<Self, HandlerError> {
        U256::from_value(value).map(Wei)
    }
}

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

infallible_outputs!(Address, U256, Wei);

/// Hand-written equivalent of what #[derive(Schema)] emits, for record types
/// defined inside this crate (the derive's generated paths only resolve for
/// consumers of `lib`).
macro_rules! record_conversions {
    ($name:ident { $($field:ident: $ty:ty),* $(,)? }) => {
        impl AsSchema for $name {
            fn schema() -> Schema {
                Schema::Record(vec![
                    $((stringify!($field).to_string(), <$ty as AsSchema>::schema())),*
                ])
            }
        }

        impl IntoValue for $name {
            fn into_value(self) -> Value {
                Value::Map(vec![
                    $((
                        Value::Str(stringify!($field).to_string()),
                        self.$field.into_value(),
                    )),*
                ])
            }
        }

        impl FromValue for $name {
            fn from_value(value: Value) -> Result<Self, HandlerError> {
                let Value::Map(mut entries) = value else {
                    return Err("expected a record".into());
                };
                $(
                    let $field = {
                        let label = stringify!($field);
                        let index = entries
                            .iter()
                            .position(|(key, _)| {
                                matches!(key, Value::Str(name) if name == label)
                            })
                            .ok_or_else(|| {
                                HandlerError::from(format!("missing field {label:?}"))
                            })?;
                        let (_, value) = entries.swap_remove(index);
                        <$ty as FromValue>::from_value(value)?
                    };
                )*
                Ok($name { $($field),* })
            }
        }

        infallible_outputs!($name);
    };
}

/// A block header, as streamed by [`blocks`](super::blocks).
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub number: u64,
    pub hash: String,
    pub timestamp: u64,
    pub gas_used: u64,
    pub base_fee: Option<Wei>,
}

record_conversions!(Block {
    number: u64,
    hash: String,
    timestamp: u64,
    gas_used: u64,
    base_fee: Option<Wei>,
});

/// A mined transaction receipt, returned by
/// [`TxHandle::wait`](super::TxHandle::wait).
#[derive(Debug, Clone, PartialEq)]
pub struct Receipt {
    pub tx_hash: String,
    pub block_number: u64,
    pub status: bool,
    pub gas_used: u64,
}

record_conversions!(Receipt {
    tx_hash: String,
    block_number: u64,
    status: bool,
    gas_used: u64,
});

#[cfg(test)]
mod tests {
    use super::*;

    const VITALIK: &str = "0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045";

    #[test]
    fn addresses_parse_and_checksum() {
        let addr = Address::parse(VITALIK).unwrap();
        assert_eq!(addr.to_string(), VITALIK);

        let lower = Address::parse(&VITALIK.to_lowercase()).unwrap();
        assert_eq!(lower, addr);
        assert_eq!(lower.to_string(), VITALIK);
    }

    #[test]
    fn mixed_case_addresses_must_pass_eip55() {
        let bad = "0xD8dA6BF26964aF9D7eEd9e03E53415D37aA96045"; // first letter flipped
        assert!(Address::parse(bad).is_err());
    }

    #[test]
    fn malformed_addresses_are_rejected() {
        for bad in ["", "0x", "0x1234", "d8da6bf26964af9d7eed9e03e53415d37aa96045"] {
            assert!(Address::parse(bad).is_err(), "accepted {bad:?}");
        }
        let no_hex = format!("0x{}", "g".repeat(40));
        assert!(Address::parse(&no_hex).is_err());
    }

    #[test]
    fn addresses_use_pattern_constrained_str_schemas() {
        let Schema::Constrained(inner, constraints) = Address::schema() else {
            panic!("expected constrained schema");
        };
        assert_eq!(*inner, Schema::Primitive(Primitive::Str));
        assert_eq!(
            constraints,
            vec![Constraint::Pattern(ADDRESS_PATTERN.to_string())]
        );
    }

    #[test]
    fn addresses_roundtrip_through_values() {
        let addr = Address::parse(VITALIK).unwrap();
        assert_eq!(
            Address::from_value(addr.into_value()).unwrap(),
            addr
        );
        assert!(Address::from_value(Value::Str("0xnope".into())).is_err());
        assert!(Address::from_value(Value::I64(1)).is_err());
    }

    #[test]
    fn quantities_parse_dec_and_hex() {
        assert_eq!(U256::from_dec("1000").unwrap(), U256::from(1000u64));
        assert_eq!(U256::from_hex("0x3e8").unwrap(), U256::from(1000u64));
        assert_eq!("1000".parse::<U256>().unwrap(), U256::from(1000u64));
        assert_eq!("0x3e8".parse::<U256>().unwrap(), U256::from(1000u64));
        assert!(U256::from_hex("0x").is_err());
        assert!(U256::from_dec("abc").is_err());
        assert!(U256::from_hex(&format!("0x{}", "f".repeat(65))).is_err());
    }

    #[test]
    fn quantities_serialize_as_hex() {
        assert_eq!(U256::from(255u64).to_string(), "0xff");
        assert_eq!(U256::from(255u64).into_value(), Value::Str("0xff".into()));
        assert_eq!(
            U256::from_value(Value::Str("0xff".into())).unwrap(),
            U256::from(255u64)
        );
    }

    #[test]
    fn quantity_math_checks_overflow() {
        let max = U256::ZERO.checked_sub(U256::from(1u64));
        assert!(max.is_none());
        assert_eq!(
            U256::from(2u64).checked_mul(U256::from(3u64)),
            Some(U256::from(6u64))
        );
        assert_eq!(U256::from(2u64) + U256::from(3u64), U256::from(5u64));
    }

    #[test]
    fn wei_constructors_scale_correctly() {
        assert_eq!(Wei::from_gwei(1).as_u256(), U256::from(1_000_000_000u64));
        assert_eq!(
            Wei::from_eth(2).as_u256(),
            U256::from(2_000_000_000_000_000_000u64)
        );
        assert_eq!(Wei::from_wei(7u64).as_u256(), U256::from(7u64));
    }
}
