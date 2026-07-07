import type { Metadata } from "next";
import Link from "next/link";
import { DocsSidebar } from "@/components/docs/sidebar";

export const metadata: Metadata = {
  title: "✧ loop docs",
  description: "The loop sdk manual — schemas, endpoints, database, cli, and the blueprints for what comes next.",
};

export default function DocsLayout({ children }: { children: React.ReactNode }) {
  return (
    <div className="relative flex min-h-dvh flex-col">
      <div aria-hidden="true" className="pointer-events-none fixed inset-0 -z-10 overflow-hidden">
        <div className="absolute inset-0 grid-veil opacity-30" />
        <div className="absolute inset-0 brand-glow opacity-40" />
        <div className="absolute inset-0 grain opacity-[0.04] mix-blend-soft-light" />
        <div className="absolute inset-0 vignette" />
      </div>

      <header className="relative z-10 border-b border-border/60 bg-background/70 backdrop-blur">
        <div className="mx-auto flex max-w-6xl items-center justify-between px-6 py-4">
          <div className="flex items-baseline gap-3">
            <Link href="/" className="font-mono text-sm tracking-widest text-foreground">
              ✧ l00p
            </Link>
            <span className="font-mono text-xs text-muted-foreground">/ docs</span>
          </div>
          <nav className="flex items-center gap-6 font-mono text-xs text-muted-foreground">
            <Link href="/" className="transition-colors hover:text-foreground">
              home
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

      <div className="relative z-10 mx-auto flex w-full max-w-6xl flex-1 flex-col gap-10 px-6 py-10 lg:flex-row">
        <DocsSidebar />
        <main className="min-w-0 flex-1 pb-24">{children}</main>
      </div>

      <footer className="relative z-10 border-t border-border/60">
        <div className="mx-auto flex max-w-6xl flex-col items-center justify-between gap-4 px-6 py-8 font-mono text-xs text-muted-foreground sm:flex-row">
          <p>© 2026 intheloop — an independent software r&d studio</p>
          <p className="tracking-[0.5em]">✧ ❦ ✠</p>
        </div>
      </footer>
    </div>
  );
}
