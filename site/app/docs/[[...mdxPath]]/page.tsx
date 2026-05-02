import { importPage } from "nextra/pages";
import { notFound } from "next/navigation";

type DocsPageProps = {
	params: Promise<{
		mdxPath?: string[];
	}>;
};

export const dynamic = "force-dynamic";

export async function generateMetadata(props: DocsPageProps) {
	const params = await props.params;
	try {
		const { metadata } = await importPage(params.mdxPath);
		return metadata;
	} catch {
		return {
			title: "Documentacao | Seshat"
		};
	}
}

export default async function DocsPage(props: DocsPageProps) {
	const params = await props.params;
	let page;

	try {
		page = await importPage(params.mdxPath);
	} catch {
		notFound();
	}

	const { default: MDXContent } = page;

	return (
		<article className="docsArticle">
			<MDXContent {...props} params={params} />
		</article>
	);
}
