/* transaction sending: fill (nonce, gas, fees) → sign → broadcast, with a
per-sender lock so concurrent handlers cannot race nonces */

use std::time::{Duration, Instant};

use alloy::network::TransactionBuilder;
use alloy::providers::Provider;
use alloy::rpc::types::TransactionRequest;

use super::abi::AbiParam;
use super::client::rpc_error;
use super::error::EthError;
use super::signer::Signer;
use super::types::{Address, Receipt, Wei};
use crate::schema::{AsSchema, Constraint, FromValue, IntoValue, Primitive, Schema, Value};
use crate::server::endpoint::{HandlerError, IntoHandlerOutput};

const WAIT_TIMEOUT: Duration = Duration::from_secs(600);
const TX_HASH_PATTERN: &str = "^0x[0-9a-fA-F]{64}$";

/// A filled, ready-to-sign transaction handed to a [`Signer`]. Opaque so
/// alloy stays out of the public surface.
pub struct TxRequest(pub(crate) TransactionRequest);

pub(crate) async fn send(
    to: Address,
    signature: &str,
    args: Vec<AbiParam>,
    value: Wei,
    signer: Option<Box<dyn Signer>>,
) -> Result<TxHandle, EthError> {
    let Some(signer) = signer else {
        return Err(EthError::Tx(
            "transaction needs a signer — name one with .from(eth::treasury())".into(),
        ));
    };
    signer.ready()?;
    let eth = super::global::get();
    let sender = signer.address();
    let data = super::abi::encode_call(signature, args)?;

    // hold the per-sender lock through the whole fill → sign → send window;
    // two handlers filling in parallel would otherwise pick the same nonce
    let _guard = eth.sender_lock(sender).await;

    let provider = eth.provider();
    let nonce = provider
        .get_transaction_count(sender.0)
        .pending()
        .await
        .map_err(rpc_error)?;

    let mut request = TransactionRequest::default()
        .with_from(sender.0)
        .with_to(to.0)
        .with_input(data)
        .with_value(value.as_u256().0)
        .with_nonce(nonce)
        .with_chain_id(eth.chain_id());

    // estimate before fees: reverts surface here with the node's reason
    let gas = provider
        .estimate_gas(request.clone())
        .await
        .map_err(|e| EthError::Tx(format!("gas estimation failed: {e}")))?;
    request.set_gas_limit(gas);

    match provider.estimate_eip1559_fees().await {
        Ok(fees) => {
            request.set_max_fee_per_gas(fees.max_fee_per_gas);
            request.set_max_priority_fee_per_gas(fees.max_priority_fee_per_gas);
        }
        // chains without eip-1559 still take legacy gas-priced transactions
        Err(_) => {
            let price = provider.get_gas_price().await.map_err(rpc_error)?;
            request.set_gas_price(price);
        }
    }

    let raw = signer.sign(&TxRequest(request))?;
    let pending = provider
        .send_raw_transaction(&raw)
        .await
        .map_err(|e| EthError::Tx(format!("broadcast failed: {e}")))?;
    Ok(TxHandle {
        hash: *pending.tx_hash(),
    })
}

/// A broadcast transaction. [`hash`](TxHandle::hash) is available
/// immediately for optimistic UIs; [`wait`](TxHandle::wait) polls until the
/// receipt has the requested depth.
#[derive(Debug, Clone, PartialEq)]
pub struct TxHandle {
    hash: alloy::primitives::B256,
}

impl TxHandle {
    pub fn hash(&self) -> String {
        format!("{}", self.hash)
    }

    pub fn wait(&self, confirmations: u64) -> Result<Receipt, EthError> {
        let eth = super::global::get();
        eth.block_on(self.wait_async(confirmations))
    }

    pub async fn wait_async(&self, confirmations: u64) -> Result<Receipt, EthError> {
        let eth = super::global::get();
        let started = Instant::now();
        let receipt = loop {
            if let Some(receipt) = eth
                .provider()
                .get_transaction_receipt(self.hash)
                .await
                .map_err(rpc_error)?
            {
                break receipt;
            }
            if started.elapsed() > WAIT_TIMEOUT {
                return Err(EthError::Tx(format!(
                    "transaction {} not mined within {}s",
                    self.hash(),
                    WAIT_TIMEOUT.as_secs()
                )));
            }
            tokio::time::sleep(eth.poll_interval()).await;
        };

        let mined_in = receipt.block_number.ok_or_else(|| {
            EthError::Tx(format!("receipt for {} has no block number", self.hash()))
        })?;
        while confirmations > 1 {
            let head = eth.block_number_async().await?;
            if head + 1 >= mined_in + confirmations {
                break;
            }
            if started.elapsed() > WAIT_TIMEOUT {
                return Err(EthError::Tx(format!(
                    "transaction {} not confirmed within {}s",
                    self.hash(),
                    WAIT_TIMEOUT.as_secs()
                )));
            }
            tokio::time::sleep(eth.poll_interval()).await;
        }

        Ok(Receipt {
            tx_hash: self.hash(),
            block_number: mined_in,
            status: receipt.status(),
            gas_used: receipt.gas_used as u64,
        })
    }
}

impl AsSchema for TxHandle {
    fn schema() -> Schema {
        Schema::Record(vec![(
            "hash".to_string(),
            Schema::Constrained(
                Box::new(Schema::Primitive(Primitive::Str)),
                vec![Constraint::Pattern(TX_HASH_PATTERN.to_string())],
            ),
        )])
    }
}

impl IntoValue for TxHandle {
    fn into_value(self) -> Value {
        Value::Map(vec![(
            Value::Str("hash".to_string()),
            Value::Str(self.hash()),
        )])
    }
}

impl FromValue for TxHandle {
    fn from_value(value: Value) -> Result<Self, HandlerError> {
        let Value::Map(entries) = value else {
            return Err("expected a record".into());
        };
        let hash = entries
            .into_iter()
            .find_map(|(key, value)| match (key, value) {
                (Value::Str(name), Value::Str(hash)) if name == "hash" => Some(hash),
                _ => None,
            })
            .ok_or_else(|| HandlerError::from("missing field \"hash\""))?;
        let hash = hash
            .parse()
            .map_err(|e| HandlerError::from(format!("invalid transaction hash: {e}")))?;
        Ok(TxHandle { hash })
    }
}

impl IntoHandlerOutput for TxHandle {
    type Ok = Self;
    fn into_handler_output(self) -> Result<Value, HandlerError> {
        Ok(self.into_value())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_roundtrip_through_values() {
        let handle = TxHandle {
            hash: alloy::primitives::B256::repeat_byte(0xab),
        };
        assert_eq!(handle.hash(), format!("0x{}", "ab".repeat(32)));
        assert_eq!(
            TxHandle::from_value(handle.clone().into_value()).unwrap(),
            handle
        );
        assert!(TxHandle::from_value(Value::Str("0xab".into())).is_err());
    }
}
