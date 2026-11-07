import js from "@eslint/js";
import { defineConfig } from "eslint/config";
import * as mdx from "eslint-plugin-mdx";
import globals from "globals";
import tseslint from "typescript-eslint";

export default defineConfig(
    {
        ignores: ["node_modules/**", ".docusaurus/**", "build/**"]
    },
    {
        files: ["**/*.{js,mjs,cjs}"],
        extends: [js.configs.recommended],
        languageOptions: {
            globals: globals.node
        }
    },
    {
        files: ["**/*.{ts,tsx}"],
        extends: [js.configs.recommended, tseslint.configs.strictTypeChecked, tseslint.configs.stylisticTypeChecked],
        languageOptions: {
            parserOptions: {
                projectService: true,
                tsconfigRootDir: import.meta.dirname
            },
            globals: { ...globals.node, ...globals.browser }
        },
        rules: {
            "@typescript-eslint/consistent-type-imports": "error"
        }
    },
    {
        ...mdx.flat,
        files: ["docs/**/*.mdx"]
    }
);
