export type DocLink = {
  href: string;
  title: string;
  planned?: boolean;
};

export type DocSection = {
  number: string;
  title: string;
  links: DocLink[];
};

export const docsNav: DocSection[] = [
  {
    number: "№ 001",
    title: "manual",
    links: [
      { href: "/docs", title: "introduction" },
      { href: "/docs/quickstart", title: "quickstart" },
      { href: "/docs/schemas", title: "schemas & validation" },
      { href: "/docs/endpoints", title: "endpoints" },
      { href: "/docs/database", title: "database" },
      { href: "/docs/eth", title: "ethereum sdk" },
      { href: "/docs/cli", title: "cli & manifest" },
    ],
  },
  {
    number: "№ 002",
    title: "blueprints",
    links: [
      { href: "/docs/auth", title: "authentication", planned: true },
      { href: "/docs/payments", title: "payments & subscriptions", planned: true },
    ],
  },
];

export function flatDocsNav(): DocLink[] {
  return docsNav.flatMap((section) => section.links);
}
