import Link from "next/link";
import type { AnchorHTMLAttributes, ReactNode } from "react";
import type { MDXComponents } from "mdx/types";

type DocsLinkProps = AnchorHTMLAttributes<HTMLAnchorElement> & {
  children?: ReactNode;
  href?: string;
};

function DocsLink({ children, href = "", ...props }: DocsLinkProps) {
  if (href.startsWith("/")) {
    return <Link href={href}>{children}</Link>;
  }

  return (
    <a href={href} {...props}>
      {children}
    </a>
  );
}

export function useMDXComponents(components: MDXComponents = {}): MDXComponents {
  return {
    a: DocsLink,
    ...components
  };
}
