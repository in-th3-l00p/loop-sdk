import { CodeBlock } from "@/components/docs/code-block";
import { Callout, Code, DocTitle, H2, Lead, P, Table } from "@/components/docs/prose";

export default function Schemas() {
  return (
    <article>
      <DocTitle ornament="✧ schemas & validation">types carry the contract</DocTitle>
      <Lead>
        every endpoint signature is read into a schema — a precise, serializable
        description of what your api accepts and returns. validation falls out of it
        for free.
      </Lead>

      <H2>deriving records</H2>
      <P>
        <Code>#[derive(Schema)]</Code> turns a struct with named fields into a record
        schema. fields may be primitives, <Code>Option&lt;T&gt;</Code>,{" "}
        <Code>Vec&lt;T&gt;</Code>, maps, or other derived structs — nesting works to
        any depth. structs must be non-generic with named fields.
      </P>
      <CodeBlock
        title="src/main.rs"
        code={`#[derive(Schema)]
struct Motorcycle {
    #[check(min_len = 1)]
    brand: String,
    #[check(min = 1885, max = 2100)]
    year: u32,
    #[check(min = 0.0)]
    price: f64,
    nickname: Option<String>,
}`}
      />
      <P>
        the derive also implements the value conversions both ways, so the struct can
        be an endpoint parameter (decoded and validated from the request) or a return
        type (encoded to JSON) with no extra code.
      </P>

      <H2>supported types</H2>
      <Table
        head={["rust type", "schema", "json shape"]}
        rows={[
          [<Code key="a">bool</Code>, "bool", "boolean"],
          [<Code key="b">i32 / u32 / i64 / u64</Code>, "integer primitives", "number"],
          [<Code key="c">f32 / f64</Code>, "float primitives", "number (NaN/∞ → null)"],
          [<Code key="d">String</Code>, "str", "string"],
          [<Code key="e">Date</Code>, "date", "string"],
          [<Code key="f">Blob</Code>, "blob", "base64 string"],
          [<Code key="g">Option&lt;T&gt;</Code>, "optional", "value or null"],
          [<Code key="h">Vec&lt;T&gt;</Code>, "list", "array"],
          [
            <Code key="i">BTreeMap / HashMap</Code>,
            "map",
            "object (string keys) or [key, value] pairs",
          ],
          [<Code key="j">derived structs</Code>, "record", "object"],
        ]}
      />
      <P>
        <Code>Blob(Vec&lt;u8&gt;)</Code> and <Code>Date(String)</Code> are thin wrapper
        types from the prelude — <Code>Blob</Code> exists so that{" "}
        <Code>Vec&lt;T&gt;</Code> can stay the generic list type while binary data
        travels as base64.
      </P>

      <H2>the six checks</H2>
      <P>
        <Code>#[check(...)]</Code> attaches declarative constraints to struct fields
        and to endpoint parameters. exactly six constraint keys exist:
      </P>
      <Table
        head={["check", "applies to", "failure message"]}
        rows={[
          [<Code key="a">min = n</Code>, "numbers", "must be at least {n}"],
          [<Code key="b">max = n</Code>, "numbers", "must be at most {n}"],
          [
            <Code key="c">min_len = n</Code>,
            "str (chars), blob (bytes), list, map",
            "length must be at least {n}",
          ],
          [
            <Code key="d">max_len = n</Code>,
            "str (chars), blob (bytes), list, map",
            "length must be at most {n}",
          ],
          [
            <Code key="e">pattern = &quot;regex&quot;</Code>,
            "str only",
            "must match pattern {regex}",
          ],
          [
            <Code key="f">one_of(a, b, ...)</Code>,
            "any value, exact equality",
            "must be one of the allowed values",
          ],
        ]}
      />
      <CodeBlock
        title="checks on endpoint parameters"
        code={`#[rest(get, "/move/{player}")]
fn move_player(
    #[check(one_of(1, 2))] player: u32,
    #[check(min = -50.0, max = 50.0)] dx: f64,
    #[check(min = -50.0, max = 50.0)] dy: f64,
) -> bool {
    apply(player, dx, dy)
}`}
      />
      <P>
        several checks can share one attribute (<Code>#[check(min = 0, max = 10)]</Code>)
        and several attributes accumulate. constraints run after type validation and
        are skipped when an optional value is null — a present value is always
        checked.
      </P>

      <H2>validation errors</H2>
      <P>
        validation runs at the wire boundary, before your handler is called. errors
        compose structurally, so a deep failure names its path:
      </P>
      <CodeBlock
        language="json"
        title="400 response"
        code={`{ "error": "invalid request: field \\"year\\": expected u32, found str" }`}
      />
      <P>
        records are strict — unknown fields are rejected, and declared fields must be
        present. a missing field is only tolerated when its schema is optional, in
        which case it decodes to null.
      </P>

      <Callout>
        schemas serialize — <Code>Schema::save</Code> and <Code>Schema::load</Code>{" "}
        persist a schema definition as BSON, the groundwork for cross-language
        integration.
      </Callout>
    </article>
  );
}
