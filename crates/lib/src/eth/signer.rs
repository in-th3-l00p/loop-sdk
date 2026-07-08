use std::fmt;

use alloy::consensus::transaction::SignableTransaction;
use alloy::eips::eip2718::Encodable2718;
use alloy::network::{Ethereum, NetworkTransactionBuilder, TxSignerSync};
use alloy::signers::local::PrivateKeySigner;

use super::error::EthError;
use super::tx::TxRequest;
use super::types::Address;

/// Something that can authorize a transaction. Shipped by [`Treasury`] (the
/// app's own wallet); the auth module's user wallets implement the same
/// trait once they land, so handlers can take the rail decision at runtime.
///
/// Signing is synchronous because server-held keys sign locally; a deferred
/// variant for wallets that round-trip to a client can be added as a
/// defaulted method without breaking implementors.
pub trait Signer: Send + Sync {
    fn address(&self) -> Address;

    /// Signs a fully-filled transaction, returning the raw bytes for
    /// `eth_sendRawTransaction`.
    fn sign(&self, tx: &TxRequest) -> Result<Vec<u8>, EthError>;

    /// Whether this signer can sign at all, checked before any rpc work —
    /// a self-custodial wallet refuses here rather than after gas
    /// estimation has already run.
    fn ready(&self) -> Result<(), EthError> {
        Ok(())
    }
}

/// Lets `.from(eth::treasury())` work directly with the global accessor.
impl<T: Signer> Signer for &'static T {
    fn address(&self) -> Address {
        (**self).address()
    }
    fn sign(&self, tx: &TxRequest) -> Result<Vec<u8>, EthError> {
        (**self).sign(tx)
    }
    fn ready(&self) -> Result<(), EthError> {
        (**self).ready()
    }
}

/// The app's own wallet, configured via `[eth.treasury]` in loop.toml and
/// held by the global client. Signs on the server without user interaction.
#[derive(Clone)]
pub struct Treasury {
    signer: PrivateKeySigner,
}

impl Treasury {
    pub fn from_key(hex: &str) -> Result<Treasury, EthError> {
        hex.parse()
            .map(|signer| Treasury { signer })
            .map_err(|e| EthError::Parse(format!("invalid treasury key: {e}")))
    }

    pub fn address(&self) -> Address {
        Address(self.signer.address())
    }

    /// Signs an EIP-191 personal message, returning the 65-byte signature.
    pub fn sign_message(&self, message: &[u8]) -> Result<Vec<u8>, EthError> {
        use alloy::signers::SignerSync;
        self.signer
            .sign_message_sync(message)
            .map(|signature| signature.as_bytes().to_vec())
            .map_err(|e| EthError::Tx(format!("message signing failed: {e}")))
    }
}

impl Signer for Treasury {
    fn address(&self) -> Address {
        Treasury::address(self)
    }

    fn sign(&self, tx: &TxRequest) -> Result<Vec<u8>, EthError> {
        sign_request(&self.signer, tx)
    }
}

/// Fill-agnostic local signing: typed tx from the filled request, secp256k1
/// signature, eip-2718 raw bytes. Shared by the treasury and by the auth
/// module's embedded wallets.
pub(crate) fn sign_request(
    signer: &PrivateKeySigner,
    tx: &TxRequest,
) -> Result<Vec<u8>, EthError> {
    let mut unsigned = NetworkTransactionBuilder::<Ethereum>::build_unsigned(tx.0.clone())
        .map_err(|e| EthError::Tx(format!("incomplete transaction: {e}")))?;
    let signature = signer
        .sign_transaction_sync(&mut unsigned)
        .map_err(|e| EthError::Tx(format!("signing failed: {e}")))?;
    let envelope: alloy::consensus::TxEnvelope = unsigned.into_signed(signature).into();
    Ok(envelope.encoded_2718())
}

// never expose the private key, not even in debug output
impl fmt::Debug for Treasury {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Treasury")
            .field("address", &self.address())
            .finish_non_exhaustive()
    }
}

/// Generates a fresh keypair, for `loop eth wallet new`. Returns the
/// 0x-hex private key and its address.
pub fn generate_key() -> (String, Address) {
    let signer = PrivateKeySigner::random();
    let key = format!("0x{:x}", signer.to_bytes());
    (key, Address(signer.address()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_keys_roundtrip_through_from_key() {
        let (key, address) = generate_key();
        assert!(key.starts_with("0x") && key.len() == 66);
        let treasury = Treasury::from_key(&key).unwrap();
        assert_eq!(treasury.address(), address);
    }

    #[test]
    fn bad_keys_are_rejected() {
        assert!(Treasury::from_key("0x1234").is_err());
        assert!(Treasury::from_key("not a key").is_err());
    }

    #[test]
    fn debug_output_never_leaks_the_key() {
        let (key, _) = generate_key();
        let treasury = Treasury::from_key(&key).unwrap();
        let debug = format!("{treasury:?}");
        assert!(!debug.contains(&key[2..]));
    }
}
