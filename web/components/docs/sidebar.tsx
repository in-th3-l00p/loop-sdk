"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { docsNav } from "@/lib/docs-nav";
import { cn } from "@/lib/utils";

export function DocsSidebar() {
  const pathname = usePathname();

  return (
    <aside className="w-full shrink-0 lg:w-56">
      <nav className="lg:sticky lg:top-10">
        {docsNav.map((section) => (
          <div key={section.title} className="mb-8">
            <p className="font-mono text-[10px] tracking-[0.3em] text-muted-foreground">
              {section.number}
            </p>
            <p className="mt-1 font-serif text-lg italic text-foreground">{section.title}</p>
            <ul className="mt-3 space-y-1 border-l border-border pl-4">
              {section.links.map((link) => {
                const active = pathname === link.href;
                return (
                  <li key={link.href}>
                    <Link
                      href={link.href}
                      className={cn(
                        "block py-1 font-mono text-xs transition-colors",
                        active
                          ? "text-brand-soft"
                          : "text-muted-foreground hover:text-foreground"
                      )}
                    >
                      {active && <span aria-hidden="true">✧ </span>}
                      {link.title}
                      {link.planned && (
                        <span className="ml-2 text-[9px] tracking-widest text-muted-foreground/70">
                          soon
                        </span>
                      )}
                    </Link>
                  </li>
                );
              })}
            </ul>
          </div>
        ))}
      </nav>
    </aside>
  );
}
