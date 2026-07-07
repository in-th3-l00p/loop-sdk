import Link from "next/link";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";

const features = [
  {
    number: "01",
    title: "business logic",
    body: "write a statically typed backend that integrates itself. schemas, validation, and live endpoints over SSE — derived from your types, not duplicated beside them.",
  },
  {
    number: "02",
    title: "authentication",
    body: "oauth, web3 wallets, and one-time passcodes behind a single interface. identity is configuration, not a subsystem you maintain.",
  },
  {
    number: "03",
    title: "ethereum sdk",
    body: "contracts, wallets, and on-chain state as first-class citizens of your backend, with the same typed guarantees as everything else.",
  },
  {
    number: "04",
    title: "payments",
    body: "a payment and subscription manager that treats billing as part of the application model — plans, entitlements, and lifecycle included.",
  },
  {
    number: "05",
    title: "ai engineering",
    body: "streaming-first primitives for building with models: typed endpoints that speak SSE natively, ready for agents and generative interfaces.",
  },
];

const steps = [
  {
    number: "i",
    title: "set up",
    body: "start a project from the CLI or the web app.",
  },
  {
    number: "ii",
    title: "configure",
    body: "add an authentication provider, set up a database, choose your drivers.",
  },
  {
    number: "iii",
    title: "write",
    body: "express your endpoints in Rust or TypeScript — types carry the contract.",
  },
  {
    number: "iv",
    title: "integrate",
    body: "call it from your JavaScript frontend as if it were local.",
  },
];

const heroCode = [
  { text: "#[derive(Schema)]", tone: "text-brand-soft" },
  { text: "struct Motorcycle {", tone: "text-foreground" },
  { text: "    #[check(min_len = 1)]", tone: "text-brand-soft" },
  { text: "    brand: String,", tone: "text-foreground" },
  { text: "    #[check(min = 1885, max = 2100)]", tone: "text-brand-soft" },
  { text: "    year: u32,", tone: "text-foreground" },
  { text: "    nickname: Option<String>,", tone: "text-foreground" },
  { text: "}", tone: "text-foreground" },
  { text: "", tone: "text-foreground" },
  { text: '#[rest(post, "/motorcycles")]', tone: "text-brand-soft" },
  { text: "fn create(motorcycle: Motorcycle) -> u64 {", tone: "text-foreground" },
  { text: '    db::query("INSERT INTO motorcycles ...")', tone: "text-muted-foreground" },
  { text: "        .bind(motorcycle)", tone: "text-muted-foreground" },
  { text: "        .execute()", tone: "text-muted-foreground" },
  { text: "}", tone: "text-foreground" },
];

export default function Home() {
  return (
    <div className="relative flex min-h-dvh flex-col">
      <div aria-hidden="true" className="pointer-events-none fixed inset-0 -z-10 overflow-hidden">
        <div className="absolute inset-0 grid-veil opacity-50" />
        <div className="absolute inset-0 will-change-transform">
          <div className="absolute inset-0 brand-glow animate-drift" />
        </div>
        <div className="absolute inset-0 grain opacity-[0.04] mix-blend-soft-light" />
        <div className="absolute inset-0 vignette" />
      </div>

      <header className="relative z-10">
        <div className="mx-auto flex max-w-5xl items-center justify-between px-6 py-6">
          <Link href="/" className="font-mono text-sm tracking-widest text-foreground">
            ✧ l00p
          </Link>
          <nav className="flex items-center gap-6 font-mono text-xs text-muted-foreground">
            <Link href="#offers" className="transition-colors hover:text-foreground">
              features
            </Link>
            <Link href="#flow" className="transition-colors hover:text-foreground">
              flow
            </Link>
            <Link href="/docs" className="transition-colors hover:text-foreground">
              docs
            </Link>
            <Link
              href="https://github.com/inth3l00p"
              className="transition-colors hover:text-foreground"
            >
              github →
            </Link>
          </nav>
        </div>
      </header>

      <main className="relative z-10 flex-1">
        <section className="mx-auto max-w-5xl px-6 pb-24 pt-16 sm:pt-24">
          <p className="font-mono text-xs tracking-[0.3em] text-muted-foreground">
            № 001 — the sdk
          </p>
          <h1 className="mt-6 max-w-3xl text-balance text-4xl font-medium tracking-tight sm:text-6xl">
            build apps for the modern age of <span className="text-brand-soft">AI</span>
          </h1>
          <p className="mt-6 max-w-xl font-serif text-xl italic text-muted-foreground sm:text-2xl">
            a development kit focused on decentralization — statically typed backends that
            integrate themselves, using the modern standards of SSE.
          </p>
          <div className="mt-10 flex flex-wrap items-center gap-4">
            <Button size="lg" className="bg-brand text-primary hover:bg-brand-soft">
              get started
            </Button>
            <Button size="lg" variant="outline" asChild>
              <Link href="/docs">read the docs →</Link>
            </Button>
          </div>

          <Card className="mt-16 gap-0 overflow-hidden border-border bg-card/80 py-0 backdrop-blur">
            <CardHeader className="flex flex-row items-center justify-between border-b py-3 [.border-b]:pb-3">
              <CardTitle className="font-mono text-xs font-normal text-muted-foreground">
                src/main.rs
              </CardTitle>
              <Badge variant="secondary" className="font-mono text-[10px] tracking-widest">
                rust · live
              </Badge>
            </CardHeader>
            <CardContent className="overflow-x-auto p-6">
              <pre className="font-mono text-xs leading-6 sm:text-sm">
                {heroCode.map((line, index) => (
                  <div key={index} className={line.tone}>
                    {line.text || " "}
                  </div>
                ))}
              </pre>
            </CardContent>
          </Card>
        </section>

        <Separator className="mx-auto max-w-5xl" />

        <section id="offers" className="mx-auto max-w-5xl px-6 py-24">
          <p className="font-mono text-xs tracking-[0.3em] text-muted-foreground">
            № 002 — what it offers
          </p>
          <h2 className="mt-6 max-w-2xl text-3xl font-medium tracking-tight sm:text-4xl">
            everything between your types and your users
          </h2>
          <div className="mt-12 grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
            {features.map((feature) => (
              <Card
                key={feature.number}
                className="border-border bg-card/60 transition-colors hover:border-ring"
              >
                <CardHeader>
                  <p className="font-mono text-xs text-brand-soft">{feature.number}</p>
                  <CardTitle className="text-lg font-medium">{feature.title}</CardTitle>
                </CardHeader>
                <CardContent>
                  <p className="text-sm leading-relaxed text-muted-foreground">{feature.body}</p>
                </CardContent>
              </Card>
            ))}
            <Card className="flex items-center justify-center border-dashed bg-transparent">
              <CardContent className="py-10 text-center">
                <p className="font-serif text-2xl italic text-muted-foreground">❦</p>
                <p className="mt-3 font-mono text-xs text-muted-foreground">more in the works</p>
              </CardContent>
            </Card>
          </div>
        </section>

        <Separator className="mx-auto max-w-5xl" />

        <section id="flow" className="mx-auto max-w-5xl px-6 py-24">
          <p className="font-mono text-xs tracking-[0.3em] text-muted-foreground">
            № 003 — the flow
          </p>
          <h2 className="mt-6 max-w-2xl text-3xl font-medium tracking-tight sm:text-4xl">
            four movements, one loop
          </h2>
          <ol className="mt-12 grid gap-8 sm:grid-cols-2">
            {steps.map((step) => (
              <li key={step.number} className="flex gap-5">
                <span className="font-serif text-2xl italic text-brand-soft">{step.number}.</span>
                <div>
                  <h3 className="font-medium">{step.title}</h3>
                  <p className="mt-1 text-sm leading-relaxed text-muted-foreground">{step.body}</p>
                </div>
              </li>
            ))}
          </ol>

          <Card className="mt-16 border-border bg-card/80 py-0">
            <CardContent className="p-6">
              <pre className="overflow-x-auto font-mono text-xs leading-7 sm:text-sm">
                <div className="text-muted-foreground">$ loop init my-api</div>
                <div className="text-muted-foreground">$ loop dev</div>
                <div className="text-brand-soft">✧ database connected (sqlite)</div>
                <div className="text-brand-soft">✧ 3 endpoints registered</div>
                <div className="text-foreground">→ serving on http://localhost:3000</div>
              </pre>
            </CardContent>
          </Card>
        </section>

        <Separator className="mx-auto max-w-5xl" />

        <section className="mx-auto max-w-5xl px-6 py-24 text-center">
          <p className="font-serif text-3xl italic text-muted-foreground">✧</p>
          <h2 className="mx-auto mt-6 max-w-xl text-balance text-3xl font-medium tracking-tight sm:text-4xl">
            stay in the loop
          </h2>
          <p className="mx-auto mt-4 max-w-md text-sm text-muted-foreground">
            open source, built quietly at the edge of research and craft.
          </p>
          <div className="mt-8 flex justify-center gap-4">
            <Button size="lg" className="bg-brand text-primary hover:bg-brand-soft">
              start a project
            </Button>
          </div>
        </section>
      </main>

      <footer className="relative z-10 border-t">
        <div className="mx-auto flex max-w-5xl flex-col items-center justify-between gap-4 px-6 py-8 font-mono text-xs text-muted-foreground sm:flex-row">
          <p>© 2026 intheloop — an independent software r&d studio</p>
          <p className="tracking-[0.5em]">✧ ❦ ✠</p>
        </div>
      </footer>
    </div>
  );
}
