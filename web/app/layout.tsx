import type { Metadata, Viewport } from "next";
import { Cormorant_Garamond, JetBrains_Mono, Space_Grotesk } from "next/font/google";
import "./globals.css";

const grotesk = Space_Grotesk({
  variable: "--font-grotesk",
  subsets: ["latin"],
});

const cormorant = Cormorant_Garamond({
  variable: "--font-cormorant",
  subsets: ["latin"],
  weight: ["400", "500", "600"],
  style: ["normal", "italic"],
});

const jetbrains = JetBrains_Mono({
  variable: "--font-jetbrains",
  subsets: ["latin"],
});

export const metadata: Metadata = {
  title: "✧ loop — the sdk for the modern age ✧",
  description:
    "loop is a development kit for building apps in the modern age of AI — statically typed backends, live endpoints over SSE, auth, payments, and an ethereum sdk, focused on decentralization.",
  openGraph: {
    title: "✧ loop — the sdk for the modern age ✧",
    description:
      "A development kit for building apps in the modern age of AI, focused on decentralization.",
    siteName: "loop sdk",
    type: "website",
  },
};

export const viewport: Viewport = {
  themeColor: "#0b090e",
  colorScheme: "dark",
};

export default function RootLayout({
  children,
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html
      lang="en"
      className={`${grotesk.variable} ${cormorant.variable} ${jetbrains.variable} h-full antialiased`}
    >
      <body className="min-h-full flex flex-col bg-background text-foreground">
        {children}
      </body>
    </html>
  );
}
