import { CodeBlock } from "@/components/docs/code-block";
import { Callout, Code, DocTitle, H2, Lead, P } from "@/components/docs/prose";

export default function Quickstart() {
  return (
    <article>
      <DocTitle ornament="✧ quickstart">from zero to serving</DocTitle>
      <Lead>
        one script to install the cli, one command to scaffold, one to serve.
      </Lead>

      <H2>install the cli</H2>
      <P>
        clone the repository and run the installer. it builds the cli in release mode,
        installs it as <Code>loop</Code> into <Code>~/.local/bin</Code> (override with{" "}
        <Code>LOOP_INSTALL_DIR</Code>), and adds that directory to your PATH if needed.
      </P>
      <CodeBlock
        language="bash"
        title="terminal"
        code={`$ git clone https://github.com/inth3l00p/loop-sdk
$ cd loop-sdk && ./bin/install.sh
$ loop --version`}
      />

      <H2>scaffold a project</H2>
      <P>
        <Code>loop init</Code> refuses to overwrite an existing project and lays down
        four files: the manifest, a cargo manifest wired to the sdk, a gitignore, and a
        working endpoint.
      </P>
      <CodeBlock
        language="bash"
        title="terminal"
        code={`$ mkdir my-api && cd my-api
$ loop init
loop project initialized`}
      />
      <CodeBlock
        language="toml"
        title="loop.toml"
        code={`name = "my-api"

[dev]
port = 3000`}
      />
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

      <H2>serve it</H2>
      <P>
        <Code>loop dev</Code> reads the manifest, exports <Code>LOOP_PORT</Code> (and{" "}
        <Code>LOOP_DB_URL</Code> when a <Code>[database]</Code> section exists), then
        runs your project with cargo. the server prints every registered route on
        boot.
      </P>
      <CodeBlock
        language="bash"
        title="terminal"
        code={`$ loop dev
POST /add
serving on http://127.0.0.1:3000

$ curl -X POST localhost:3000/add \\
    -H 'content-type: application/json' \\
    -d '{"a": 40, "b": 2}'
42`}
      />

      <P>
        arguments bind from the request automatically: path segments, body fields, and
        query parameters all resolve against your function&apos;s signature. send the
        wrong shape and the request is rejected with a 400 and a precise message
        before your code runs.
      </P>

      <Callout>
        ports default to 3000 — the flag <Code>loop dev --port 4000</Code> beats the
        manifest, which beats the default. <Code>loop build</Code> produces a release
        binary at <Code>target/release/&lt;name&gt;</Code>.
      </Callout>
    </article>
  );
}
