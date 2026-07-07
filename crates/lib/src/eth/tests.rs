/* integration tests against an in-process mock json-rpc server. one server +
one global init are shared by every test in this module (the global client is
a process-wide OnceLock, like the database) */

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use axum::Json;
use axum::extract::State;
use serde_json::{Value as Json_, json};

use super::abi::{AbiParam, AbiType};
use super::contract::{CallBuilder, TxBuilder};
use super::events::ContractEvent;
use super::types::{Address, U256, Wei};
use super::{Config, Eth};
use crate::schema::IntoValue;

// hardhat's well-known first dev key — never fund this account
const TREASURY_KEY: &str = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
const TREASURY_ADDR: &str = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";
const TOKEN: &str = "0x0000000000000000000000000000000000000002";
const TX_HASH: &str = "0x1111111111111111111111111111111111111111111111111111111111111111";

#[derive(Clone, Default)]
struct MockState {
    block: Arc<AtomicU64>,
    calls: Arc<Mutex<Vec<String>>>,
    raw_txs: Arc<Mutex<Vec<String>>>,
    log_polls: Arc<AtomicU64>,
}

fn hex_u64(value: &Json_) -> u64 {
    u64::from_str_radix(
        value.as_str().unwrap_or_default().trim_start_matches("0x"),
        16,
    )
    .unwrap_or_default()
}

fn pad_address(address: &str) -> String {
    format!("0x{}{}", "0".repeat(24), address.trim_start_matches("0x"))
}

async fn rpc(State(state): State<MockState>, Json(request): Json<Json_>) -> Json<Json_> {
    let method = request["method"].as_str().unwrap_or_default();
    let id = request["id"].clone();
    let params = &request["params"];

    let result = match method {
        "eth_chainId" => json!("0x7a69"),               // 31337
        "eth_getBalance" => json!("0xde0b6b3a7640000"), // 1 ether
        "eth_gasPrice" => json!("0x3b9aca00"),          // 1 gwei
        "eth_getTransactionCount" => json!("0x5"),
        "eth_estimateGas" => json!("0x5208"),
        "eth_blockNumber" => {
            let head = 100 + state.block.fetch_add(1, Ordering::SeqCst);
            json!(format!("0x{head:x}"))
        }
        "eth_call" => {
            let data = params[0]["input"]
                .as_str()
                .or_else(|| params[0]["data"].as_str())
                .unwrap_or_default()
                .to_string();
            state.calls.lock().unwrap().push(data);
            // uint256 1000
            json!(format!("0x{:0>64}", "3e8"))
        }
        "eth_sendRawTransaction" => {
            let raw = params[0].as_str().unwrap_or_default().to_string();
            state.raw_txs.lock().unwrap().push(raw);
            json!(TX_HASH)
        }
        "eth_getTransactionReceipt" => json!({
            "transactionHash": TX_HASH,
            "transactionIndex": "0x0",
            "blockHash": format!("0x{}", "22".repeat(32)),
            "blockNumber": "0x65",
            "from": TREASURY_ADDR,
            "to": TOKEN,
            "cumulativeGasUsed": "0x5208",
            "gasUsed": "0x5208",
            "contractAddress": null,
            "logs": [],
            "logsBloom": format!("0x{}", "0".repeat(512)),
            "type": "0x0",
            "status": "0x1",
            "effectiveGasPrice": "0x3b9aca00",
        }),
        "eth_getLogs" => {
            if state.log_polls.fetch_add(1, Ordering::SeqCst) == 0 {
                let amount = format!("0x{:0>64}", "1f4"); // uint256 500
                json!([{
                    "address": TOKEN,
                    "topics": [
                        format!("0x{}", alloy::primitives::hex::encode(
                            super::abi::event_topic("Transfer(address,address,uint256)"),
                        )),
                        pad_address(TREASURY_ADDR),
                        pad_address("0x0000000000000000000000000000000000000001"),
                    ],
                    "data": amount,
                    "blockNumber": "0x65",
                    "transactionHash": TX_HASH,
                    "transactionIndex": "0x0",
                    "blockHash": format!("0x{}", "22".repeat(32)),
                    "logIndex": "0x0",
                    "removed": false,
                }])
            } else {
                json!([])
            }
        }
        "eth_getBlockByNumber" => {
            let number = hex_u64(&params[0]);
            json!({
                "hash": format!("0x{}", "33".repeat(32)),
                "parentHash": format!("0x{}", "44".repeat(32)),
                "sha3Uncles": format!("0x{}", "55".repeat(32)),
                "miner": "0x0000000000000000000000000000000000000000",
                "stateRoot": format!("0x{}", "66".repeat(32)),
                "transactionsRoot": format!("0x{}", "77".repeat(32)),
                "receiptsRoot": format!("0x{}", "88".repeat(32)),
                "logsBloom": format!("0x{}", "0".repeat(512)),
                "difficulty": "0x0",
                "number": format!("0x{number:x}"),
                "gasLimit": "0x1c9c380",
                "gasUsed": "0xabcd",
                "timestamp": "0x684d92c8",
                "extraData": "0x",
                "mixHash": format!("0x{}", "99".repeat(32)),
                "nonce": "0x0000000000000000",
                "baseFeePerGas": "0x3b9aca00",
                "transactions": [],
                "uncles": [],
            })
        }
        // no eth_feeHistory: exercises the legacy gas-price fallback
        _ => {
            return Json(json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": format!("{method} not mocked") },
            }));
        }
    };
    Json(json!({ "jsonrpc": "2.0", "id": id, "result": result }))
}

static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
static MOCK: OnceLock<MockState> = OnceLock::new();

fn runtime() -> &'static tokio::runtime::Runtime {
    RUNTIME.get_or_init(|| tokio::runtime::Runtime::new().expect("test runtime"))
}

async fn spawn_mock(state: MockState) -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let router = axum::Router::new()
        .route("/", axum::routing::post(rpc))
        .with_state(state);
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    format!("http://{addr}")
}

/// Starts the mock server and initializes the global client exactly once;
/// every test shares them.
fn setup() -> &'static MockState {
    MOCK.get_or_init(|| {
        let state = MockState::default();
        let server_state = state.clone();
        runtime().block_on(async {
            let url = spawn_mock(server_state).await;
            let mut config = Config::from_rpc(url);
            config.chain_id = Some(31337);
            config.treasury_key = Some(TREASURY_KEY.to_string());
            config.poll_interval = Duration::from_millis(10);
            super::init(&config).await.expect("init against mock");
        });
        state
    })
}

#[test]
fn connect_rejects_a_chain_id_mismatch() {
    setup();
    // a second server so the mismatch doesn't fight the shared global
    let url = runtime().block_on(spawn_mock(MockState::default()));
    let mut config = Config::from_rpc(url);
    config.chain_id = Some(1);
    let error = runtime()
        .block_on(Eth::connect(&config))
        .expect_err("chain 31337 should not pass as mainnet");
    assert!(error.to_string().contains("chain id mismatch"), "{error}");
}

#[test]
fn global_reads_hit_the_rpc() {
    setup();
    let holder = Address::parse(TREASURY_ADDR).unwrap();
    assert_eq!(super::balance(holder).unwrap(), Wei::from_eth(1));
    assert_eq!(super::gas_price().unwrap(), Wei::from_gwei(1));
    assert!(super::block_number().unwrap() >= 100);
    assert_eq!(super::get().chain_id(), 31337);
    assert_eq!(super::treasury().address(), holder);
}

#[test]
fn contract_calls_encode_and_decode() {
    let state = setup();
    let holder = Address::parse(TREASURY_ADDR).unwrap();
    let token = Address::parse(TOKEN).unwrap();

    let balance: U256 = CallBuilder::new(
        token,
        "balanceOf(address)",
        "uint256",
        vec![holder.into_abi()],
    )
    .call()
    .unwrap();
    assert_eq!(balance, U256::from(1000u64));

    let data = state.calls.lock().unwrap().last().unwrap().clone();
    assert!(data.starts_with("0x70a08231"), "unexpected calldata {data}");
    assert!(data.ends_with(&TREASURY_ADDR[2..].to_lowercase()));
}

#[test]
fn transactions_fill_sign_and_broadcast_as_the_signer() {
    let state = setup();
    let token = Address::parse(TOKEN).unwrap();
    let to = Address::parse("0x0000000000000000000000000000000000000001").unwrap();

    let handle = TxBuilder::new(
        token,
        "transfer(address,uint256)",
        vec![to.into_abi(), U256::from(500u64).into_abi()],
    )
    .from(super::treasury())
    .send()
    .unwrap();
    assert_eq!(handle.hash(), TX_HASH);

    let raw = state.raw_txs.lock().unwrap().last().unwrap().clone();
    let bytes = alloy::primitives::hex::decode(&raw).unwrap();
    use alloy::consensus::transaction::SignerRecoverable;
    use alloy::eips::eip2718::Decodable2718;
    let envelope = alloy::consensus::TxEnvelope::decode_2718(&mut bytes.as_slice()).unwrap();
    let sender = envelope.recover_signer().unwrap();
    assert_eq!(Address(sender), Address::parse(TREASURY_ADDR).unwrap());

    let receipt = handle.wait(1).unwrap();
    assert_eq!(receipt.block_number, 101);
    assert!(receipt.status);
    assert_eq!(receipt.tx_hash, TX_HASH);
}

#[test]
fn transactions_without_a_signer_explain_the_fix() {
    setup();
    let token = Address::parse(TOKEN).unwrap();
    let error = TxBuilder::new(token, "transfer(address,uint256)", vec![])
        .send()
        .unwrap_err();
    assert!(error.to_string().contains(".from("), "{error}");
}

/// What #[contract] generates for an abi event, written out by hand since
/// the macro's `::lib::` paths only resolve outside this crate.
#[derive(Debug, Clone, PartialEq)]
struct Transfer {
    from: Address,
    to: Address,
    value: U256,
}

impl crate::schema::AsSchema for Transfer {
    fn schema() -> crate::schema::Schema {
        crate::schema::Schema::Record(vec![])
    }
}

impl IntoValue for Transfer {
    fn into_value(self) -> crate::schema::Value {
        crate::schema::Value::Map(vec![])
    }
}

impl ContractEvent for Transfer {
    const SIGNATURE: &'static str = "Transfer(address,address,uint256)";
    const FIELDS: &'static [(&'static str, bool)] =
        &[("address", true), ("address", true), ("uint256", false)];

    fn from_params(params: Vec<AbiParam>) -> Result<Self, super::EthError> {
        let mut params = params.into_iter();
        Ok(Transfer {
            from: AbiType::from_abi(params.next().unwrap())?,
            to: AbiType::from_abi(params.next().unwrap())?,
            value: AbiType::from_abi(params.next().unwrap())?,
        })
    }
}

#[test]
fn event_streams_decode_new_logs() {
    setup();
    let token = Address::parse(TOKEN).unwrap();
    let mut stream = super::events::watch::<Transfer>(token).unwrap();
    let event = stream.next().expect("one mocked transfer");
    assert_eq!(
        event,
        Transfer {
            from: Address::parse(TREASURY_ADDR).unwrap(),
            to: Address::parse("0x0000000000000000000000000000000000000001").unwrap(),
            value: U256::from(500u64),
        }
    );
}

#[test]
fn block_streams_follow_the_head() {
    setup();
    let mut stream = super::blocks().unwrap();
    let block = stream.next().expect("next block");
    assert!(block.number > 100);
    assert_eq!(block.gas_used, 0xabcd);
    assert_eq!(block.base_fee, Some(Wei::from_gwei(1)));
}

/// Read-only sanity check against a real node, for manual runs:
/// `LOOP_TEST_ETH_RPC=https://… cargo test -p lib --features eth -- --ignored`
#[test]
#[ignore = "live network test; set LOOP_TEST_ETH_RPC to run"]
fn live_rpc_reads() {
    let url = std::env::var("LOOP_TEST_ETH_RPC").expect("LOOP_TEST_ETH_RPC not set");
    let eth = runtime()
        .block_on(Eth::connect(&Config::from_rpc(url)))
        .expect("connect to live rpc");
    assert!(eth.block_number().expect("head") > 0);
    assert!(eth.gas_price().expect("gas price") > Wei::ZERO);
}
