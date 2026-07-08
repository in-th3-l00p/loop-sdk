/* the wallet manager: what makes wallet auth symmetrical with email auth.
embedded wallets are created for email signups, their keys encrypted at rest
(per-user chacha20-poly1305 key derived from the app secret) and signing
server-side; linked wallets are siwe-proven and self-custodial — the server
never holds their key and says so when asked to sign */

use std::fmt;

use alloy::signers::SignerSync;
use alloy::signers::local::PrivateKeySigner;
use chacha20poly1305::aead::Aead;
use chacha20poly1305::{ChaCha20Poly1305, KeyInit, Nonce};
use hkdf::Hkdf;
use sha2::Sha256;

use super::error::AuthError;
use super::user::UserId;
use super::{crypto, store};
use crate::eth::{Address, EthError, Signer, TxRequest};

const KEY_CONTEXT: &[u8] = b"loop-auth-wallet";
const NONCE_LEN: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalletKind {
    /// Created and custodied by the app; signs server-side.
    Embedded,
    /// The user's own wallet, proven via sign-in-with-ethereum; the server
    /// never holds its key.
    Linked,
}

impl WalletKind {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            WalletKind::Embedded => "embedded",
            WalletKind::Linked => "linked",
        }
    }

    fn parse(s: &str) -> WalletKind {
        match s {
            "embedded" => WalletKind::Embedded,
            _ => WalletKind::Linked,
        }
    }
}

/// One of a user's wallets. Implements [`eth::Signer`](crate::eth::Signer),
/// so `TxBuilder::from(user.wallet())` works — embedded wallets sign
/// server-side, linked wallets refuse with a clear error.
#[derive(Clone)]
pub struct Wallet {
    address: Address,
    kind: WalletKind,
    user_id: String,
}

// key material only ever lives in short-lived locals, never in the struct
impl fmt::Debug for Wallet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Wallet")
            .field("address", &self.address)
            .field("kind", &self.kind)
            .finish_non_exhaustive()
    }
}

impl Wallet {
    pub fn address(&self) -> Address {
        self.address
    }

    pub fn kind(&self) -> WalletKind {
        self.kind
    }

    /// Signs an EIP-191 personal message, returning the 65-byte signature.
    /// Embedded wallets only — linked wallets sign client-side.
    pub fn sign_message(&self, message: &[u8]) -> Result<Vec<u8>, AuthError> {
        let signature = self
            .signer()?
            .sign_message_sync(message)
            .map_err(|e| AuthError::Crypto(format!("message signing failed: {e}")))?;
        Ok(signature.as_bytes().to_vec())
    }

    /// Hands the decrypted private key out as 0x-hex, for users ready to
    /// self-custody. A deliberate, auditable action — call it only from
    /// flows that just re-authenticated the user.
    pub fn export(&self) -> Result<String, AuthError> {
        let key = self.stored_key()?;
        Ok(format!("0x{}", key))
    }

    fn stored_key(&self) -> Result<String, AuthError> {
        if self.kind == WalletKind::Linked {
            return Err(AuthError::Unsupported(
                "this wallet is self-custodial — the server never holds its key".into(),
            ));
        }
        let row = store::wallet_by_address(&storage_address(self.address))?
            .ok_or_else(|| AuthError::Unavailable("wallet row disappeared".into()))?;
        let encrypted = row
            .encrypted_key
            .ok_or_else(|| AuthError::Unavailable("embedded wallet has no key".into()))?;
        decrypt_key(&self.user_id, &encrypted)
    }

    fn signer(&self) -> Result<PrivateKeySigner, AuthError> {
        self.stored_key()?
            .parse()
            .map_err(|e| AuthError::Crypto(format!("stored key is corrupt: {e}")))
    }
}

impl Signer for Wallet {
    fn address(&self) -> Address {
        self.address
    }

    fn sign(&self, tx: &TxRequest) -> Result<Vec<u8>, EthError> {
        match self.kind {
            WalletKind::Linked => Err(EthError::Tx(
                "this wallet is self-custodial: its key never leaves the user, so the \
                 transaction must be signed client-side. use an embedded wallet or \
                 eth::treasury() for server-sent transactions"
                    .into(),
            )),
            WalletKind::Embedded => {
                let signer = self.signer().map_err(|e| EthError::Tx(e.to_string()))?;
                crate::eth::sign_request(&signer, tx)
            }
        }
    }
}

/// Addresses are stored lowercase so lookups are case-insensitive.
pub(crate) fn storage_address(address: Address) -> String {
    address.to_string().to_lowercase()
}

/// Stored (lowercase) address back to its EIP-55 display form.
pub(crate) fn display_address(stored: &str) -> String {
    Address::parse(stored)
        .map(|a| a.to_string())
        .unwrap_or_else(|_| stored.to_string())
}

fn from_row(row: store::WalletRow) -> Wallet {
    Wallet {
        address: Address::parse(&row.address).expect("stored wallet addresses are valid"),
        kind: WalletKind::parse(&row.kind),
        user_id: row.user_id,
    }
}

/// A user's wallets, oldest (primary) first. Storage failures are
/// programmer/deployment errors and panic like an uninitialized global.
pub(crate) fn wallets_of(user_id: &UserId) -> Vec<Wallet> {
    store::wallets_of(user_id.as_str())
        .expect("wallet lookup failed")
        .into_iter()
        .map(from_row)
        .collect()
}

/// Creates the server-custodied wallet an email signup gets.
pub(crate) fn create_embedded(user_id: &str) -> Result<Wallet, AuthError> {
    let (key_hex, address) = crate::eth::generate_key();
    let encrypted = encrypt_key(user_id, key_hex.trim_start_matches("0x"))?;
    store::insert_wallet(
        &storage_address(address),
        user_id,
        WalletKind::Embedded.as_str(),
        Some(&encrypted),
    )?;
    Ok(Wallet {
        address,
        kind: WalletKind::Embedded,
        user_id: user_id.to_string(),
    })
}

/// Binds a proven external wallet to a user. Idempotent for the same user;
/// a wallet bound to someone else is a conflict.
pub(crate) fn link(user_id: &UserId, address: Address) -> Result<(), AuthError> {
    if let Some(existing) = store::wallet_by_address(&storage_address(address))? {
        if existing.user_id == user_id.as_str() {
            return Ok(());
        }
        return Err(AuthError::Conflict(format!(
            "wallet {address} is already linked to another account"
        )));
    }
    store::insert_wallet(
        &storage_address(address),
        user_id.as_str(),
        WalletKind::Linked.as_str(),
        None,
    )
}

fn master_secret() -> Result<[u8; 32], AuthError> {
    let secret = super::global::get()
        .config()
        .secret
        .as_deref()
        .ok_or_else(|| {
            AuthError::Config(
                "embedded wallets need a secret — generate one with `loop auth secret new`".into(),
            )
        })?;
    let bytes = crypto::from_hex(secret)?;
    bytes.try_into().map_err(|_| {
        AuthError::Config("auth secret must be exactly 32 bytes of hex".into())
    })
}

/// Per-user encryption key: HKDF-SHA256(master, user id). A leaked row can
/// only be opened with the app secret AND the owning user's id.
fn user_key(user_id: &str) -> Result<ChaCha20Poly1305, AuthError> {
    let master = master_secret()?;
    let hk = Hkdf::<Sha256>::new(Some(KEY_CONTEXT), &master);
    let mut okm = [0u8; 32];
    hk.expand(user_id.as_bytes(), &mut okm)
        .map_err(|e| AuthError::Crypto(format!("key derivation failed: {e}")))?;
    Ok(ChaCha20Poly1305::new(&okm.into()))
}

/// hex(nonce12 || ciphertext) — nonce fresh per encryption.
pub(crate) fn encrypt_key(user_id: &str, key_hex: &str) -> Result<String, AuthError> {
    let cipher = user_key(user_id)?;
    let nonce_bytes = crypto::random_bytes(NONCE_LEN);
    let nonce = Nonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, key_hex.as_bytes())
        .map_err(|e| AuthError::Crypto(format!("key encryption failed: {e}")))?;
    let mut stored = nonce_bytes;
    stored.extend(ciphertext);
    Ok(crypto::to_hex(&stored))
}

pub(crate) fn decrypt_key(user_id: &str, stored: &str) -> Result<String, AuthError> {
    let bytes = crypto::from_hex(stored)?;
    if bytes.len() <= NONCE_LEN {
        return Err(AuthError::Crypto("stored key is truncated".into()));
    }
    let (nonce, ciphertext) = bytes.split_at(NONCE_LEN);
    let plaintext = user_key(user_id)?
        .decrypt(Nonce::from_slice(nonce), ciphertext)
        .map_err(|_| AuthError::Crypto("key decryption failed — wrong secret?".into()))?;
    String::from_utf8(plaintext).map_err(|_| AuthError::Crypto("stored key is corrupt".into()))
}
