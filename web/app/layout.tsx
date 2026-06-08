import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "leanstart devnets",
  description: "Every Lean Ethereum devnet run, filterable.",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body>
        <header className="topbar">
          <a href="/" className="brand">
            ⬡ leanstart <span className="muted">devnets</span>
          </a>
          <nav>
            <a href="/">Runs</a>
            <a href="/compare">Compare</a>
          </nav>
        </header>
        <main>{children}</main>
      </body>
    </html>
  );
}
