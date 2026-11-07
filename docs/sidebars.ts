import type { SidebarsConfig } from "@docusaurus/plugin-content-docs";

const sidebars: SidebarsConfig = {
    docsSidebar: [
        { type: "doc", id: "index", label: "Overview" },
        "feature-comparison",
        "getting-started",
        "tutorial",
        {
            type: "category",
            label: "Concepts",
            items: [
                "concepts/buffers-and-views",
                "concepts/mutation",
                "concepts/storage"
            ]
        },
        {
            type: "category",
            label: "Language guides",
            items: [
                "languages/rust",
                "languages/cpp",
                "languages/typescript",
                "languages/csharp",
                "languages/python",
                "languages/dart"
            ]
        },
        {
            type: "category",
            label: "Reference",
            items: [
                "reference/code-generation",
                "reference/annotations",
                "reference/naming",
                "reference/binary-layout"
            ]
        },
        {
            type: "category",
            label: "Contributing",
            items: [
                "contributing/development",
                "contributing/design-principles",
                "contributing/code-style"
            ]
        }
    ]
};

export default sidebars;
