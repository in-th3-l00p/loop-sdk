import type { ReactNode } from "react";
import { cn } from "@/lib/utils";

const STRING_TONE = "text-[#cdbcf5]";
const KEYWORD_TONE = "text-brand-soft";
const QUIET_TONE = "text-muted-foreground";

type Language = {
  regex: RegExp;
  tones: string[];
};

function language(parts: [string, string][]): Language {
  return {
    regex: new RegExp(parts.map(([source]) => `(${source})`).join("|"), "g"),
    tones: parts.map(([, tone]) => tone),
  };
}

const languages: Record<string, Language> = {
  rust: language([
    ["#!?\\[[^\\]]*\\]", KEYWORD_TONE],
    ["\\/\\/.*", QUIET_TONE],
    ['"(?:[^"\\\\]|\\\\.)*"', STRING_TONE],
    [
      "\\b(?:fn|struct|enum|impl|trait|let|mut|pub|use|mod|async|await|match|if|else|for|in|while|loop|return|static|const|move|dyn|where|Self|self|crate|super|as|ref|type)\\b",
      KEYWORD_TONE,
    ],
  ]),
  toml: language([
    ["#.*", QUIET_TONE],
    ["^\\[[^\\]]*\\]", KEYWORD_TONE],
    ['"(?:[^"\\\\]|\\\\.)*"', STRING_TONE],
  ]),
  bash: language([
    ["#.*", QUIET_TONE],
    ["^\\$(?=\\s)", QUIET_TONE],
    ['"(?:[^"\\\\]|\\\\.)*"', STRING_TONE],
  ]),
  json: language([
    ['"(?:[^"\\\\]|\\\\.)*"(?=\\s*:)', KEYWORD_TONE],
    ['"(?:[^"\\\\]|\\\\.)*"', STRING_TONE],
  ]),
};

function tokenizeLine(line: string, lang: string): ReactNode[] {
  const spec = languages[lang];
  if (!spec) {
    return [line];
  }

  const nodes: ReactNode[] = [];
  let cursor = 0;
  let key = 0;

  for (const match of line.matchAll(spec.regex)) {
    const index = match.index ?? 0;
    if (index > cursor) {
      nodes.push(line.slice(cursor, index));
    }
    const group = match.slice(1).findIndex((value) => value !== undefined);
    nodes.push(
      <span key={key++} className={spec.tones[group] ?? undefined}>
        {match[0]}
      </span>
    );
    cursor = index + match[0].length;
  }
  if (cursor < line.length) {
    nodes.push(line.slice(cursor));
  }
  return nodes;
}

export function CodeBlock({
  code,
  language: lang = "rust",
  title,
  className,
}: {
  code: string;
  language?: string;
  title?: string;
  className?: string;
}) {
  const lines = code.replace(/\n$/, "").split("\n");
  return (
    <figure
      className={cn(
        "my-6 overflow-hidden rounded-lg border border-border bg-card/70",
        className
      )}
    >
      {title && (
        <figcaption className="flex items-center justify-between border-b border-border px-4 py-2">
          <span className="font-mono text-xs text-muted-foreground">{title}</span>
          <span className="font-mono text-[10px] tracking-widest text-muted-foreground">
            {lang}
          </span>
        </figcaption>
      )}
      <pre className="overflow-x-auto p-4 font-mono text-[13px] leading-6">
        {lines.map((line, index) => (
          <div key={index}>{line ? tokenizeLine(line, lang) : " "}</div>
        ))}
      </pre>
    </figure>
  );
}
