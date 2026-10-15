import { nativeAssets } from "./releases.mjs";
const root = document.documentElement;
const zh = root.lang === "zh-CN";
const header = document.querySelector(".site-header");
const menu = document.querySelector(".menu-toggle");
const theme = document.querySelector(".theme-toggle");
const dark = matchMedia("(prefers-color-scheme: dark)");
const refreshHeader = () => root.classList.toggle("scrolled", scrollY > 24);
refreshHeader();
addEventListener("scroll", refreshHeader, { passive: true });
function themeState() {
  const isDark = root.dataset.theme
    ? root.dataset.theme === "dark"
    : dark.matches;
  theme.setAttribute("aria-pressed", String(isDark));
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
