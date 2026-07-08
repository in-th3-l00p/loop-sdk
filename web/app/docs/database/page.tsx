import { CodeBlock } from "@/components/docs/code-block";
import { Callout, Code, DocTitle, H2, Lead, P, Table, Ul, Li } from "@/components/docs/prose";

export default function Database() {
  return (
    <article>
      <DocTitle ornament="✧ database">schema-guided storage</DocTitle>
      <Lead>
        the same types that shape your endpoints shape your rows. one query api over
        sqlite and postgres, with versioned migrations and dialect translation built
        in.
      </Lead>

      <H2>configuration</H2>
      <P>
        add a <Code>[database]</Code> section to <Code>loop.toml</Code> and{" "}
        <Code>loop dev</Code> exports <Code>LOOP_DB_URL</Code> for you. an empty
        section defaults to a project-named sqlite file; a shell-level{" "}
        <Code>LOOP_DB_URL</Code> overrides everything. the driver is inferred from the
        url scheme.
      </P>
      <CodeBlock
        language="toml"
        title="loop.toml"
        code={`name = "my-api"

[database]
url = "postgres://localhost/shop"`}
      />
      <P>
        the server connects at startup and runs pending migrations before accepting
        traffic. enable the driver features on the sdk dependency:{" "}
        <Code>db-sqlite</Code> and/or <Code>db-postgres</Code>.
      </P>

      <H2>migrations</H2>
      <P>
        a migration is a version, a name, and sql. applied migrations are recorded
        with a checksum — editing an applied migration is rejected, as are duplicate
        or out-of-order versions. each migration and its bookkeeping commit in one
        transaction.
      </P>
      <CodeBlock
        title="declaring migrations"
        code={`use lib::database::Migration;

fn migrations() -> Vec<Migration> {
    vec![Migration::new(
        1,
        "create_users",
        "CREATE TABLE users (
            id BIGSERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            nickname TEXT
        )",
    )]
}`}
      />
      <P>
        migration sql is written once and translated per dialect —{" "}
        <Code>BIGSERIAL</Code> becomes <Code>INTEGER</Code> on sqlite,{" "}
        <Code>BLOB</Code> becomes <Code>BYTEA</Code> on postgres, and{" "}
        <Code>TIMESTAMP</Code> becomes <Code>TEXT</Code> on both. multi-statement
        migrations apply atomically.
      </P>

      <H2>queries</H2>
      <P>
        <Code>lib::database::query</Code> builds a bound query against the global
        connection. placeholders are always <Code>?</Code> — they are renumbered to{" "}
        <Code>$1, $2, ...</Code> on postgres automatically, skipping string literals
        and comments.
      </P>
      <CodeBlock
        title="inside a handler"
        code={`#[rest(post, "/users")]
fn create(name: String, nickname: Option<String>) -> Result<u64, HandlerError> {
    let inserted = lib::database::query(
        "INSERT INTO users (name, nickname) VALUES (?, ?)")
        .bind(name)
        .bind(nickname)
        .execute()?;
    Ok(inserted)
}

#[rest(get, "/users")]
fn list() -> Result<Vec<User>, HandlerError> {
    Ok(lib::database::query(
        "SELECT id, name, nickname FROM users ORDER BY id")
        .fetch_all()?)
}`}
      />
      <P>
        results decode by the target type&apos;s schema: a{" "}
        <Code>#[derive(Schema)]</Code> struct maps columns by field name, a scalar
        like <Code>i64</Code> or <Code>Option&lt;String&gt;</Code> reads the first
        column. every fetch method has a blocking form for handler threads and an{" "}
        <Code>_async</Code> form:
      </P>
      <Table
        head={["method", "returns"]}
        rows={[
          [<Code key="a">fetch_all()</Code>, <Code key="a2">Vec&lt;T&gt;</Code>],
          [<Code key="b">fetch_one()</Code>, <span key="b2"><Code>T</Code> — error when empty</span>],
          [<Code key="c">fetch_optional()</Code>, <Code key="c2">Option&lt;T&gt;</Code>],
          [<Code key="d">execute()</Code>, "affected row count"],
        ]}
      />

      <H2>transactions</H2>
      <P>
        <Code>database::atomic()</Code> runs a batch of statements in one
        transaction — all or nothing. a <Code>guard</Code> statement must affect at
        least one row, or the whole batch rolls back: the natural home for balance
        checks and optimistic conditions.
      </P>
      <CodeBlock
        code={`database::atomic()
    .guard("UPDATE ledger SET balance = balance - ? WHERE id = ? AND balance >= ?")
    .bind(amount).bind(&payer).bind(amount)   // no row → rollback, DatabaseError::Guard
    .query("UPDATE ledger SET balance = balance + ? WHERE id = ?")
    .bind(amount).bind(&payee)
    .query("INSERT INTO transfers (payer, payee, amount) VALUES (?, ?, ?)")
    .bind(&payer).bind(&payee).bind(amount)
    .execute()?;`}
      />

      <H2>drivers</H2>
      <Ul>
        <Li>
          <Code>sqlite</Code> — file or <Code>sqlite::memory:</Code> urls, WAL mode on
          disk, foreign keys on, files created on demand
        </Li>
        <Li>
          <Code>postgres</Code> — <Code>postgres://</Code> and{" "}
          <Code>postgresql://</Code> urls, pooled connections
        </Li>
      </Ul>
      <P>
        the storage layer is pluggable behind a backend trait — each driver is one
        self-contained implementation, and new backends slot in without touching the
        query api.
      </P>

      <Callout>
        null handling is symmetrical: <Code>Option&lt;T&gt;</Code> binds as sql NULL
        and NULL columns decode to <Code>None</Code>. binding is limited to primitive
        values for now — lists and maps are rejected with a clear error.
      </Callout>
    </article>
  );
}
