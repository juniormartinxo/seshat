import type { Metadata } from "next";
import "./globals.css";

const themeScript = `
try {
  const theme = localStorage.getItem("seshat-theme") === "light" ? "light" : "dark";
  document.documentElement.classList.toggle("light", theme === "light");
  document.documentElement.dataset.theme = theme;
} catch {}
`;

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
		<html lang="pt-BR" dir="ltr" data-scroll-behavior="smooth" suppressHydrationWarning>
			<head>
				<script dangerouslySetInnerHTML={{ __html: themeScript }} />
			</head>
			<body>{children}</body>
		</html>
	);
}
