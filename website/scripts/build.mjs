import { cp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";
import { createHash } from "node:crypto";
import { content } from "../src/content.mjs";
import { featureSections } from "../src/feature-sections.mjs";

const root = fileURLToPath(new URL("../", import.meta.url));
const out = resolve(root, "dist");
// One content version keeps HTML, CSS, scripts and the screenshot in sync on refresh.
const assetFiles = [
  "styles.css",
  "features.css",
  "site.js",
  "hero-aurora.js",
  "theme.js",
  "releases.mjs",
];
const assetHash = createHash("sha256");
for (const name of assetFiles)
  assetHash.update(await readFile(resolve(root, "src", name)));
assetHash.update(
  await readFile(resolve(root, "public/workbench-screenshot.png")),
);
for (const name of ["claude.svg", "openai.svg", "opencode.svg"])
  assetHash.update(await readFile(resolve(root, "public/agents", name)));
const assetVersion = assetHash.digest("hex").slice(0, 16);
const assetUrl = (name) => `/${name}?v=${assetVersion}`;
const github = "https://github.com/tyuan511/apifuck";
const origin = "https://apifuck.fastvibe.dev";
const esc = (s) =>
  String(s).replace(
    /[&<>"']/g,
    (c) =>
      ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[
        c
      ],
  );
const lines = (s) => esc(s).replaceAll("\n", "<br>");
const arrow = '<span aria-hidden="true">↗</span>';
const brand =
  '<a class="brand" href="#top"><img src="/apifuck.png" width="32" height="32" alt=""><span>ApiFuck</span></a>';
function page(t, locale) {
  const guide = `${github}/blob/main/${locale === "zh" ? "docs/development.md" : "README.md"}`;
  return `<!doctype html>
<html lang="${t.lang}">
<head>
<meta charset="UTF-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>${esc(t.title)}</title><meta name="description" content="${esc(t.description)}">
<meta name="color-scheme" content="light dark"><meta name="theme-color" content="#3278a7">
<meta property="og:type" content="website"><meta property="og:title" content="${esc(t.title)}"><meta property="og:description" content="${esc(t.description)}"><meta property="og:image" content="${origin}/apifuck.png"><meta property="og:url" content="${origin}/${locale === "en" ? "en/" : ""}">
<link rel="canonical" href="${origin}/${locale === "en" ? "en/" : ""}"><link rel="alternate" hreflang="zh-CN" href="${origin}/"><link rel="alternate" hreflang="en" href="${origin}/en/"><link rel="alternate" hreflang="x-default" href="${origin}/">
<link rel="icon" href="/apifuck.png"><script src="${assetUrl("theme.js")}"></script><link rel="stylesheet" href="${assetUrl("styles.css")}"><link rel="stylesheet" href="${assetUrl("features.css")}"><script src="${assetUrl("site.js")}" type="module"></script>
</head>
<body>
<a class="skip-link" href="#main">${t.skip}</a>
<header class="site-header"><div class="header-inner">${brand}
<nav id="navigation" aria-label="${t.menu}">${["native", "local", "workbench", "agents", "download"].map((id, i) => `<a href="#${id}">${t.nav[i]}</a>`).join("")}</nav>
<div class="header-actions"><div class="languages"><a href="/" lang="zh-CN" hreflang="zh-CN" ${locale === "zh" ? 'aria-current="page"' : ""}>中文</a><a href="/en/" lang="en" hreflang="en" ${locale === "en" ? 'aria-current="page"' : ""}>EN</a></div>
<button class="theme-toggle" aria-label="${t.theme}" title="${t.theme}" hidden><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" aria-hidden="true"><path d="M20.5 13A8.5 8.5 0 0 1 11 3.5 8.5 8.5 0 1 0 20.5 13Z"/></svg></button>
<a class="github-link" href="${github}" aria-label="GitHub"><svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M12 2a10 10 0 0 0-3.16 19.49c.5.09.68-.22.68-.48v-1.86c-2.78.6-3.37-1.18-3.37-1.18-.45-1.16-1.11-1.47-1.11-1.47-.91-.62.07-.61.07-.61 1 .07 1.53 1.03 1.53 1.03.89 1.53 2.34 1.09 2.91.83.09-.64.35-1.09.64-1.34-2.22-.25-4.56-1.11-4.56-4.94 0-1.09.39-1.99 1.03-2.69-.1-.26-.45-1.27.1-2.65 0 0 .84-.27 2.75 1.03A9.6 9.6 0 0 1 12 6.82c.85 0 1.71.11 2.51.34 1.91-1.3 2.75-1.03 2.75-1.03.55 1.38.2 2.39.1 2.65.64.7 1.03 1.6 1.03 2.69 0 3.84-2.34 4.69-4.57 4.94.36.31.68.92.68 1.85v2.75c0 .27.18.58.69.48A10 10 0 0 0 12 2Z"/></svg></a>
<button class="menu-toggle" aria-expanded="false" aria-controls="navigation" aria-label="${t.menu}" hidden>☰</button></div></div></header>
<main id="main">
<section class="hero" id="top"><div class="hero-backdrop" aria-hidden="true"><canvas class="hero-aurora" width="1" height="1" aria-hidden="true"></canvas><div class="hero-flow" data-active="false" aria-hidden="true"><div class="hero-ribbon"></div><div class="hero-ribbon"></div></div></div>
<div class="hero-copy"><p class="eyebrow hero-eyebrow"><span></span>${t.eyebrow}</p><h1 class="hero-title"><span class="sr-only">${esc(t.headlineAccessible)}</span><span class="hero-heading" aria-hidden="true">${esc(t.headline)}</span><span class="typewriter-stage" data-typewriter data-phrases="${esc(JSON.stringify(t.headlinePhrases))}" aria-hidden="true">${t.headlinePhrases.map((phrase) => `<span class="typewriter-reserve">${esc(phrase)}</span>`).join("")}<span class="typewriter-output"><span data-typewriter-text>${esc(t.headlinePhrases[0])}</span><span class="typewriter-cursor"></span></span></span></h1><p class="hero-lead">${lines(t.lead)}</p><div class="hero-actions"><a class="button hero-button" href="#download">${t.download}<span aria-hidden="true">↓</span></a><a class="hero-source" href="${github}">${t.source} ${arrow}</a></div><p class="platform-hint">${t.platforms}</p></div>
<figure class="preview-wrap"><a class="screenshot-link" href="${assetUrl("workbench-screenshot.png")}" target="_blank" rel="noreferrer"><img class="workbench-screenshot" src="${assetUrl("workbench-screenshot.png")}" width="2560" height="1680" alt="${esc(t.previewAlt)}" fetchpriority="high" decoding="async"></a><figcaption class="preview-caption">${t.preview}</figcaption></figure></section>
<div class="trust-strip wrap">${t.strip.map((s, i) => `<span><i aria-hidden="true">${["◈", "◎", "⌘", "↗"][i]}</i>${s}</span>`).join("")}</div>
${featureSections(t, { esc, lines, github, assetUrl })}
<section class="download-section" id="download"><div class="wrap"><div class="section-heading"><p class="eyebrow">${t.downloadEyebrow}</p><h2>${t.downloadTitle}</h2><p>${t.downloadLead}</p></div><div class="download-grid">${t.os.map(([os, arch], i) => `<article class="download-card"><div class="os-icon" aria-hidden="true">${["⌘", "⊞", ">_"][i]}</div><h3>${os}</h3><p>${arch}</p><a class="button download-button" data-platform="${["mac", "windows", "linux"][i]}" href="${github}/releases">${t.releaseLink} ${arrow}</a><span class="release-status" role="status">${t.unavailable}</span></article>`).join("")}</div><p class="release-note">${t.releaseNote}</p><div class="download-links"><a href="${guide}">${t.sourceGuide} ${arrow}</a><a href="${github}/releases">${t.releaseLink} ${arrow}</a></div></div></section>
<section class="faq-section wrap"><h2>${t.faqTitle}</h2><div>${t.faqs.map(([q, a]) => `<details><summary>${q}<span aria-hidden="true">＋</span></summary><p>${a}</p></details>`).join("")}</div></section>
</main><footer class="wrap"><div class="footer-main"><div>${brand}<p>${t.footer}</p></div><div class="footer-links"><div><h2>${t.product}</h2><a href="#native">${t.nav[0]}</a><a href="#agents">${t.nav[3]}</a><a href="#download">${t.nav[4]}</a></div><div><h2>${t.resources}</h2><a href="${guide}">${t.docs}</a><a href="${github}/releases">${t.changelog}</a></div><div><h2>${t.community}</h2><a href="${github}">GitHub</a><a href="${github}/issues">${t.issues}</a></div></div></div><div class="footer-bottom"><span>© ${new Date().getUTCFullYear()} ApiFuck</span><a href="${github}/blob/main/LICENSE">${t.license} ${arrow}</a><span>Built with Rust. Made for your flow.</span></div></footer>
</body></html>`;
}
await rm(out, { recursive: true, force: true });
await mkdir(resolve(out, "en"), { recursive: true });
await cp(resolve(root, "public"), out, { recursive: true });
for (const name of assetFiles) {
  if (name === "site.js") {
    const source = await readFile(resolve(root, "src", name), "utf8");
    await writeFile(
      resolve(out, name),
      source
        .replace('"./releases.mjs"', `"./releases.mjs?v=${assetVersion}"`)
        .replace('"./hero-aurora.js"', `"./hero-aurora.js?v=${assetVersion}"`),
    );
  } else await cp(resolve(root, "src", name), resolve(out, name));
}
for (const [locale, t] of Object.entries(content))
  await writeFile(
    resolve(out, locale === "en" ? "en/index.html" : "index.html"),
    page(t, locale),
  );
await writeFile(
  resolve(out, "robots.txt"),
  `User-agent: *\nAllow: /\nSitemap: ${origin}/sitemap.xml\n`,
);
await writeFile(
  resolve(out, "sitemap.xml"),
  `<?xml version="1.0" encoding="UTF-8"?><urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9"><url><loc>${origin}/</loc></url><url><loc>${origin}/en/</loc></url></urlset>`,
);
const revision = process.env.SITE_REVISION || "local";
await writeFile(
  resolve(out, "health.json"),
  JSON.stringify({ status: "ok", revision }),
);
console.log(`Built Chinese and English pages in ${out}`);
