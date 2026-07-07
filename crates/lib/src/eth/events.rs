/* on-chain activity as plain blocking iterators — exactly what #[sse] and
#[live] handlers return. polling over http (from the head at stream start,
at-least-once, no reorg rewind) keeps v1 to a single transport */

use std::collections::VecDeque;
use std::marker::PhantomData;

use alloy::providers::Provider;
use alloy::rpc::types::Filter;

use super::abi::{self, AbiParam};
use super::client::{Eth, rpc_error};
use super::error::EthError;
use super::types::{Address, Block, Wei};
use crate::schema::IntoValue;

/// how many blocks one eth_getLogs may span, to stay under public-rpc limits
const MAX_RANGE: u64 = 2000;

/// Implemented by the event structs #[contract] generates from an abi.
pub trait ContractEvent: Sized + IntoValue {
    /// Canonical signature, e.g. `"Transfer(address,address,uint256)"`.
    const SIGNATURE: &'static str;
    /// Each field's canonical type and whether it is indexed, in order.
    const FIELDS: &'static [(&'static str, bool)];

    fn from_params(params: Vec<AbiParam>) -> Result<Self, EthError>;
}

/// An endless iterator over a contract's events, starting at the chain head
/// when created. On rpc failure it logs and ends the stream; reconnecting
/// clients get a fresh stream from the new head.
pub struct EventStream<E: ContractEvent> {
    address: Address,
    topic0: alloy::primitives::B256,
    next_block: u64,
    buffer: VecDeque<E>,
    _marker: PhantomData<E>,
}

pub(crate) fn watch<E: ContractEvent>(address: Address) -> Result<EventStream<E>, EthError> {
    let eth = super::global::get();
    let head = eth.block_number()?;
    Ok(EventStream {
        address,
        topic0: alloy::primitives::B256::from(abi::event_topic(E::SIGNATURE)),
        next_block: head + 1,
        buffer: VecDeque::new(),
        _marker: PhantomData,
    })
}

impl<E: ContractEvent> EventStream<E> {
    /// Fetches the next unseen block range into the buffer. Ok(false) means
    /// the head has not advanced yet.
    fn poll(&mut self, eth: &Eth) -> Result<bool, EthError> {
        let head = eth.block_number()?;
        if head < self.next_block {
            return Ok(false);
        }
        let to = head.min(self.next_block + MAX_RANGE - 1);
        let filter = Filter::new()
            .address(self.address.0)
            .event_signature(self.topic0)
            .from_block(self.next_block)
            .to_block(to);
        let logs = eth
            .block_on(eth.provider().get_logs(&filter))
            .map_err(rpc_error)?;
        for log in logs {
            let topics: Vec<[u8; 32]> = log.topics().iter().map(|t| t.0).collect();
            let params = abi::decode_log(E::FIELDS, &topics, &log.data().data)?;
            self.buffer.push_back(E::from_params(params)?);
        }
        self.next_block = to + 1;
        Ok(!self.buffer.is_empty())
    }
}

impl<E: ContractEvent> Iterator for EventStream<E> {
    type Item = E;

    fn next(&mut self) -> Option<E> {
        loop {
            if let Some(event) = self.buffer.pop_front() {
                return Some(event);
            }
            let eth = super::global::get();
            match self.poll(eth) {
                Ok(true) => {}
                Ok(false) => std::thread::sleep(eth.poll_interval()),
                Err(e) => {
                    eprintln!("event stream ended: {e}");
                    return None;
                }
            }
        }
    }
}

/// An endless iterator over new chain heads, starting after the block at
/// stream creation. Same failure behavior as [`EventStream`].
pub struct BlockStream {
    next: u64,
}

pub(crate) fn blocks() -> Result<BlockStream, EthError> {
    let head = super::global::get().block_number()?;
    Ok(BlockStream { next: head + 1 })
}

impl BlockStream {
    fn poll(&mut self, eth: &Eth) -> Result<Option<Block>, EthError> {
        let number = self.next;
        let fetched = eth
            .block_on(async { eth.provider().get_block_by_number(number.into()).await })
            .map_err(rpc_error)?;
        let Some(block) = fetched else {
            return Ok(None);
        };
        self.next += 1;
        Ok(Some(Block {
            number: block.header.number,
            hash: format!("{}", block.header.hash),
            timestamp: block.header.timestamp,
            gas_used: block.header.gas_used,
            base_fee: block
                .header
                .base_fee_per_gas
                .map(|fee| Wei::from_wei(u64::from(fee))),
        }))
    }
}

impl Iterator for BlockStream {
    type Item = Block;

    fn next(&mut self) -> Option<Block> {
        loop {
            let eth = super::global::get();
            match self.poll(eth) {
                Ok(Some(block)) => return Some(block),
                Ok(None) => std::thread::sleep(eth.poll_interval()),
                Err(e) => {
                    eprintln!("block stream ended: {e}");
                    return None;
                }
            }
        }
    }
}
