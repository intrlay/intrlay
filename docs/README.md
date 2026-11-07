# Documentation site

This directory contains Intrlay's Docusaurus site. All published content lives
in `docs/` as MDX. The documentation overview is the home page; there are no
separate pages, blog, or translations.

Use Node.js 24 or later. From this directory:

```sh
npm ci
npm start
```

The development site is available at `http://localhost:3000/intrlay/`.

```sh
npm run check
npm run serve
```

`check` runs ESLint on the site code and MDX, TypeScript checking, and a production
build with broken-link checks. `serve` previews that build locally. Generated
files in `build/` and `.docusaurus/` are ignored.

Edit `sidebars.ts` when adding or moving a document. Use relative `.mdx` links
between documents so Docusaurus can resolve and validate their routes.

The reading order is quick start, a complete drawing tutorial, concepts,
language guides, then reference. Concepts explain behavior through examples;
language guides reuse the drawing to show each target's actual API. Reference
pages document exact options and include their visible effects. Design and
implementation guidance lives under `docs/contributing/`.
