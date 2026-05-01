import nextra from "nextra";

const withNextra = nextra({
  contentDirBasePath: "/docs"
});

/** @type {import('next').NextConfig} */
const nextConfig = {
  reactStrictMode: true,
  turbopack: {
    resolveAlias: {
      "next-mdx-import-source-file": "./mdx-components.tsx"
    }
  }
};

export default withNextra(nextConfig);
