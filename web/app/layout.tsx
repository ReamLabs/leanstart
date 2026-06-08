import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "leanstart devnets",
  description: "Every Lean Ethereum devnet run, tracked and filterable.",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="en">
      <body>
        <header className="topbar">
          <a href="/" className="brand">
            <span className="logo">⬡</span>
            leanstart <span className="sub">devnets</span>
          </a>
          <nav>
            <a href="/">Runs</a>
          </nav>
          <span className="spacer" />
          <span className="meta">Lean Ethereum devnet tracker</span>
        </header>
        <main>{children}</main>
      </body>
    </html>
  );
}
