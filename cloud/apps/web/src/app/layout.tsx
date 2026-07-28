import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "Ghost",
  description: "AI operator that learns your workflows and runs them across your software.",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body className="min-h-screen">{children}</body>
    </html>
  );
}
