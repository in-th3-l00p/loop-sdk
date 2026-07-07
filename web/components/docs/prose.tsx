import type { ReactNode } from "react";
import { Badge } from "@/components/ui/badge";
import { cn } from "@/lib/utils";

export function DocTitle({
  ornament,
  children,
  planned,
}: {
  ornament: string;
  children: ReactNode;
  planned?: boolean;
}) {
  return (
    <header className="mb-10">
      <p className="font-mono text-xs tracking-[0.3em] text-muted-foreground">{ornament}</p>
      <div className="mt-4 flex flex-wrap items-center gap-4">
        <h1 className="text-3xl font-medium tracking-tight sm:text-4xl">{children}</h1>
        {planned && (
          <Badge
            variant="outline"
            className="border-ring font-mono text-[10px] tracking-widest text-brand-soft"
          >
            blueprint — not yet implemented
          </Badge>
        )}
      </div>
    </header>
  );
}

export function Lead({ children }: { children: ReactNode }) {
  return (
    <p className="mb-8 max-w-2xl font-serif text-xl italic leading-relaxed text-muted-foreground">
      {children}
    </p>
  );
}

export function H2({ id, children }: { id?: string; children: ReactNode }) {
  return (
    <h2 id={id} className="mb-4 mt-12 scroll-mt-24 text-xl font-medium tracking-tight">
      {children}
    </h2>
  );
}

export function H3({ children }: { children: ReactNode }) {
  return <h3 className="mb-3 mt-8 font-mono text-sm text-brand-soft">{children}</h3>;
}

export function P({ children }: { children: ReactNode }) {
  return <p className="mb-4 max-w-2xl text-sm leading-7 text-foreground/90">{children}</p>;
}

export function Ul({ children }: { children: ReactNode }) {
  return (
    <ul className="mb-4 max-w-2xl list-none space-y-2 text-sm leading-7 text-foreground/90">
      {children}
    </ul>
  );
}

export function Li({ children }: { children: ReactNode }) {
  return (
    <li className="flex gap-3">
      <span aria-hidden="true" className="select-none text-brand-soft">
        →
      </span>
      <span>{children}</span>
    </li>
  );
}

export function Code({ children }: { children: ReactNode }) {
  return (
    <code className="rounded bg-secondary px-1.5 py-0.5 font-mono text-[0.85em] text-foreground">
      {children}
    </code>
  );
}

export function Callout({
  children,
  tone = "note",
}: {
  children: ReactNode;
  tone?: "note" | "planned";
}) {
  return (
    <aside
      className={cn(
        "my-6 max-w-2xl rounded-lg border px-4 py-3 text-sm leading-6",
        tone === "planned"
          ? "border-ring/60 bg-brand/5 text-foreground/90"
          : "border-border bg-card/50 text-muted-foreground"
      )}
    >
      <span aria-hidden="true" className="mr-2 select-none text-brand-soft">
        ✧
      </span>
      {children}
    </aside>
  );
}

export function Table({
  head,
  rows,
}: {
  head: string[];
  rows: ReactNode[][];
}) {
  return (
    <div className="my-6 max-w-3xl overflow-x-auto rounded-lg border border-border">
      <table className="w-full text-left text-sm">
        <thead>
          <tr className="border-b border-border bg-card/60">
            {head.map((cell) => (
              <th key={cell} className="px-4 py-2 font-mono text-xs font-normal text-muted-foreground">
                {cell}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {rows.map((row, rowIndex) => (
            <tr key={rowIndex} className="border-b border-border/50 last:border-0">
              {row.map((cell, cellIndex) => (
                <td key={cellIndex} className="px-4 py-2 align-top leading-6">
                  {cell}
                </td>
              ))}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
