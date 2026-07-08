/* runtime targets of #[contract]-generated code: reads become CallBuilders,
writes become TxBuilders that must name a signer */

use std::marker::PhantomData;

use alloy::network::TransactionBuilder;
use alloy::providers::Provider;
use alloy::rpc::types::TransactionRequest;

use super::abi::{self, AbiParam, AbiType};
use super::client::rpc_error;
use super::error::EthError;
use super::signer::Signer;
use super::tx::{self, TxHandle};
use super::types::{Address, Wei};

/// A prepared read-only contract call. Terminal methods: [`call`]
/// (blocking, for handler threads) and [`call_async`].
///
/// [`call`]: CallBuilder::call
/// [`call_async`]: CallBuilder::call_async
#[must_use = "a contract call does nothing until .call() is invoked"]
pub struct CallBuilder<T: AbiType> {
    address: Address,
    signature: &'static str,
    output: &'static str,
    args: Vec<AbiParam>,
    _marker: PhantomData<T>,
}

impl<T: AbiType> CallBuilder<T> {
    #[doc(hidden)]
    pub fn new(
        address: Address,
        signature: &'static str,
        output: &'static str,
        args: Vec<AbiParam>,
    ) -> CallBuilder<T> {
        CallBuilder {
            address,
            signature,
            output,
            args,
            _marker: PhantomData,
        }
    }

    pub fn call(self) -> Result<T, EthError> {
        let eth = super::global::get();
        eth.block_on(self.call_async())
    }

    pub async fn call_async(self) -> Result<T, EthError> {
        let eth = super::global::get();
        let data = abi::encode_call(self.signature, self.args)?;
        let request = TransactionRequest::default()
            .with_to(self.address.0)
            .with_input(data);
        let returned = eth.provider().call(request).await.map_err(rpc_error)?;
        if self.output.is_empty() {
            return T::from_abi(AbiParam::Unit);
        }
        // a non-reverting call that returns no data means the address holds no
        // contract on this chain — decoding it would surface as an opaque
        // "buffer overrun", so name the real cause instead
        if returned.is_empty() {
            return Err(EthError::Call(format!(
                "{} returned no data — there is no contract at that address on chain {} \
                 (is ETH_RPC_URL pointed at the right network?)",
                self.address,
                eth.chain_id(),
            )));
        }
        T::from_abi(abi::decode_return(self.output, &returned)?)
    }
}

/// A prepared state-changing contract call. Requires a signer via
/// [`from`](TxBuilder::from) before [`send`](TxBuilder::send); payable
/// functions take ether through [`value`](TxBuilder::value).
#[must_use = "a transaction does nothing until .send() is invoked"]
pub struct TxBuilder {
    address: Address,
    signature: &'static str,
    args: Vec<AbiParam>,
    value: Wei,
    signer: Option<Box<dyn Signer>>,
}

impl TxBuilder {
    #[doc(hidden)]
    pub fn new(address: Address, signature: &'static str, args: Vec<AbiParam>) -> TxBuilder {
        TxBuilder {
            address,
            signature,
            args,
            value: Wei::ZERO,
            signer: None,
        }
    }

    pub fn from(mut self, signer: impl Signer + 'static) -> TxBuilder {
        self.signer = Some(Box::new(signer));
        self
    }

    /// The contract address this call targets.
    pub fn to(&self) -> Address {
        self.address
    }

    /// The abi-encoded calldata of this call as 0x-hex — for handing to a
    /// browser wallet (`eth_sendTransaction`) when the user signs
    /// client-side instead of the server.
    pub fn calldata(&self) -> Result<String, EthError> {
        let data = abi::encode_call(self.signature, self.args.clone())?;
        Ok(format!("0x{}", alloy::primitives::hex::encode(data)))
    }

    pub fn value(mut self, value: Wei) -> TxBuilder {
        self.value = value;
        self
    }

    pub fn send(self) -> Result<TxHandle, EthError> {
        let eth = super::global::get();
        eth.block_on(self.send_async())
    }

    pub async fn send_async(self) -> Result<TxHandle, EthError> {
        tx::send(
            self.address,
            self.signature,
            self.args,
            self.value,
            self.signer,
        )
        .await
    }
}
