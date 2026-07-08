//! Compile-level coverage of #[contract]: the macro reads the fixture abi
//! and must generate methods, builders and event structs with the shapes the
//! docs promise. Network behavior is covered by the in-crate mock-rpc tests.
#![cfg(feature = "eth")]

use lib::eth::{Address, CallBuilder, ContractEvent, EthError, TxBuilder, U256};
use lib::prelude::*;
use lib::schema::{Constraint, Primitive, Schema};

#[contract("tests/fixtures/erc20.json")]
struct Erc20;

const USDC: &str = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";

#[test]
fn at_binds_literals_and_addresses() {
    let token = Erc20::at(USDC);
    assert_eq!(token.address(), Address::parse(USDC).unwrap());

    let same = Erc20::at(token.address());
    assert_eq!(same.address(), token.address());

    assert!(Erc20::try_at("0xnot-an-address").is_err());
    assert!(Erc20::try_at(USDC).is_ok());
}

#[test]
fn views_become_call_builders_and_writes_tx_builders() {
    let token = Erc20::at(USDC);
    let holder = Address::parse("0xd8dA6BF26964aF9D7eEd9e03E53415D37aA96045").unwrap();

    // typed builders: the compiler checks argument and return types
    let _balance: CallBuilder<U256> = token.balance_of(holder);
    let _decimals: CallBuilder<u32> = token.decimals();
    let _symbol: CallBuilder<String> = token.symbol();
    let _supply: CallBuilder<U256> = token.total_supply();
    let _approve: TxBuilder = token.approve(holder, U256::from(1u64));

    // calldata for client-side signing: selector + padded args, no network
    let transfer: TxBuilder = token.transfer(holder, U256::from(500u64));
    assert_eq!(transfer.to(), token.address());
    let calldata = transfer.calldata().unwrap();
    assert!(calldata.starts_with("0xa9059cbb"), "{calldata}");
    assert_eq!(calldata.len(), 2 + 8 + 64 + 64);
}

#[test]
fn events_carry_canonical_signatures_and_schemas() {
    assert_eq!(Transfer::SIGNATURE, "Transfer(address,address,uint256)");
    assert_eq!(
        Transfer::FIELDS,
        &[("address", true), ("address", true), ("uint256", false)]
    );
    assert_eq!(Approval::SIGNATURE, "Approval(address,address,uint256)");

    let Schema::Record(fields) = Transfer::schema() else {
        panic!("expected a record schema");
    };
    let labels: Vec<&str> = fields.iter().map(|(label, _)| label.as_str()).collect();
    assert_eq!(labels, ["from", "to", "value"]);
    for (_, field) in &fields {
        let Schema::Constrained(inner, constraints) = field else {
            panic!("expected wire-validated hex strings");
        };
        assert_eq!(**inner, Schema::Primitive(Primitive::Str));
        assert!(matches!(constraints.as_slice(), [Constraint::Pattern(_)]));
    }
}

#[test]
fn events_decode_from_abi_params() {
    let from = Address::parse(USDC).unwrap();
    let to = Address::parse("0x0000000000000000000000000000000000000001").unwrap();
    let event = Transfer::from_params(vec![
        lib::eth::abi::AbiParam::Address(from),
        lib::eth::abi::AbiParam::Address(to),
        lib::eth::abi::AbiParam::Uint(U256::from(42u64)),
    ])
    .unwrap();
    assert_eq!(event.from, from);
    assert_eq!(event.to, to);
    assert_eq!(event.value, U256::from(42u64));

    let short: Result<Transfer, EthError> = Transfer::from_params(vec![]);
    assert!(short.is_err());
}
