// Build-time, accessible illustrations. All examples are local and illustrative.
export function featureSections(t, { esc, lines, github, assetUrl }) {
  const points = (items) =>
    `<ul class="feature-points">${items.map(([title, description]) => `<li><span class="point-mark" aria-hidden="true">↗</span><div><h3>${esc(title)}</h3><p>${esc(description)}</p></div></li>`).join("")}</ul>`;
  const head = (eyebrow, title, lead) =>
    `<p class="eyebrow">${esc(eyebrow)}</p><h2>${lines(title)}</h2><p class="section-lead">${esc(lead)}</p>`;
  const source = (href, label) => `<a href="${href}">${esc(label)} ↗</a>`;
  const panelHead = (label, extra = "") =>
    `<div class="art-heading"><span class="window-dots" aria-hidden="true"><i></i><i></i><i></i></span><span>${esc(label)}</span><span>${esc(extra)}</span></div>`;
  const scene = (name, content, label) =>
    `<figure class="feature-visual ${name}" data-motion-scene><div class="scene-art">${content}</div><figcaption>${esc(label)}</figcaption></figure>`;
  const nativeArt = `
    <div class="architecture-title">${esc(t.nativeCompare)}<span>01</span></div>
    <div class="architecture-grid">
      <div class="runtime-stack electron-stack"><span class="stack-label">${esc(t.nativeElectron)}</span><div class="runtime-layer">${esc(t.nativeLayers[0])}<span>HTML / CSS / JS</span></div><div class="runtime-layer">${esc(t.nativeLayers[1])}<span>Browser engine</span></div><div class="runtime-layer">${esc(t.nativeLayers[2])}<span>JavaScript runtime</span></div></div>
      <div class="runtime-stack native-stack"><span class="stack-label">ApiFuck</span><div class="native-chip"><div class="chip-orbit animated" aria-hidden="true"></div><img src="/apifuck.png" alt="" width="58" height="58"><strong>Rust + GPUI</strong><span>${esc(t.nativeUI)}</span></div><div class="native-core">${esc(t.nativeCore)}<span class="pulse-dot animated" aria-hidden="true"></span></div></div>
    </div>
    <div class="native-rail" aria-hidden="true"><span></span><i class="native-packet animated"></i></div>
    <div class="native-caption"><span class="status-dot"></span>${esc(t.nativeBadge)}</div>`;
  const localArt = `
    <div class="repo-window">${panelHead("my-api/", t.localSaved)}<div class="repo-body"><div class="repo-files"><span>⌄ &nbsp; my-api</span><span>├─ opencollection.yml</span><span>├─ environments/</span><span class="file-selected">├─ users.yml <b>M</b></span><span class="ignored-file">└─ .apifuck/</span></div><div class="repo-format"><span>YAML</span><strong>{ }</strong><small>OpenCollection</small></div></div></div>
    <div class="diff-window">${panelHead("users.yml", "Git diff")}<pre><code><span class="diff-context"> http:</span><span class="diff-context">   method: GET</span><span class="diff-removed">−  url: '{{base_url}}/v1/users'</span><span class="diff-added animated">+  url: '{{base_url}}/v2/users'</span></code></pre><div class="diff-summary"><span>+1 &nbsp; −1</span>${esc(t.localDiff)}</div></div>
    <div class="git-track"><svg viewBox="0 0 420 65" fill="none" aria-hidden="true"><path class="git-main" d="M20 43H400"/><path class="git-branch" d="M95 43C130 43 125 14 160 14H260C295 14 295 43 330 43"/><path class="git-travel animated" d="M95 43C130 43 125 14 160 14H260C295 14 295 43 330 43"/><g class="git-nodes"><circle cx="35" cy="43" r="5"/><circle cx="95" cy="43" r="5"/><circle cx="330" cy="43" r="5"/><circle cx="390" cy="43" r="5"/></g><g class="git-feature-nodes"><circle cx="175" cy="14" r="5"/><circle cx="245" cy="14" r="5"/></g></svg><div><code>main</code><code>feat/users-v2</code><span>✓ commit</span></div></div>
    <div class="local-private"><span aria-hidden="true">⌁</span><code>.apifuck/</code><span>${esc(t.localIgnored)}</span></div>`;
  const protocolArt = `
    <div class="protocol-tabs" role="tablist" aria-label="${esc(t.nav[2])}">${t.protocols.map((p, i) => `<button role="tab" id="protocol-tab-${i}" aria-selected="${i === 0}" tabindex="${i === 0 ? 0 : -1}" aria-controls="protocol-panel-${i}" data-protocol-tab="${i}">${p[0]}</button>`).join("")}</div>
    ${t.protocols
      .map(
        (
          p,
          i,
        ) => `<div class="protocol-panel" role="tabpanel" id="protocol-panel-${i}" aria-labelledby="protocol-tab-${i}" ${i ? "hidden" : ""}>
      <div class="request-heading"><span class="method method-${i}">${p[3]}</span><code>${esc(p[4])}</code></div>
      <div class="packet-route" aria-hidden="true"><span>${esc(t.requestLabel)}</span><div class="packet-line"><i class="packet-out animated"></i><i class="packet-back animated"></i></div><span>API</span></div>
      <div class="response-header"><span>${esc(t.responseLabel)}</span><span class="response-status">${["200 OK", "200 OK", "text/event-stream", "101 Switching Protocols"][i]}</span></div>
      <pre class="response-code"><code>${(i === 1
        ? '{\n  "data": {\n    "viewer": {\n      "id": "1",\n      "name": "Alex"\n    }\n  }\n}'
        : p[5]
      )
        .split("\n")
        .map(
          (line, n) =>
            `<span class="response-line line-${n % 4} animated">${esc(line) || " "}</span>`,
        )
        .join("")}</code></pre>
      <div class="protocol-bottom"><span class="demo-result" role="status" aria-live="polite">${esc(p[1])}</span><button data-send-demo hidden>${esc(t.sendDemo)} <span aria-hidden="true">↻</span></button></div>
      <p class="protocol-description">${esc(p[2])}</p>
    </div>`,
      )
      .join("")}`;
  const agents = [
    ["claude", "Claude Code", "claude.svg"],
    ["codex", "Codex", "openai.svg"],
    ["opencode", "OpenCode", "opencode.svg"],
    ["pi", "Pi", null],
  ];
  const agentArt = `
    <div class="agent-orbit"><svg class="agent-connections" viewBox="0 0 500 250" fill="none" aria-hidden="true"><path d="M85 65C180 65 160 125 250 125M415 65C320 65 340 125 250 125M85 205C180 205 160 125 250 125M415 205C320 205 340 125 250 125"/><path class="agent-signal animated" d="M85 65C180 65 160 125 250 125M415 65C320 65 340 125 250 125M85 205C180 205 160 125 250 125M415 205C320 205 340 125 250 125"/></svg>
      ${agents.map(([id, label, file]) => `<div class="agent-node agent-${id}"><div class="agent-icon">${file ? `<img src="${assetUrl(`agents/${file}`)}" alt="" width="36" height="36" loading="lazy">` : '<span class="pi-symbol" aria-hidden="true">π</span>'}</div><span>${label}</span></div>`).join("")}
      <div class="agent-hub"><div class="hub-ring animated" aria-hidden="true"></div><img src="/apifuck.png" alt="ApiFuck" width="58" height="58"><span>MCP</span></div>
    </div>
    <div class="agent-terminal">${panelHead(t.agentsDemo, "MCP")}<p class="agent-prompt"><span aria-hidden="true">›</span>${esc(t.agentsPrompt)}</p><ol class="agent-calls"><li class="animated"><span>01</span><code>get_workspace</code><b>✓</b></li><li class="animated"><span>02</span><code>send_request</code><b>✓</b></li><li class="animated"><span>03</span><code>run_collection</code><b>✓</b></li></ol><div class="agent-terminal-footer"><i class="status-dot"></i>${esc(t.agentsResult)}</div></div>`;
  return `<div class="story-intro wrap"><span>${esc(t.motionHint)}</span><span class="story-line" aria-hidden="true"></span><button class="motion-toggle" aria-pressed="false" data-pause-label="${esc(t.motionPause)}" data-play-label="${esc(t.motionResume)}" hidden><span aria-hidden="true">Ⅱ</span><span data-motion-label>${esc(t.motionPause)}</span></button></div>
    <div class="feature-band native-band"><section class="feature-section wrap" id="native"><div class="feature-copy">${head(t.nativeEyebrow, t.nativeTitle, t.nativeLead)}${points(t.nativeItems)}<p class="feature-source">${esc(t.nativeCompetitors)}<br>${source("https://github.com/usebruno/bruno/blob/main/packages/bruno-electron/package.json", "Bruno")} · ${source("https://github.com/Kong/insomnia/blob/develop/packages/insomnia/package.json", "Insomnia")} · ${source("https://www.electronjs.org/docs/latest", t.nativeSource)}</p></div>${scene("native-visual", nativeArt, t.nativeFootnote)}</section></div>
    <div class="feature-band local-band"><section class="feature-section feature-reverse wrap" id="local"><div class="feature-copy">${head(t.localEyebrow, t.localTitle, t.localLead)}${points(t.localItems)}</div>${scene("local-visual", localArt, t.fileCaption)}</section></div>
    <div class="feature-band protocol-band"><section class="feature-section wrap" id="workbench"><div class="feature-copy">${head(t.workflowEyebrow, t.workflowTitle, t.workflowLead)}<p class="protocol-hint">${esc(t.protocolHint)} <span aria-hidden="true">→</span></p><div class="body-formats"><span>${esc(t.bodyLabel)}</span><p>${esc(t.bodyFormats)}</p></div><a class="text-link" href="${github}/blob/main/docs/execution.md">${esc(t.docs)} ↗</a></div>${scene("protocol-visual", protocolArt, t.demoLabel)}</section></div>
    <div class="feature-band agent-band"><section class="feature-section feature-reverse wrap" id="agents"><div class="feature-copy">${head(t.agentsEyebrow, t.agentsTitle, t.agentsLead)}${points(t.agentsItems)}<a class="text-link" href="${github}/blob/main/docs/mcp.md">${esc(t.agentsLink)} ↗</a><p class="agent-note">${esc(t.agentsNote)}</p></div>${scene("agents-visual", agentArt, t.agentsConnected)}</section></div>`;
}
