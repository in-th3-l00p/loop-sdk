import { CodeBlock } from "@/components/docs/code-block";
import { Callout, Code, DocTitle, H2, Lead, P, Ul, Li } from "@/components/docs/prose";

export default function Eth() {
  return (
    <article>
      <DocTitle ornament="❦ blueprint" planned>
        ethereum sdk
      </DocTitle>
      <Lead>
        contracts, wallets, and on-chain state as first-class citizens of your
        backend — typed like everything else, streamed like everything else.
      </Lead>

      <Callout tone="planned">
        this page is a design document. the API below is settled in shape but not yet
        implemented — names may drift before it ships.
      </Callout>

      <H2>configuration</H2>
      <CodeBlock
        language="toml"
        title="loop.toml"
        code={`[eth]
rpc = "env:ETH_RPC_URL"
chain_id = 1

[eth.treasury]
key = "env:TREASURY_KEY"`}
      />
      <P>
        the rpc connection is established at startup alongside the database. the
        optional treasury is a server-owned wallet for transactions your app sends on
        its own behalf.
      </P>

      <H2>chain primitives</H2>
      <P>
        <Code>Address</Code>, <Code>Wei</Code>, and <Code>U256</Code> implement the
        schema traits, so they appear in endpoint signatures, validate at the wire
        boundary (a malformed address is a 400 before your code runs), and serialize
        as hex strings.
      </P>
      <CodeBlock
        title="planned — module surface"
        code={`use lib::eth;

#[rest(get, "/balance/{address}")]
fn balance(address: Address) -> Result<Wei, HandlerError> {
    Ok(eth::balance(address)?)
}

#[rest(get, "/gas")]
fn gas() -> Result<Wei, HandlerError> {
    Ok(eth::gas_price()?)
}`}
      />
      <P>
        every chain call has the blocking form for handler threads and an{" "}
        <Code>_async</Code> twin, mirroring the database api.
      </P>

      <H2>typed contracts</H2>
      <P>
        a contract binding is derived from its abi at compile time — reads produce a{" "}
        <Code>.call()</Code>, writes produce a builder that must name its signer.
        method names, argument types, and return types come from the abi, checked by
        the compiler.
      </P>
      <CodeBlock
        title="planned — contract bindings"
        code={`use lib::eth::contract;

#[contract("abi/erc20.json")]
struct Erc20;

#[rest(get, "/usdc/{holder}")]
fn usdc_balance(holder: Address) -> Result<U256, HandlerError> {
    let usdc = Erc20::at("0xa0b8…eb48");
    Ok(usdc.balance_of(holder).call()?)
}

#[rest(post, "/tip")]
fn tip(user: User, to: Address, amount: U256) -> Result<TxHandle, HandlerError> {
    let usdc = Erc20::at("0xa0b8…eb48");
    Ok(usdc.transfer(to, amount).from(user.wallet()).send()?)
}`}
      />
      <P>
        <Code>send()</Code> returns a <Code>TxHandle</Code> immediately —{" "}
        <Code>handle.hash()</Code> for optimistic UIs,{" "}
        <Code>handle.wait(confirmations)</Code> when settlement matters.
      </P>

      <H2>events are iterators</H2>
      <P>
        contract events and chain heads arrive as iterators, which is exactly what{" "}
        <Code>#[sse]</Code> and <Code>#[live]</Code> handlers return. watching a
        contract from a browser becomes three lines:
      </P>
      <CodeBlock
        title="planned — on-chain activity over sse"
        code={`#[sse("/transfers/{token}")]
fn transfers(token: Address) -> Result<impl Iterator<Item = Transfer>, HandlerError> {
    Ok(Erc20::at(token).events::<Transfer>()?)
}

#[live("/heads")]
fn heads() -> Result<impl Iterator<Item = Block>, HandlerError> {
    Ok(eth::blocks()?)
}`}
      />

      <H2>signers</H2>
      <Ul>
        <Li>
          <Code>user.wallet()</Code> — the authenticated user&apos;s wallet from the{" "}
          wallet manager; embedded wallets sign server-side, linked wallets round-trip
          to the client
        </Li>
        <Li>
          <Code>eth::treasury()</Code> — the app&apos;s own wallet, for protocol fees,
          airdrops, sponsored transactions
        </Li>
        <Li>
          both implement one <Code>Signer</Code> trait, so a handler can take the rail
          decision at runtime
        </Li>
      </Ul>

      <Callout>
        the auth wallet manager, the eth payment rail, and this sdk are one layer
        seen from three angles — the same <Code>Wallet</Code> signs a login, a
        subscription, and a contract call.
      </Callout>
    </article>
  );
}
