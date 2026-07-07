import { CodeBlock } from "@/components/docs/code-block";
import { Callout, Code, DocTitle, H2, Lead, P, Table, Ul, Li } from "@/components/docs/prose";

export default function Endpoints() {
  return (
    <article>
      <DocTitle ornament="✧ endpoints">three ways to answer</DocTitle>
      <Lead>
        one attribute turns a function into an endpoint. rest for request/response,
        sse for one-way streams, live for websocket feeds — all typed, all validated,
        all registered automatically.
      </Lead>

      <H2>rest</H2>
      <P>
        <Code>#[rest(method, &quot;path&quot;)]</Code> accepts get, post, put, delete,
        patch, head, or options, and a path that may contain{" "}
        <Code>{"{param}"}</Code> segments.
      </P>
      <CodeBlock
        title="rest endpoints"
        code={`#[rest(get, "/motorcycles/{id}")]
fn get(id: u64) -> Result<Listing, HandlerError> {
    let shop = SHOP.lock().unwrap();
    let motorcycle = shop.inventory.get(&id).ok_or(not_found(id))?;
    Ok(Listing { id, motorcycle: motorcycle.clone() })
}

#[rest(put, "/motorcycles/{id}")]
fn update(id: u64, motorcycle: Motorcycle) -> Result<bool, HandlerError> {
    Ok(SHOP.lock().unwrap().inventory.insert(id, motorcycle).is_some())
}

fn not_found(id: u64) -> HandlerError {
    format!("no motorcycle with id {id}").into()
}`}
      />

      <H2>how parameters bind</H2>
      <P>
        every function parameter resolves against the request, trying sources in a
        fixed order of precedence:
      </P>
      <Ul>
        <Li>
          a path segment named <Code>{"{name}"}</Code>
        </Li>
        <Li>
          the whole JSON body — when the parameter is the signature&apos;s only
          record-typed parameter
        </Li>
        <Li>
          the body field <Code>body[name]</Code>
        </Li>
        <Li>
          the query parameter <Code>?name=...</Code>
        </Li>
        <Li>
          null, if the type is <Code>Option&lt;T&gt;</Code> — otherwise a 400 for the
          missing parameter
        </Li>
      </Ul>
      <P>
        path and query values are strings, so they must decode to primitives. lists,
        maps, and records travel in the body. one handler can mix all three sources:
      </P>
      <CodeBlock
        title="path + body + query in one signature"
        code={`#[rest(post, "/teams/{team}")]
fn join(
    #[check(pattern = "^[a-z]+$")] team: String,
    person: Person,
    #[check(one_of(1, 2, 3))] level: u32,
) -> String {
    format!("{} joined {team} at level {level}", person.name)
}`}
      />

      <H2>return types</H2>
      <P>
        rest handlers return any schema value directly — primitives, options, vectors,
        maps, derived structs — or <Code>Result&lt;T, HandlerError&gt;</Code> for the
        fallible form. the response body is the bare JSON encoding of the value, no
        envelope.
      </P>
      <P>
        <Code>HandlerError</Code> is a boxed error; anything that converts in works,
        and a string is the common case: <Code>Err(&quot;no such thing&quot;.into())</Code>.
      </P>
      <Table
        head={["failure", "status"]}
        rows={[
          ["bad shape, failed check, missing parameter", "400 with { \"error\": message }"],
          ["unknown route", "404"],
          ["handler error, invalid output", "500 with { \"error\": message }"],
        ]}
      />

      <H2>sse — server-sent streams</H2>
      <P>
        <Code>#[sse(&quot;path&quot;)]</Code> mounts a GET route that speaks{" "}
        <Code>text/event-stream</Code>. the handler returns a fallible iterator; each
        item becomes one <Code>data:</Code> event as it is produced. this is the shape
        that streams LLM tokens:
      </P>
      <CodeBlock
        title="ollama-stream — forwarding generation tokens"
        code={`#[sse("/generate")]
fn generate(prompt: String) -> Result<impl Iterator<Item = String>, HandlerError> {
    let response = ureq::post(OLLAMA)
        .send_json(ureq::json!({ "model": MODEL, "prompt": prompt, "stream": true }))
        .map_err(|e| format!("ollama request failed: {e}"))?;

    let lines = BufReader::new(response.into_reader()).lines();
    let mut done = false;

    Ok(lines
        .map_while(move |line| {
            if done { return None; }
            let chunk: Chunk = serde_json::from_str(&line.ok()?).ok()?;
            done = chunk.done;
            Some(chunk.response)
        })
        .filter(|token| !token.is_empty()))
}`}
      />
      <CodeBlock
        language="bash"
        title="terminal"
        code={`$ curl -N "localhost:3000/generate?prompt=why%20rust"
data: because
data:  the
data:  compiler
data:  is on your side`}
      />

      <H2>live — websocket feeds</H2>
      <P>
        <Code>#[live(&quot;path&quot;)]</Code> upgrades to a websocket and pushes each
        iterator item as a JSON text frame. same contract as sse, but items can be any
        schema value, not just what fits in an event line:
      </P>
      <CodeBlock
        title="circle-game — a 20fps board feed"
        code={`#[live("/watch")]
fn watch() -> Result<impl Iterator<Item = BTreeMap<String, Vec<f64>>>, HandlerError> {
    Ok(std::iter::repeat_with(|| {
        std::thread::sleep(FRAME);
        frame()
    }))
}`}
      />
      <P>
        streaming handlers run on a blocking task and items are bridged through a
        bounded channel, so a slow consumer applies natural backpressure. a mid-stream
        error becomes an <Code>event: error</Code> frame on sse and an{" "}
        <Code>{`{"error": ...}`}</Code> frame on live.
      </P>

      <H2>the server</H2>
      <P>
        endpoints self-register at link time through a global inventory —{" "}
        <Code>lib::server::run()</Code> collects them, prints the route table, honors{" "}
        <Code>LOOP_ADDR</Code> and <Code>LOOP_PORT</Code>, connects the database when
        configured, and serves. duplicate routes are rejected at startup.
      </P>
      <CodeBlock
        title="src/main.rs"
        code={`fn main() {
    lib::server::run();
}`}
      />

      <Callout>
        sse and live both occupy the GET slot for their path — an sse route and a rest
        GET on the same path is a startup conflict, caught before serving begins.
      </Callout>
    </article>
  );
}
