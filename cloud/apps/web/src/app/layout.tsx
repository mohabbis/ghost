import type { Metadata } from "next";
import { Fraunces, IBM_Plex_Mono } from "next/font/google";
import "./globals.css";

// Self-hosted at build time via next/font — no runtime request to Google
// Fonts, so this carries no CSP or third-party-tracking implication. Only
// the landing page (app/page.tsx) reaches for these; every authenticated
// view keeps the plain --font-sans stack.
const fraunces = Fraunces({
  subsets: ["latin"],
  variable: "--font-fraunces",
  style: ["normal", "italic"],
  axes: ["opsz", "SOFT", "WONK"],
});
const plexMono = IBM_Plex_Mono({
  subsets: ["latin"],
  weight: ["400", "500"],
  variable: "--font-plex-mono",
});

export const metadata: Metadata = {
  title: "Ghost",
  description: "AI operator that learns your workflows and runs them across your software.",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en" className={`${fraunces.variable} ${plexMono.variable}`}>
      <body className="min-h-screen">{children}</body>
    </html>
  );
}
