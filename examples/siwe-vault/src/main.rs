/* the eth and auth pillars together: users sign in with their own wallet
(SIWE — self-custodial, the server never sees the key) or with an email
one-time code (and get an embedded, server-custodied wallet). either way,
user.wallet() drives personalized on-chain reads and — for embedded
wallets — server-side transactions */

use lib::eth;
use lib::prelude::*;

#[contract("abi/erc20.json")]
struct Erc20;

const USDC: &str = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";

#[derive(Schema)]
struct WalletInfo {
    address: String,
    kind: String,
}

#[derive(Schema)]
struct Portfolio {
    address: String,
    eth: Wei,
    usdc: U256,
}

/// The session's wallets: "linked" for SIWE logins, "embedded" for email
/// signups — one wallet manager seen from two doors.
#[rest(get, "/me")]
fn me(user: User) -> Vec<WalletInfo> {
    user.wallets()
        .into_iter()
        .map(|wallet| WalletInfo {
            address: wallet.address().to_string(),
            kind: wallet.kind().as_str().to_string(),
        })
        .collect()
}

/// On-chain state of the caller's own wallet — the session picks the
/// address, no address parameter to spoof.
#[rest(get, "/portfolio")]
fn portfolio(user: User) -> Result<Portfolio, HandlerError> {
    let address = user.wallet().address();
    Ok(Portfolio {
        address: address.to_string(),
        eth: eth::balance(address)?,
        usdc: Erc20::at(USDC).balance_of(address).call()?,
    })
}

/// Live feed of USDC transfers into the caller's wallet.
#[sse("/incoming")]
fn incoming(user: User) -> Result<impl Iterator<Item = Transfer>, HandlerError> {
    let mine = user.wallet().address();
    Ok(Erc20::at(USDC)
        .events::<Transfer>()?
        .filter(move |transfer| transfer.to == mine))
}

/// Sends USDC from the caller's wallet. Embedded wallets sign server-side;
/// a linked wallet refuses — it is self-custodial, the user signs in their
/// own wallet app.
#[rest(post, "/tip")]
fn tip(user: User, to: Address, amount: U256) -> Result<TxHandle, HandlerError> {
    Ok(Erc20::at(USDC)
        .transfer(to, amount)
        .from(user.wallet())
        .send()?)
}

fn main() {
    lib::server::run();
}
