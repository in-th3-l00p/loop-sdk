import { CodeBlock } from "@/components/docs/code-block";
import { Callout, Code, DocTitle, H2, Lead, P, Table, Ul, Li } from "@/components/docs/prose";

export default function Auth() {
  return (
    <article>
      <DocTitle ornament="❦ identity">authentication</DocTitle>
      <Lead>
        privy-grade identity as configuration: email &amp; password, email one-time
        codes, and ethereum wallets — with an embedded wallet manager so every user
        can hold keys, even the ones who signed up with an email.
      </Lead>

      <H2>configuration</H2>
      <P>
        authentication is declared in the manifest (and needs a{" "}
        <Code>[database]</Code>, where sessions and users live). enabling it mounts
        the auth routes, provisions the session store, and — when the wallet manager
        is on — creates an embedded wallet for every new user. enable the{" "}
        <Code>auth</Code> cargo feature; the wallet provider additionally needs{" "}
        <Code>eth</Code>.
      </P>
      <CodeBlock
        language="toml"
        title="loop.toml"
        code={`[auth]
providers = ["email-password", "email-otp", "wallet"]
session_ttl = "30d"
secret = "env:LOOP_AUTH_SECRET"   # for embedded wallets; loop auth secret new

[auth.otp]
from = "login@my-api.dev"
digits = 6
ttl = "10m"

[auth.wallet]
chain_id = 1
embedded = true`}
      />
      <P>
        one-time codes are delivered through a pluggable <Code>Mailer</Code>; the
        default prints the code to the server console, which is exactly what local
        development wants. register a real provider with{" "}
        <Code>lib::auth::set_mailer(...)</Code> before <Code>lib::server::run()</Code>.
      </P>

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
        sessions travel as bearer tokens (with a <Code>?token=</Code> query fallback
        for EventSource and browser WebSockets, which cannot set headers). every
        session-returning flow answers the same shape:{" "}
        <Code>{`{ "token": ..., "user": { "id": ..., "email": ..., "wallets": [...] } }`}</Code>
        . tokens are stored hashed, so a leaked table cannot be replayed.
      </P>

      <H2>guarding endpoints</H2>
      <P>
        the signature carries the contract, so it also carries the guard. a{" "}
        <Code>User</Code> parameter means the endpoint requires a session — no
        annotation, no middleware. absent or invalid credentials answer 401 before
        your code runs. <Code>Option&lt;User&gt;</Code> makes it optional.
      </P>
      <CodeBlock
        title="user injection"
        code={`use lib::prelude::*;

#[rest(get, "/me")]
fn me(user: User) -> Profile {
    Profile {
        id: user.id().to_string(),
        email: user.email().unwrap_or_default(),
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
      <P>
        detection is by type name, so <Code>User</Code> is a reserved parameter type
        in endpoint signatures (and <Code>token</Code> a reserved query name).
        context parameters never appear in the endpoint&apos;s wire schema.
      </P>

      <H2>the user</H2>
      <CodeBlock
        title="lib::auth surface"
        code={`impl User {
    pub fn id(&self) -> UserId;
    pub fn email(&self) -> Option<String>;
    pub fn wallet(&self) -> Wallet;          // primary (first) wallet
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
        title="wallets"
        code={`impl Wallet {
    pub fn address(&self) -> Address;
    pub fn kind(&self) -> WalletKind;        // Embedded | Linked
    pub fn sign_message(&self, message: &[u8]) -> Result<Vec<u8>, AuthError>;
    pub fn export(&self) -> Result<String, AuthError>;   // 0x-hex key, embedded only
}

// Wallet implements eth::Signer, so sending is the contract builder:
usdc.transfer(to, amount).from(user.wallet()).send()?`}
      />
      <Ul>
        <Li>
          embedded keys live encrypted at rest in the project database
          (chacha20-poly1305 under a per-user key derived from{" "}
          <Code>LOOP_AUTH_SECRET</Code>), unlocked per request — never written to
          logs, never serialized into a schema value
        </Li>
        <Li>
          <Code>export()</Code> is a deliberate, auditable action — call it only from
          flows that just re-authenticated the user
        </Li>
        <Li>
          signing over an embedded wallet happens server-side; a linked wallet is
          self-custodial, so server-side signing refuses with a clear error and the
          client signs in the user&apos;s own wallet app — one <Code>Wallet</Code>{" "}
          api either way
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
        — the nonce route issues the message (single-use, 10 minutes), the verify
        route checks the signature and recovers the address. verifying with a live
        session <em>links</em> the proven wallet to that account; without one it
        logs in, registering on first use.
      </Callout>
    </article>
  );
}
