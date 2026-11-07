import type * as Preset from "@docusaurus/preset-classic";
import type { Config } from "@docusaurus/types";
import { themes } from "prism-react-renderer";

const config: Config = {
    title: "Intrlay",
    tagline: "Typed buffer views across languages",
    favicon: "img/favicon.ico",
    url: "https://intrlay.github.io",
    baseUrl: "/intrlay/",
    trailingSlash: true,
    organizationName: "intrlay",
    projectName: "intrlay",
    onBrokenLinks: "throw",
    onBrokenAnchors: "throw",
    markdown: {
        hooks: {
            onBrokenMarkdownLinks: "throw"
        }
    },
    presets: [
        [
            "classic",
            {
                docs: {
                    routeBasePath: "/",
                    include: ["**/*.mdx"],
                    sidebarPath: "./sidebars.ts",
                    editUrl: "https://github.com/intrlay/intrlay/edit/main/docs/"
                },
                blog: false,
                pages: false,
                theme: {
                    customCss: "./src/css/custom.css"
                }
            } satisfies Preset.Options
        ]
    ],
    themeConfig: {
        colorMode: {
            respectPrefersColorScheme: true
        },
        navbar: {
            logo: {
                alt: "Intrlay",
                src: "img/logo-wordmark.svg",
                srcDark: "img/logo-wordmark-dark.svg",
                width: 106,
                height: 32
            },
            items: [
                {
                    type: "docSidebar",
                    sidebarId: "docsSidebar",
                    label: "Docs",
                    position: "left"
                },
                {
                    href: "https://github.com/intrlay/intrlay",
                    label: "GitHub",
                    position: "right"
                }
            ]
        },
        prism: {
            theme: themes.github,
            darkTheme: themes.dracula,
            additionalLanguages: ["rust", "cpp", "csharp", "python", "dart", "toml", "cmake", "powershell"]
        }
    } satisfies Preset.ThemeConfig
};

export default config;
