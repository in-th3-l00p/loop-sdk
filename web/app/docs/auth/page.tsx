import { CodeBlock } from "@/components/docs/code-block";
import { Callout, Code, DocTitle, H2, Lead, P, Table, Ul, Li } from "@/components/docs/prose";

export default function Auth() {
  return (
    <article>
      <DocTitle ornament="❦ blueprint" planned>
        authentication
      </DocTitle>
      <Lead>
        privy-grade identity as configuration: email &amp; password, email one-time
        codes, and ethereum wallets — with an embedded wallet manager so every user
        can hold keys, even the ones who signed up with an email.
      </Lead>

      <Callout tone="planned">
        this page is a design document. the API below is settled in shape but not yet
        implemented — names may drift before it ships.
      </Callout>

      <H2>configuration</H2>
      <P>
        authentication is declared in the manifest. enabling it mounts the{" "}
        <Code>/auth/*</Code> routes, provisions the session store, and — when the
        wallet manager is on — creates an embedded wallet for every new user.
      </P>
      <CodeBlock
        language="toml"
        title="loop.toml"
        code={`[auth]
providers = ["email-password", "email-otp", "wallet"]
session_ttl = "30d"

[auth.otp]
from = "login@my-api.dev"
digits = 6
ttl = "10m"

[auth.wallet]
chain_id = 1
embedded = true`}
      />

      <H2>mounted routes</H2>
      <Table
        head={["route", "provider", "flow"]}
        rows={[
          [
            <Code key="a">POST /auth/register</Code>,
            "email-password",
            "create account, returns a session",
          ],
          [
            <Code key="b">POST /auth/login</Code>,
            "email-password",
            "verify credentials, returns a session",
          ],
          [
            <Code key="c">POST /auth/otp/send</Code>,
            "email-otp",
            "email a one-time code",
          ],
          [
            <Code key="d">POST /auth/otp/verify</Code>,
            "email-otp",
            "exchange code for a session; registers on first use",
          ],
          [
            <Code key="e">GET /auth/wallet/nonce</Code>,
            "wallet",
            "issue a SIWE nonce for an address",
          ],
          [
            <Code key="f">POST /auth/wallet/verify</Code>,
            "wallet",
            "verify the signed message, returns a session",
          ],
          [<Code key="g">POST /auth/logout</Code>, "all", "revoke the session"],
          [<Code key="h">GET /auth/session</Code>, "all", "introspect the current session"],
        ]}
      />
      <P>
        sessions travel as bearer tokens. every flow returns the same shape:{" "}
        <Code>{`{ "token": ..., "user": { "id": ..., "email": ..., "wallets": [...] } }`}</Code>
        .
      </P>

      <H2>guarding endpoints</H2>
      <P>
        the signature carries the contract, so it also carries the guard. a{" "}
        <Code>User</Code> parameter means the endpoint requires a session — no
        annotation, no middleware. absent or invalid credentials answer 401 before
        your code runs. <Code>Option&lt;User&gt;</Code> makes it optional.
      </P>
      <CodeBlock
        title="planned — user injection"
        code={`use lib::prelude::*;
use lib::auth::User;

#[rest(get, "/me")]
fn me(user: User) -> Profile {
    Profile {
        email: user.email(),
        address: user.wallet().address(),
    }
}

#[rest(get, "/feed")]
fn feed(viewer: Option<User>) -> Vec<Post> {
    match viewer {
        Some(user) => personalized(&user),
        None => public_feed(),
    }
}`}
      />

      <H2>the user</H2>
      <CodeBlock
        title="planned — lib::auth surface"
        code={`impl User {
    pub fn id(&self) -> UserId;
    pub fn email(&self) -> Option<String>;
    pub fn wallet(&self) -> Wallet;
    pub fn wallets(&self) -> Vec<Wallet>;
    pub fn link_wallet(&self, address: Address) -> Result<(), AuthError>;
}

pub fn users() -> Users;

impl Users {
    pub fn find(&self, id: UserId) -> Result<Option<User>, AuthError>;
    pub fn by_email(&self, email: &str) -> Result<Option<User>, AuthError>;
    pub fn by_wallet(&self, address: Address) -> Result<Option<User>, AuthError>;
}`}
      />

      <H2>the wallet manager</H2>
      <P>
        the wallet manager is what makes wallet auth symmetrical with email auth. a
        user who arrives with metamask links their external wallet; a user who
        arrives with an email gets an embedded wallet, created and custodied by your
        app, exportable when they are ready to self-custody.
      </P>
      <CodeBlock
        title="planned — wallets"
        code={`impl Wallet {
    pub fn address(&self) -> Address;
    pub fn kind(&self) -> WalletKind;
    pub fn sign_message(&self, message: &[u8]) -> Result<Signature, WalletError>;
    pub fn send(&self, tx: Transaction) -> Result<TxHandle, WalletError>;
    pub fn export(&self) -> Result<EncryptedKey, WalletError>;
}

pub enum WalletKind {
    Embedded,
    Linked,
}`}
      />
      <Ul>
        <Li>
          embedded keys live encrypted at rest in the project database, unlocked per
          request — never written to logs, never serialized into a schema value
        </Li>
        <Li>
          <Code>export()</Code> is a deliberate, auditable action gated by fresh
          re-authentication
        </Li>
        <Li>
          signing over a linked wallet round-trips through the client; signing over an
          embedded wallet happens server-side — one <Code>Wallet</Code> api either way
        </Li>
      </Ul>

      <Callout>
        wallet login speaks{" "}
        <a
          href="https://eips.ethereum.org/EIPS/eip-4361"
          className="text-brand-soft hover:underline"
        >
          sign-in-with-ethereum (EIP-4361)
        </a>{" "}
        — the nonce route issues the message, the verify route checks the signature
        and recovers the address.
      </Callout>
    </article>
  );
}
