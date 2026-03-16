import react from "@astrojs/react";
import starlight from "@astrojs/starlight";
import { defineConfig } from "astro/config";
import starlightLlmsTxt from "starlight-llms-txt";

export default defineConfig({
	site: "https://appz.dev/docs",
	server: { port: 4323 },
	prefetch: true,
	integrations: [
		starlight({
			title: "appz",
			logo: {
				src: "./src/assets/logo.svg",
				replacesTitle: true,
			},
			lastUpdated: true,
			favicon: "/favicon.svg",
			tableOfContents: { minHeadingLevel: 2, maxHeadingLevel: 3 },
			customCss: ["./src/styles/custom.css"],
			head: [
				{
					tag: "script",
					content: `localStorage.setItem('starlight-theme','light');document.documentElement.dataset.theme='light';`,
				},
			],
			plugins: [starlightLlmsTxt({ rawContent: true })],
			sidebar: [
				{
					label: "Getting Started",
					items: [
						"docs/getting-started",
						"docs/getting-started/quick-start",
						"docs/getting-started/installation",
					],
				},
				{
					label: "CLI",
					items: [
						"docs/cli/commands",
						"docs/cli/configuration",
					],
				},
				{
					label: "Guides",
					items: [
						"docs/guides/wordpress",
						"docs/guides/static-sites",
						"docs/guides/frameworks",
					],
				},
				{
					label: "Concepts",
					items: [
						"docs/concepts/init",
						"docs/concepts/dev",
						"docs/concepts/build",
						"docs/concepts/deploy",
					],
				},
				{
					label: "Changelog",
					items: [
						"docs/changelog",
					],
				},
			],
		}),
		react(),
	],
});
