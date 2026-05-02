import Link from "next/link";
import type { ReactNode } from "react";
import { BookOpenText, GitBranch, Terminal } from "lucide-react";
import { ThemeToggle } from "../theme-toggle";

const docsLinks = [
	{ href: "/docs", label: "Inicio" },
	{ href: "/docs/cli", label: "CLI" },
	{ href: "/docs/configuracao", label: "Configuracao" },
	{ href: "/docs/seshat-examples", label: "Exemplos" },
	{ href: "/docs/seshat-commit", label: "Modelo seshat-commit" },
	{ href: "/docs/tooling-architecture", label: "Arquitetura do tooling" },
	{ href: "/docs/ui-contract", label: "Contrato de UI" },
	{ href: "/docs/ui-customization", label: "Customizacao da UI" },
	{ href: "/docs/json-contract", label: "Contrato JSONL" },
	{ href: "/docs/parity-matrix", label: "Matriz de paridade" },
	{ href: "/docs/release-checklist", label: "Checklist de release" },
	{ href: "/docs/cutover-decision", label: "Decisao Python x Rust" }
];

export default function DocsLayout({ children }: { children: ReactNode }) {
	return (
		<div className="docsShell">
			<header className="docsTopbar">
				<Link className="docsBrand" href="/">
					<Terminal aria-hidden="true" size={20} />
					<span>Seshat</span>
				</Link>
				<nav aria-label="Documentacao">
					<Link href="/">Site</Link>
					<Link href="/docs/seshat-commit">seshat-commit</Link>
					<a href="https://github.com/juniormartinxo/seshat" rel="noreferrer" target="_blank">
						<GitBranch aria-hidden="true" size={18} />
						GitHub
					</a>
					<ThemeToggle />
				</nav>
			</header>
			<div className="docsFrame">
				<aside className="docsSidebar" aria-label="Secoes da documentacao">
					<div className="docsSidebarTitle">
						<BookOpenText aria-hidden="true" size={18} />
						Documentacao
					</div>
					<nav>
						{docsLinks.map((item) => (
							<Link key={item.href} href={item.href}>
								{item.label}
							</Link>
						))}
					</nav>
				</aside>
				<main className="docsMain">{children}</main>
			</div>
		</div>
	);
}
