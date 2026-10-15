import { cp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { fileURLToPath } from "node:url";
import { resolve } from "node:path";
import { content } from "../src/content.mjs";

const root = fileURLToPath(new URL("../", import.meta.url));
const out = resolve(root, "dist");
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
  const code =
    "info:\n  name: Get users\n  type: http\nhttp:\n  method: GET\n  url: '{{base_url}}/users'\n  params:\n    - name: limit\n      value: '10'\n      type: query";
  return `<!doctype html>
<html lang="${t.lang}">
<head>
<meta charset="UTF-8"><meta name="viewport" content="width=device-width, initial-scale=1">
<title>${esc(t.title)}</title><meta name="description" content="${esc(t.description)}">
<meta name="color-scheme" content="light dark"><meta name="theme-color" content="#3262d8">
<meta property="og:type" content="website"><meta property="og:title" content="${esc(t.title)}"><meta property="og:description" content="${esc(t.description)}"><meta property="og:image" content="${origin}/apifuck.png"><meta property="og:url" content="${origin}/${locale === "en" ? "en/" : ""}">
<link rel="canonical" href="${origin}/${locale === "en" ? "en/" : ""}"><link rel="alternate" hreflang="zh-CN" href="${origin}/"><link rel="alternate" hreflang="en" href="${origin}/en/"><link rel="alternate" hreflang="x-default" href="${origin}/">
<link rel="icon" href="/apifuck.png"><script src="/theme.js"></script><link rel="stylesheet" href="/styles.css"><script src="/site.js" type="module"></script>
</head>
<body>
<a class="skip-link" href="#main">${t.skip}</a>
<header class="site-header"><div class="header-inner">${brand}
<nav id="navigation" aria-label="${t.menu}">${["workbench", "local", "download"].map((id, i) => `<a href="#${id}">${t.nav[i]}</a>`).join("")}</nav>
<div class="header-actions"><div class="languages"><a href="/" lang="zh-CN" hreflang="zh-CN" ${locale === "zh" ? 'aria-current="page"' : ""}>中文</a><a href="/en/" lang="en" hreflang="en" ${locale === "en" ? 'aria-current="page"' : ""}>EN</a></div>
<button class="theme-toggle" aria-label="${t.theme}" title="${t.theme}" hidden><svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.6" aria-hidden="true"><path d="M20.5 13A8.5 8.5 0 0 1 11 3.5 8.5 8.5 0 1 0 20.5 13Z"/></svg></button>
<a class="github-link" href="${github}" aria-label="GitHub"><svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><path d="M12 2a10 10 0 0 0-3.16 19.49c.5.09.68-.22.68-.48v-1.86c-2.78.6-3.37-1.18-3.37-1.18-.45-1.16-1.11-1.47-1.11-1.47-.91-.62.07-.61.07-.61 1 .07 1.53 1.03 1.53 1.03.89 1.53 2.34 1.09 2.91.83.09-.64.35-1.09.64-1.34-2.22-.25-4.56-1.11-4.56-4.94 0-1.09.39-1.99 1.03-2.69-.1-.26-.45-1.27.1-2.65 0 0 .84-.27 2.75 1.03A9.6 9.6 0 0 1 12 6.82c.85 0 1.71.11 2.51.34 1.91-1.3 2.75-1.03 2.75-1.03.55 1.38.2 2.39.1 2.65.64.7 1.03 1.6 1.03 2.69 0 3.84-2.34 4.69-4.57 4.94.36.31.68.92.68 1.85v2.75c0 .27.18.58.69.48A10 10 0 0 0 12 2Z"/></svg></a>
<button class="menu-toggle" aria-expanded="false" aria-controls="navigation" aria-label="${t.menu}" hidden>☰</button></div></div></header>
<main id="main">
<section class="hero" id="top"><div class="hero-backdrop" aria-hidden="true"></div>
<div class="hero-copy"><p class="eyebrow hero-eyebrow"><span></span>${t.eyebrow}</p><h1>${t.headline.map((x) => `<span>${esc(x)}</span>`).join("")}</h1><p class="hero-lead">${lines(t.lead)}</p><div class="hero-actions"><a class="button hero-button" href="#download">${t.download}<span aria-hidden="true">↓</span></a><a class="hero-source" href="${github}">${t.source} ${arrow}</a></div><p class="platform-hint">${t.platforms}</p></div>
<div class="preview-wrap"><div class="workbench" aria-label="${t.preview}">
<div class="window-bar"><div class="traffic" aria-hidden="true"><i></i><i></i><i></i></div><span>ApiFuck <span class="window-divider">/</span> Playground</span><span class="window-environment"><i></i> Local</span></div>
<div class="workbench-body"><aside class="collection-tree"><div class="tree-heading">${t.collection}<span>＋</span></div><div class="tree-project">▾ &nbsp; Playground</div><div class="tree-folder">▾ &nbsp; users</div><div class="tree-item active"><b>GET</b>${t.request}</div><div class="tree-item"><b class="post">POST</b>Create user</div><div class="tree-item"><b class="gql">GQL</b>GraphQL</div><div class="tree-item"><b>GET</b>Event stream</div><div class="tree-item"><b class="gql">WS</b>Echo socket</div><div class="tree-bottom"><span>◉ &nbsp; ${t.environment}</span><span>⚙ &nbsp; Playground</span></div></aside>
<div class="request-pane"><div class="request-tab"><b>GET</b> ${t.request}<span>×</span></div><div class="request-editor"><div class="url-row"><span class="method">GET</span><code><span>{{base_url}}</span>/users</code><button class="send-button" data-send="${t.send}" data-sending="${t.sending}" hidden>${t.send}<span>↵</span></button></div>
<div class="request-tabs" role="tablist" aria-label="${t.request}">${[t.params, t.headers, t.tests].map((name, i) => `<button role="tab" id="request-tab-${i}" aria-controls="request-panel-${i}" aria-selected="${i === 0}" tabindex="${i === 0 ? "0" : "-1"}" data-request-tab="${i}">${name}${i === 0 ? " <small>1</small>" : ""}</button>`).join("")}</div>
<div class="request-panel" role="tabpanel" id="request-panel-0" aria-labelledby="request-tab-0"><div class="kv-row kv-head"><span></span><span>${t.key}</span><span>${t.value}</span></div><div class="kv-row"><span class="check">✓</span><code>limit</code><code>10</code></div></div>
<div class="request-panel" role="tabpanel" id="request-panel-1" aria-labelledby="request-tab-1" hidden><div class="kv-row kv-head"><span></span><span>${t.key}</span><span>${t.value}</span></div><div class="kv-row"><span class="check">✓</span><code>Accept</code><code>application/json</code></div></div>
<div class="request-panel" role="tabpanel" id="request-panel-2" aria-labelledby="request-tab-2" hidden><pre><code>test("status is 200", () =&gt; {
  expect(api.response.status).to.equal(200);
});</code></pre></div>
<div class="response-bar"><span>${t.body}</span><div><b>200 OK</b><span>24 ms</span><span>128 B</span></div></div>
<div class="response-code"><div class="line-numbers" aria-hidden="true">1<br>2<br>3<br>4<br>5<br>6<br>7</div><pre><code>{
  <span class="syntax-key">"users"</span>: [
    { <span class="syntax-key">"id"</span>: <span class="syntax-number">1</span>, <span class="syntax-key">"name"</span>: <span class="syntax-string">"Alex"</span> },
    { <span class="syntax-key">"id"</span>: <span class="syntax-number">2</span>, <span class="syntax-key">"name"</span>: <span class="syntax-string">"Sam"</span> }
  ],
  <span class="syntax-key">"total"</span>: <span class="syntax-number">2</span>
}</code></pre></div><div class="response-footer"><span class="test-pass">✓ ${t.testPass}</span><span id="demo-status" role="status" aria-live="polite">${t.demo}</span></div></div></div></div></div><p class="preview-caption">${t.preview} <span>— ${t.demoNote}</span></p></div></section>
<div class="trust-strip wrap">${t.strip.map((s, i) => `<span><i aria-hidden="true">${["◈", "◎", "⌘", "↗"][i]}</i>${s}</span>`).join("")}</div>
<section class="section wrap" id="workbench"><div class="section-heading"><p class="eyebrow">${t.workflowEyebrow}</p><h2>${lines(t.workflowTitle)}</h2><p>${t.workflowLead}</p></div>
<div class="protocol-tabs" role="tablist" aria-label="${t.nav[0]}">${t.protocols.map((p, i) => `<button role="tab" id="protocol-tab-${i}" aria-selected="${i === 0}" tabindex="${i === 0 ? "0" : "-1"}" aria-controls="protocol-panel-${i}" data-protocol-tab="${i}">${p[0]}</button>`).join("")}</div>
${t.protocols.map((p, i) => `<div class="protocol-panel" role="tabpanel" id="protocol-panel-${i}" aria-labelledby="protocol-tab-${i}" ${i ? "hidden" : ""}><div class="protocol-copy"><span class="protocol-index">0${i + 1} / 04</span><h3>${p[1]}</h3><p>${p[2]}</p><a class="text-link" href="${github}/blob/main/docs/execution.md">${t.docs} ${arrow}</a></div><div class="protocol-media"><div class="mini-window"><div class="mini-url"><b>${p[3]}</b><code>${esc(p[4])}</code><span aria-hidden="true">↵</span></div><pre><code>${esc(p[5])}</code></pre><div class="mini-status"><i></i>${p[0]} <span>${t.demo}</span></div></div></div></div>`).join("")}</section>
<section class="local-section" id="local"><div class="wrap local-layout"><div class="local-copy"><p class="eyebrow">${t.localEyebrow}</p><h2>${lines(t.localTitle)}</h2><p class="section-lead">${t.localLead}</p><div class="local-points">${t.localItems.map(([title, desc]) => `<div><span aria-hidden="true">✓</span><div><h3>${title}</h3><p>${desc}</p></div></div>`).join("")}</div></div><div class="file-visual"><div class="file-tree"><span>⌄ &nbsp; my-api/</span><span>├─ opencollection.yml</span><span>├─ environments/</span><span class="selected">├─ users/get-users.yml</span><span class="muted">└─ .apifuck/</span></div><div class="yaml-window"><div class="yaml-heading"><span class="file-dot"></span>get-users.yml <span>YAML</span></div><pre><code>${esc(code)}</code></pre><div class="git-status"><span>＋ 10</span> &nbsp; <span>− 0</span><span>main*</span></div></div><p>${t.fileCaption}</p></div></div></section>
<section class="section wrap tools-section"><div class="section-heading"><p class="eyebrow">${t.toolsEyebrow}</p><h2>${t.toolsTitle}</h2></div><div class="tool-grid">${t.tools.map(([n, title, desc, code]) => `<article class="tool"><div class="tool-art"><pre><code>${esc(code)}</code></pre></div><span class="tool-number">${n}</span><h3>${title}</h3><p>${desc}</p></article>`).join("")}</div></section>
<section class="download-section" id="download"><div class="wrap"><div class="section-heading"><p class="eyebrow">${t.downloadEyebrow}</p><h2>${t.downloadTitle}</h2><p>${t.downloadLead}</p></div><div class="download-grid">${t.os.map(([os, arch], i) => `<article class="download-card"><div class="os-icon" aria-hidden="true">${["⌘", "⊞", ">_"][i]}</div><h3>${os}</h3><p>${arch}</p><a class="button download-button" data-platform="${["mac", "windows", "linux"][i]}" href="${github}/releases">${t.releaseLink} ${arrow}</a><span class="release-status" role="status">${t.unavailable}</span></article>`).join("")}</div><p class="release-note">${t.releaseNote}</p><div class="download-links"><a href="${guide}">${t.sourceGuide} ${arrow}</a><a href="${github}/releases">${t.releaseLink} ${arrow}</a></div></div></section>
<section class="faq-section wrap"><h2>${t.faqTitle}</h2><div>${t.faqs.map(([q, a]) => `<details><summary>${q}<span aria-hidden="true">＋</span></summary><p>${a}</p></details>`).join("")}</div></section>
</main><footer class="wrap"><div class="footer-main"><div>${brand}<p>${t.footer}</p></div><div class="footer-links"><div><h2>${t.product}</h2><a href="#workbench">${t.nav[0]}</a><a href="#download">${t.nav[2]}</a></div><div><h2>${t.resources}</h2><a href="${guide}">${t.docs}</a><a href="${github}/releases">${t.changelog}</a></div><div><h2>${t.community}</h2><a href="${github}">GitHub</a><a href="${github}/issues">${t.issues}</a></div></div></div><div class="footer-bottom"><span>© ${new Date().getUTCFullYear()} ApiFuck</span><a href="${github}/blob/main/LICENSE">${t.license} ${arrow}</a><span>Built with Rust. Made for your flow.</span></div></footer>
</body></html>`;
}
await rm(out, { recursive: true, force: true });
await mkdir(resolve(out, "en"), { recursive: true });
await cp(resolve(root, "public"), out, { recursive: true });
for (const name of ["styles.css", "site.js", "theme.js", "releases.mjs"])
  await cp(resolve(root, "src", name), resolve(out, name));
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
