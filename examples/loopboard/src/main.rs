/* loopboard — the whole framework in one app. a social tip board where
users sign in with an email code (embedded wallet) or their own wallet
(SIWE, self-custodial), post to a shared board, tip each other credits
through guarded atomic transactions, watch the board live over websocket
and their own activity over sse, and move real usdc on-chain — the server
signing for embedded wallets, the browser wallet signing for linked ones.
the frontend in public/ is served by the framework itself */

use lib::auth::{UserId, users};
use lib::database::{self, DatabaseError};
use lib::eth;
use lib::prelude::*;

#[contract("abi/erc20.json")]
struct Erc20;

const USDC: &str = "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";
const STARTING_CREDITS: i64 = 100;

// ---------------------------------------------------------------- identity

#[derive(Schema)]
struct WalletInfo {
    address: String,
    kind: String,
}

#[derive(Schema)]
struct Profile {
    id: String,
    handle: String,
    credits: i64,
    wallets: Vec<WalletInfo>,
}

/// How a user shows up on the board: their email, or their wallet.
fn handle_of(user: &User) -> String {
    if let Some(email) = user.email() {
        return email;
    }
    match user.wallets().first() {
        Some(wallet) => {
            let full = wallet.address().to_string();
            format!("{}…{}", &full[..6], &full[full.len() - 4..])
        }
        None => user.id().to_string(),
    }
}

/// First touch grants the starting credits; later touches are no-ops.
fn ensure_ledger(user: &User) -> Result<(), HandlerError> {
    database::query(
        "INSERT INTO ledger (user_id, handle, balance) VALUES (?, ?, ?) \
         ON CONFLICT(user_id) DO NOTHING",
    )
    .bind(user.id().to_string())
    .bind(handle_of(user))
    .bind(STARTING_CREDITS)
    .execute()?;
    Ok(())
}

fn credits_of(user_id: &str) -> Result<i64, HandlerError> {
    Ok(database::query("SELECT balance FROM ledger WHERE user_id = ?")
        .bind(user_id.to_string())
        .fetch_optional()?
        .unwrap_or(0))
}

#[rest(get, "/me")]
fn me(user: User) -> Result<Profile, HandlerError> {
    ensure_ledger(&user)?;
    Ok(Profile {
        id: user.id().to_string(),
        handle: handle_of(&user),
        credits: credits_of(user.id().as_str())?,
        wallets: user
            .wallets()
            .into_iter()
            .map(|wallet| WalletInfo {
                address: wallet.address().to_string(),
                kind: wallet.kind().as_str().to_string(),
            })
            .collect(),
    })
}

// ------------------------------------------------------------------- board

#[derive(Schema)]
struct PostView {
    id: i64,
    handle: String,
    text: String,
    tips: i64,
    created_at: i64,
}

#[rest(post, "/posts")]
fn create_post(
    user: User,
    #[check(min_len = 1, max_len = 280)] text: String,
) -> Result<PostView, HandlerError> {
    ensure_ledger(&user)?;
    let now = unix_now();
    database::query("INSERT INTO posts (author, handle, text, created_at) VALUES (?, ?, ?, ?)")
        .bind(user.id().to_string())
        .bind(handle_of(&user))
        .bind(text)
        .bind(now)
        .execute()?;
    database::query(
        "SELECT p.id, p.handle, p.text, 0 AS tips, p.created_at \
         FROM posts p WHERE p.author = ? ORDER BY p.id DESC",
    )
    .bind(user.id().to_string())
    .fetch_one()
    .map_err(Into::into)
}

const BOARD_SQL: &str = "SELECT p.id, p.handle, p.text, \
    COALESCE(SUM(t.amount), 0) AS tips, p.created_at \
    FROM posts p LEFT JOIN tips t ON t.post_id = p.id \
    GROUP BY p.id ORDER BY p.id DESC LIMIT 50";

/// The board is public — Option<User> so logged-out visitors read too.
#[rest(get, "/posts")]
fn list_posts(_viewer: Option<User>) -> Result<Vec<PostView>, HandlerError> {
    database::query(BOARD_SQL).fetch_all().map_err(Into::into)
}

// -------------------------------------------------------------- tipping

#[derive(Schema)]
struct TipReceipt {
    credits: i64,
}

/// Credits move atomically: debit (guarded by the balance), credit,
/// record — one transaction, all or nothing.
#[rest(post, "/tips")]
fn tip(
    user: User,
    post_id: i64,
    #[check(min = 1, max = 1000)] amount: i64,
) -> Result<TipReceipt, HandlerError> {
    ensure_ledger(&user)?;
    let tipper = user.id().to_string();

    #[derive(Schema)]
    struct PostAuthor {
        author: String,
        handle: String,
    }
    let Some(post) = database::query("SELECT author, handle FROM posts WHERE id = ?")
        .bind(post_id)
        .fetch_optional::<PostAuthor>()?
    else {
        return Err(with_status(StatusCode::NOT_FOUND, "no such post"));
    };
    if post.author == tipper {
        return Err(with_status(
            StatusCode::BAD_REQUEST,
            "tipping yourself is a loop too small",
        ));
    }
    // recipients might never have logged their ledger row yet
    database::query(
        "INSERT INTO ledger (user_id, handle, balance) VALUES (?, ?, ?) \
         ON CONFLICT(user_id) DO NOTHING",
    )
    .bind(post.author.clone())
    .bind(post.handle)
    .bind(STARTING_CREDITS)
    .execute()?;

    let moved = database::atomic()
        .guard("UPDATE ledger SET balance = balance - ? WHERE user_id = ? AND balance >= ?")
        .bind(amount)
        .bind(tipper.clone())
        .bind(amount)
        .query("UPDATE ledger SET balance = balance + ? WHERE user_id = ?")
        .bind(amount)
        .bind(post.author.clone())
        .query(
            "INSERT INTO tips (post_id, tipper, recipient, amount, created_at) \
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(post_id)
        .bind(handle_of(&user))
        .bind(post.author)
        .bind(amount)
        .bind(unix_now())
        .execute();
    match moved {
        Ok(()) => Ok(TipReceipt {
            credits: credits_of(&tipper)?,
        }),
        Err(DatabaseError::Guard(_)) => Err(with_status(
            StatusCode::BAD_REQUEST,
            "not enough credits",
        )),
        Err(e) => Err(e.into()),
    }
}

// ------------------------------------------------------------- streaming

#[derive(Schema, Clone)]
struct Received {
    post_id: i64,
    from: String,
    amount: i64,
}

/// Your personal activity stream: tips landing on your posts, as they land.
#[sse("/feed")]
fn feed(user: User) -> Result<impl Iterator<Item = Received>, HandlerError> {
    let me = user.id().to_string();
    let mut cursor: i64 = database::query("SELECT COALESCE(MAX(id), 0) FROM tips").fetch_one()?;
    let mut pending: Vec<Received> = Vec::new();

    Ok(std::iter::from_fn(move || {
        loop {
            if let Some(event) = pending.pop() {
                return Some(event);
            }
            #[derive(Schema)]
            struct Row {
                id: i64,
                post_id: i64,
                tipper: String,
                amount: i64,
            }
            let fresh: Vec<Row> = database::query(
                "SELECT id, post_id, tipper, amount FROM tips \
                 WHERE recipient = ? AND id > ? ORDER BY id DESC",
            )
            .bind(me.clone())
            .bind(cursor)
            .fetch_all()
            .ok()?;
            if let Some(newest) = fresh.first() {
                cursor = newest.id;
            }
            pending.extend(fresh.into_iter().map(|row| Received {
                post_id: row.post_id,
                from: row.tipper,
                amount: row.amount,
            }));
            if pending.is_empty() {
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
        }
    }))
}

#[derive(Schema)]
struct Rank {
    handle: String,
    credits: i64,
}

#[derive(Schema)]
struct Board {
    posts: Vec<PostView>,
    leaderboard: Vec<Rank>,
}

/// The whole board, pushed whenever anything changes. Public.
#[live("/board")]
fn board() -> Result<impl Iterator<Item = Board>, HandlerError> {
    let mut last_state: Option<(i64, i64)> = None;

    Ok(std::iter::from_fn(move || {
        loop {
            let posts: i64 = database::query("SELECT COALESCE(MAX(id), 0) FROM posts")
                .fetch_one()
                .ok()?;
            let tips: i64 = database::query("SELECT COALESCE(MAX(id), 0) FROM tips")
                .fetch_one()
                .ok()?;
            if last_state != Some((posts, tips)) {
                last_state = Some((posts, tips));
                let posts = database::query(BOARD_SQL).fetch_all().ok()?;
                let leaderboard = database::query(
                    "SELECT handle, balance AS credits FROM ledger \
                     ORDER BY balance DESC, handle LIMIT 5",
                )
                .fetch_all()
                .ok()?;
                return Some(Board { posts, leaderboard });
            }
            std::thread::sleep(std::time::Duration::from_secs(1));
        }
    }))
}

// -------------------------------------------------------------- on-chain

#[derive(Schema)]
struct OnchainWallet {
    address: String,
    kind: String,
    eth: Wei,
    usdc: U256,
}

/// Live chain state of the caller's wallet.
#[rest(get, "/wallet")]
fn wallet(user: User) -> Result<OnchainWallet, HandlerError> {
    let wallet = user.wallet();
    let address = wallet.address();
    // usdc only resolves where the token is deployed (mainnet or a mainnet
    // fork). on a bare devnet or a testnet there is no contract at that
    // address, so treat a no-contract call as a zero balance rather than
    // failing the whole panel — the native eth balance always resolves.
    let usdc = match Erc20::at(USDC).balance_of(address).call() {
        Ok(balance) => balance,
        Err(eth::EthError::Call(_)) => U256::from(0u64),
        Err(other) => return Err(other.into()),
    };
    Ok(OnchainWallet {
        address: address.to_string(),
        kind: wallet.kind().as_str().to_string(),
        eth: eth::balance(address)?,
        usdc,
    })
}

fn author_wallet(post_id: i64) -> Result<Address, HandlerError> {
    let Some(author) = database::query("SELECT author FROM posts WHERE id = ?")
        .bind(post_id)
        .fetch_optional::<String>()?
    else {
        return Err(with_status(StatusCode::NOT_FOUND, "no such post"));
    };
    let recipient = users()
        .find(UserId::new(author))
        .map_err(|e| HandlerError::from(e.to_string()))?
        .and_then(|author| author.wallets().into_iter().next())
        .ok_or_else(|| with_status(StatusCode::BAD_REQUEST, "the author has no wallet"))?;
    Ok(recipient.address())
}

/// Real usdc to a post's author. Embedded wallets sign server-side;
/// linked wallets refuse — they are self-custodial (use /tip-calldata).
#[rest(post, "/tip-onchain")]
fn tip_onchain(user: User, post_id: i64, amount: U256) -> Result<TxHandle, HandlerError> {
    let recipient = author_wallet(post_id)?;
    Ok(Erc20::at(USDC)
        .transfer(recipient, amount)
        .from(user.wallet())
        .send()?)
}

#[derive(Schema)]
struct Calldata {
    to: String,
    data: String,
}

/// The same transfer, prepared for the user's own wallet to sign
/// (eth_sendTransaction in the browser) — self-custody end to end.
#[rest(get, "/tip-calldata")]
fn tip_calldata(user: User, post_id: i64, amount: U256) -> Result<Calldata, HandlerError> {
    let _ = user; // any session may prepare calldata; the wallet signs it
    let recipient = author_wallet(post_id)?;
    let transfer = Erc20::at(USDC).transfer(recipient, amount);
    Ok(Calldata {
        to: transfer.to().to_string(),
        data: transfer.calldata()?,
    })
}

fn unix_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock before unix epoch")
        .as_secs() as i64
}

fn main() {
    lib::server::run();
}
