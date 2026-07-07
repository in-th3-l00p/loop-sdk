import Link from "next/link";
import { CodeBlock } from "@/components/docs/code-block";
import { Callout, DocTitle, H2, Lead, Li, P, Ul, Code } from "@/components/docs/prose";

export default function Introduction() {
  return (
    <article>
      <DocTitle ornament="✧ introduction">the loop sdk</DocTitle>
      <Lead>
        a development kit for building apps in the modern age of AI, focused on
        decentralization. you write ordinary Rust functions — loop derives the
        schemas, the validation, the wire protocol, and the server.
      </Lead>

      <P>
        the core idea is that your types already carry the contract. a handler&apos;s
        signature declares what it accepts and what it returns; loop reads that
        signature and takes care of everything between your types and your users:
        JSON decoding, schema validation, HTTP routing, streaming, and persistence.
      </P>

      <CodeBlock
        title="src/main.rs"
        code={`use lib::prelude::*;

#[rest(post, "/add")]
fn add(a: i64, b: i64) -> i64 {
    a + b
}

fn main() {
    lib::server::run();
}`}
      />

      <P>
        that is a complete loop application. <Code>loop dev</Code> compiles and serves
        it; a <Code>POST /add</Code> with <Code>{`{"a": 1, "b": 2}`}</Code> returns{" "}
        <Code>3</Code>, and a request with the wrong shape is rejected with a precise
        error before your function ever runs.
      </P>

      <H2>what exists today</H2>
      <Ul>
        <Li>
          <Link href="/docs/schemas" className="text-brand-soft hover:underline">
            schemas &amp; validation
          </Link>{" "}
          — <Code>#[derive(Schema)]</Code> records and declarative{" "}
          <Code>#[check(...)]</Code> constraints on fields and parameters
        </Li>
        <Li>
          <Link href="/docs/endpoints" className="text-brand-soft hover:underline">
            endpoints
          </Link>{" "}
          — <Code>#[rest]</Code> for request/response, <Code>#[sse]</Code> for
          server-sent streams, <Code>#[live]</Code> for websocket feeds
        </Li>
        <Li>
          <Link href="/docs/database" className="text-brand-soft hover:underline">
            database
          </Link>{" "}
          — schema-guided queries and versioned migrations over sqlite and postgres
        </Li>
        <Li>
          <Link href="/docs/cli" className="text-brand-soft hover:underline">
            cli
          </Link>{" "}
          — <Code>loop init</Code>, <Code>loop dev</Code>, <Code>loop build</Code>, and
          the <Code>loop.toml</Code> manifest
        </Li>
      </Ul>

      <H2>what comes next</H2>
      <P>
        the blueprints section holds the designed-but-not-yet-implemented surface:
        authentication with a wallet manager, payments and subscriptions over stripe
        and ethereum, and the ethereum sdk. those pages are API previews — the shapes
        are settled enough to read, but the code does not ship yet.
      </P>

      <Callout>
        loop is developed in the open by{" "}
        <Link href="https://intheloop.space" className="text-brand-soft hover:underline">
          intheloop
        </Link>
        , an independent software r&amp;d studio. expect sharp edges.
      </Callout>
    </article>
  );
}
