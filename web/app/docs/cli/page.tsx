import { CodeBlock } from "@/components/docs/code-block";
import { Callout, Code, DocTitle, H2, Lead, P, Table } from "@/components/docs/prose";

export default function Cli() {
  return (
    <article>
      <DocTitle ornament="✧ cli & manifest">the loop command</DocTitle>
      <Lead>
        three subcommands and one small manifest. the cli stays out of the way — it
        scaffolds, wires the environment, and hands the rest to cargo.
      </Lead>

      <H2>commands</H2>
      <Table
        head={["command", "flags", "what it does"]}
        rows={[
          [
            <Code key="a">loop init</Code>,
            "—",
            "scaffolds loop.toml, Cargo.toml, .gitignore, and src/main.rs with a working endpoint; refuses to overwrite",
          ],
          [
            <Code key="b">loop dev</Code>,
            <span key="b2">
              <Code>--port</Code>, <Code>--dir</Code>
            </span>,
            "parses the manifest, exports LOOP_PORT and LOOP_DB_URL, runs cargo run",
          ],
          [
            <Code key="c">loop build</Code>,
            <Code key="c2">--dir</Code>,
            "cargo build --release; prints the binary path target/release/<name>",
          ],
        ]}
      />
      <P>
        port precedence for <Code>loop dev</Code>: the <Code>--port</Code> flag, then{" "}
        <Code>[dev].port</Code> in the manifest, then 3000.
      </P>

      <H2>loop.toml</H2>
      <CodeBlock
        language="toml"
        title="loop.toml — every recognized key"
        code={`name = "my-api"

[dev]
port = 3000

[database]
url = "postgres://localhost/shop"`}
      />
      <Table
        head={["key", "required", "behavior"]}
        rows={[
          [<Code key="a">name</Code>, "yes", "project and binary name"],
          [<Code key="b">[dev].port</Code>, "no", "dev server port, default 3000"],
          [
            <Code key="c">[database]</Code>,
            "no",
            "presence opts the project into a database connection at startup",
          ],
          [
            <Code key="d">[database].url</Code>,
            "no",
            "connection url; empty section falls back to sqlite:<name>.db",
          ],
        ]}
      />

      <H2>environment</H2>
      <Table
        head={["variable", "read by", "meaning"]}
        rows={[
          [<Code key="a">LOOP_PORT</Code>, "server", "listen port, set by loop dev"],
          [<Code key="b">LOOP_ADDR</Code>, "server", "bind address, default 127.0.0.1"],
          [
            <Code key="c">LOOP_DB_URL</Code>,
            "server + cli",
            "database url; shell value overrides the manifest",
          ],
          [
            <Code key="d">LOOP_LIB_PATH</Code>,
            "loop init",
            "override the sdk lib path written into Cargo.toml",
          ],
          [
            <Code key="e">LOOP_INSTALL_DIR</Code>,
            "install.sh",
            "install destination, default ~/.local/bin",
          ],
        ]}
      />

      <H2>project anatomy</H2>
      <CodeBlock
        language="toml"
        title="Cargo.toml — as scaffolded"
        code={`[package]
name = "my-api"
version = "0.1.0"
edition = "2024"

[workspace]

[dependencies]
lib = { path = "/path/to/loop-sdk/crates/lib", features = ["server", "macros"] }`}
      />
      <P>
        <Code>server</Code> brings the http/sse/websocket serving layer;{" "}
        <Code>macros</Code> brings <Code>#[rest]</Code>, <Code>#[sse]</Code>,{" "}
        <Code>#[live]</Code>, and <Code>#[derive(Schema)]</Code>. add{" "}
        <Code>db-sqlite</Code> or <Code>db-postgres</Code> to opt into storage.
      </P>

      <Callout>
        the examples directory holds three complete apps — motorcycle-shop (rest
        crud), ollama-stream (sse token streaming), and circle-game (live websocket
        board) — each runnable with <Code>loop dev</Code> from its directory.
      </Callout>
    </article>
  );
}
