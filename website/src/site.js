import { nativeAssets } from "./releases.mjs";
import { mountHeroAurora } from "./hero-aurora.js";
const root = document.documentElement;
const zh = root.lang === "zh-CN";
const header = document.querySelector(".site-header");
const menu = document.querySelector(".menu-toggle");
const theme = document.querySelector(".theme-toggle");
const dark = matchMedia("(prefers-color-scheme: dark)");
let heroAurora = null;
const refreshHeader = () => root.classList.toggle("scrolled", scrollY > 24);
refreshHeader();
addEventListener("scroll", refreshHeader, { passive: true });
function themeState() {
  const isDark = root.dataset.theme
    ? root.dataset.theme === "dark"
    : dark.matches;
  theme.setAttribute("aria-pressed", String(isDark));
  heroAurora?.setTheme(isDark ? "dark" : "light");
  document.querySelector('meta[name="theme-color"]').content = isDark
    ? "#0d1621"
    : "#3278a7";
}
theme.hidden = false;
themeState();
dark.addEventListener("change", themeState);
theme.addEventListener("click", () => {
  const next = theme.getAttribute("aria-pressed") === "true" ? "light" : "dark";
  root.dataset.theme = next;
  try {
    localStorage.setItem("apifuck-theme", next);
  } catch {}
  themeState();
});
menu.hidden = false;
function closeMenu() {
  header.removeAttribute("data-open");
  menu.setAttribute("aria-expanded", "false");
}
menu.addEventListener("click", () => {
  const open = menu.getAttribute("aria-expanded") !== "true";
  header.toggleAttribute("data-open", open);
  menu.setAttribute("aria-expanded", String(open));
});
header
  .querySelectorAll("nav a, .brand")
  .forEach((link) => link.addEventListener("click", closeMenu));
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && header.hasAttribute("data-open")) {
    closeMenu();
    menu.focus();
  }
});
document.addEventListener("click", (event) => {
  if (!header.contains(event.target)) closeMenu();
});
matchMedia("(min-width:681px)").addEventListener("change", (event) => {
  if (event.matches) closeMenu();
});
function setupTabs(selector) {
  const tabs = [...document.querySelectorAll(selector)];
  function select(tab, focus = false) {
    for (const item of tabs) {
      const selected = item === tab;
      item.setAttribute("aria-selected", String(selected));
      item.tabIndex = selected ? 0 : -1;
      document.getElementById(item.getAttribute("aria-controls")).hidden =
        !selected;
    }
    if (focus) tab.focus();
  }
  tabs.forEach((tab, i) => {
    tab.addEventListener("click", () => select(tab));
    tab.addEventListener("keydown", (event) => {
      let next;
      if (event.key === "ArrowRight") next = (i + 1) % tabs.length;
      if (event.key === "ArrowLeft") next = (i + tabs.length - 1) % tabs.length;
      if (event.key === "Home") next = 0;
      if (event.key === "End") next = tabs.length - 1;
      if (next !== undefined) {
        event.preventDefault();
        select(tabs[next], true);
      }
    });
  });
}
setupTabs("[data-protocol-tab]");

// No scroll-frame rendering: scenes animate only while visible and enabled.
const reducedMotion = matchMedia("(prefers-reduced-motion: reduce)");
heroAurora = mountHeroAurora(document.querySelector(".hero-aurora"));
themeState();
const motionButton = document.querySelector(".motion-toggle");
const scenes = [...document.querySelectorAll("[data-motion-scene]")];
const visibleScenes = new Set();
const heroFlow = document.querySelector(".hero-flow");
let heroVisible = false;
let motionPaused = false;
const refreshTypewriters = setupTypewriters();

function setupTypewriters() {
  const controllers = [];
  for (const element of document.querySelectorAll("[data-typewriter]")) {
    const text = element.querySelector("[data-typewriter-text]");
    let phrases;
    try {
      const values = JSON.parse(element.dataset.phrases);
      if (!Array.isArray(values)) continue;
      phrases = values
        .filter((value) => typeof value === "string" && value.length > 0)
        .map((value) => Array.from(value));
    } catch {
      continue;
    }
    if (!text || phrases.length < 2) continue;

    let phraseIndex = 0;
    let length = phrases[0].length;
    let phase = "hold";
    let remaining = 2000;
    let timer = null;
    let deadline = 0;
    let visible = false;
    let running = false;

    const render = () => {
      text.textContent = phrases[phraseIndex].slice(0, length).join("");
    };
    function stop() {
      if (timer !== null) {
        clearTimeout(timer);
        timer = null;
        remaining = Math.max(0, deadline - performance.now());
      }
      running = false;
      element.dataset.typingActive = "false";
    }
    function schedule() {
      deadline = performance.now() + remaining;
      timer = setTimeout(tick, remaining);
    }
    function tick() {
      timer = null;
      if (!running) return;
      if (phase === "hold") phase = "erase";
      if (phase === "erase") {
        length -= 1;
        remaining = 40;
        if (length === 0) {
          phraseIndex = (phraseIndex + 1) % phrases.length;
          phase = "type";
          remaining = 75;
        }
      } else {
        length += 1;
        remaining = 75;
        if (length === phrases[phraseIndex].length) {
          phase = "hold";
          remaining = 2000;
        }
      }
      render();
      schedule();
    }
    function refresh() {
      if (reducedMotion.matches || motionPaused) {
        stop();
        if (reducedMotion.matches) phraseIndex = 0;
        length = phrases[phraseIndex].length;
        phase = "hold";
        remaining = 2000;
        render();
        return;
      }
      if (!visible || document.hidden) {
        stop();
        return;
      }
      if (!running) {
        running = true;
        element.dataset.typingActive = "true";
        schedule();
      }
    }
    controllers.push(refresh);
    render();
    element.dataset.typingActive = "false";
    if ("IntersectionObserver" in window) {
      const observer = new IntersectionObserver(
        ([entry]) => {
          visible = entry.isIntersecting;
          refresh();
        },
        { threshold: 0 },
      );
      observer.observe(element.closest(".hero") || element);
    }
    // Without visibility observation, keep the complete static heading.
  }
  return () => controllers.forEach((refresh) => refresh());
}

function refreshMotion() {
  root.classList.toggle(
    "motion-ready",
    !reducedMotion.matches && !motionPaused,
  );
  root.classList.toggle("motion-paused", motionPaused);
  for (const scene of scenes) {
    scene.classList.toggle(
      "is-active",
      visibleScenes.has(scene) &&
        !motionPaused &&
        !reducedMotion.matches &&
        !document.hidden,
    );
  }
  motionButton.setAttribute("aria-pressed", String(motionPaused));
  motionButton.querySelector("[data-motion-label]").textContent = motionPaused
    ? motionButton.dataset.playLabel
    : motionButton.dataset.pauseLabel;
  refreshTypewriters();
  heroAurora?.setPaused(motionPaused || reducedMotion.matches);
  if (heroFlow) {
    heroFlow.dataset.active = String(
      heroVisible &&
        !motionPaused &&
        !reducedMotion.matches &&
        !document.hidden,
    );
  }
}
motionButton.hidden = false;
motionButton.addEventListener("click", () => {
  motionPaused = !motionPaused;
  refreshMotion();
});
if ("IntersectionObserver" in window) {
  const observer = new IntersectionObserver(
    (entries) => {
      for (const entry of entries) {
        if (entry.isIntersecting) {
          visibleScenes.add(entry.target);
          entry.target.classList.add("is-visible");
        } else visibleScenes.delete(entry.target);
      }
      refreshMotion();
    },
    { threshold: 0.08 },
  );
  scenes.forEach((scene) => observer.observe(scene));
  const heroObserver = new IntersectionObserver(([entry]) => {
    heroVisible = entry.isIntersecting;
    refreshMotion();
  });
  heroObserver.observe(document.querySelector(".hero"));
} else {
  // Keep static content and replay controls available in older browsers.
  scenes.forEach((scene) => scene.classList.add("is-visible"));
}
document.addEventListener("visibilitychange", refreshMotion);
refreshMotion();

const demos = new Map();
function finishDemo(panel) {
  clearTimeout(demos.get(panel));
  demos.delete(panel);
  panel.classList.remove("is-playing");
  panel.querySelector("[data-send-demo]").disabled = false;
  panel.querySelector(".demo-result").textContent = zh
    ? "演示完成 · 可再次重放"
    : "Demo complete · Replay anytime";
}
document.querySelectorAll("[data-send-demo]").forEach((button) => {
  button.hidden = false;
  button.addEventListener("click", () => {
    const panel = button.closest(".protocol-panel");
    clearTimeout(demos.get(panel));
    if (reducedMotion.matches || motionPaused) {
      finishDemo(panel);
      return;
    }
    panel.classList.add("is-playing");
    button.disabled = true;
    panel.querySelector(".demo-result").textContent = zh
      ? "请求演示中…"
      : "Playing request…";
    demos.set(
      panel,
      setTimeout(() => finishDemo(panel), 2100),
    );
  });
});
reducedMotion.addEventListener("change", () => {
  if (reducedMotion.matches)
    for (const panel of demos.keys()) finishDemo(panel);
  refreshMotion();
});

async function loadDownloads() {
  const statuses = document.querySelectorAll(".release-status");
  statuses.forEach(
    (el) =>
      (el.textContent = zh ? "正在检查原生版本…" : "Checking native releases…"),
  );
  try {
    const response = await fetch(
      "https://api.github.com/repos/tyuan511/apifuck/releases/latest",
      {
        headers: { Accept: "application/vnd.github+json" },
        signal: AbortSignal.timeout(6000),
      },
    );
    if (!response.ok) throw new Error(`GitHub ${response.status}`);
    const assets = nativeAssets(await response.json());
    document.querySelectorAll("[data-platform]").forEach((link) => {
      const asset = assets[link.dataset.platform];
      const label = link.nextElementSibling;
      if (asset) {
        link.href = asset.url;
        link.textContent = `${zh ? "下载" : "Download"} ↓`;
        label.textContent = asset.version;
      } else
        label.textContent = zh ? "原生版本待发布" : "Native build coming soon";
    });
  } catch {
    statuses.forEach(
      (el) =>
        (el.textContent = zh
          ? "暂时无法检查，请查看发布页"
          : "Unable to check. Browse releases."),
    );
  }
}
loadDownloads();
