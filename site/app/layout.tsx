import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "Seshat",
  description:
    "CLI para commits automatizados com Conventional Commits, flow por arquivo e modelo local no Ollama.",
  openGraph: {
    title: "Seshat",
    description:
      "CLI para commits automatizados com Conventional Commits, flow por arquivo e modelo local no Ollama.",
    type: "website"
  }
};

export default function RootLayout({
  children
}: Readonly<{
  children: React.ReactNode;
}>) {
  return (
    <html lang="pt-BR" dir="ltr" suppressHydrationWarning>
      <body>{children}</body>
    </html>
  );
}
