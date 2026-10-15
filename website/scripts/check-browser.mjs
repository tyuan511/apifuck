// Browser DOM checks only: no screenshots or desktop-level Computer Use.
import assert from "node:assert/strict";
import { chromium } from "@playwright/test";
import { assetNames } from "../src/releases.mjs";
const origin = process.env.WEBSITE_URL || "http://127.0.0.1:9092";
const browser = await chromium.launch({
  headless: true,
  ...(process.env.CHROME_PATH
    ? { executablePath: process.env.CHROME_PATH }
    : {}),
});
const errors = [];
const endpoint =
  "https://api.github.com/repos/tyuan511/apifuck/releases/latest";
const featureIds = ["native", "local", "workbench", "agents"];
const featureBackgrounds = new Map();

function recordErrors(page) {
  page.setDefaultTimeout(15000);
  page.on("pageerror", (error) => errors.push(error.message));
}

async function assertTypewriterMarkup(page, locale, staticText = true) {
  const typewriter = page.locator("[data-typewriter]");
  assert.equal(await typewriter.count(), 1);
  const phrases = JSON.parse(await typewriter.getAttribute("data-phrases"));
  assert.ok(Array.isArray(phrases) && phrases.length >= 4);
  assert.equal(new Set(phrases).size, phrases.length);
  assert.ok(phrases.every((phrase) => typeof phrase === "string" && phrase.trim()));
  assert.equal(await typewriter.getAttribute("aria-hidden"), "true");
  const readerCopy = page.locator("h1 .sr-only");
  assert.equal(await readerCopy.count(), 1);
  assert.ok((await readerCopy.textContent()).trim().length >= phrases[0].length);
  assert.notEqual(await readerCopy.getAttribute("aria-hidden"), "true");
  assert.deepEqual(
    await typewriter.locator(".typewriter-reserve").allTextContents(),
    phrases,
    `${locale}: every phrase reserves its complete layout space`,
  );
  assert.equal(
    await typewriter.locator(".typewriter-reserve").evaluateAll((elements) =>
      elements.every((element) => {
        const style = getComputedStyle(element);
        return style.display !== "none" &&
          (style.visibility === "hidden" || Number(style.opacity) === 0) &&
          element.clientHeight > 0 && element.scrollWidth <= element.clientWidth + 1;
      }),
    ),
    true,
    `${locale}: invisible phrase reserves participate in layout without overflow`,
  );
  if (staticText)
    assert.equal(await typewriter.locator("[data-typewriter-text]").textContent(), phrases[0]);
  return phrases;
}

async function assertFeatureBackgrounds(page, locale, colorScheme, width) {
  assert.equal(await page.locator(".feature-band").count(), 4);
  const backgrounds = await page.locator(".feature-band").evaluateAll((bands) =>
    bands.map((band) => ({
      id: band.querySelector(".feature-section")?.id || band.id,
      paint: [null, "::before", "::after"].map((pseudo) => {
        const style = getComputedStyle(band, pseudo);
        return `${style.backgroundColor} ${style.backgroundImage}`;
      }).join(" | "),
    })),
  );
  assert.deepEqual(backgrounds.map(({ id }) => id), featureIds);
  for (const { id, paint } of backgrounds)
    assert.match(paint, /gradient\(/, `${colorScheme}: ${id} has a gradient background`);
  assert.equal(
    new Set(backgrounds.map(({ paint }) => paint)).size,
    4,
    `${colorScheme}: the four feature backgrounds have distinct colors`,
  );
  if (locale === "zh" && width === 1440)
    featureBackgrounds.set(colorScheme, backgrounds);
}

async function startTypewriterSampling(page) {
  await page.evaluate(() => {
    const text = document.querySelector("[data-typewriter-text]");
    const heading = document.querySelector("h1");
    const actions = document.querySelector(".hero-actions");
    const state = { samples: [] };
    const sample = () => {
      const rect = heading.getBoundingClientRect();
      const writer = text.closest("[data-typewriter]");
      state.samples.push({
        text: text.textContent,
        width: rect.width,
        height: rect.height,
        actionsY: actions.getBoundingClientRect().top + scrollY,
        overflow: document.documentElement.scrollWidth > innerWidth ||
          writer.scrollWidth > writer.clientWidth + 1 ||
          text.scrollWidth > text.clientWidth + 1,
      });
    };
    sample();
    state.observer = new MutationObserver(sample);
    state.observer.observe(text, { childList: true, characterData: true, subtree: true });
    window.__apifuckTypewriterCheck = state;
  });
}

async function assertTypewriterSamples(page, phrases, label) {
  const samples = await page.evaluate(() => window.__apifuckTypewriterCheck.samples);
  for (const phrase of phrases)
    assert.ok(samples.some(({ text }) => text === phrase), `${label}: every complete phrase appears`);
  assert.ok(
    samples.some(({ text }, index) => index > 0 && text.length > 0 &&
      !phrases.includes(text) && phrases.some((phrase) => phrase.startsWith(text)) &&
      Array.from(text).length === Array.from(samples[index - 1].text).length + 1),
    `${label}: phrases appear character by character`,
  );
  for (const sample of samples) {
    assert.ok(Math.abs(sample.width - samples[0].width) < 1, `${label}: heading width is stable`);
    assert.ok(Math.abs(sample.height - samples[0].height) < 1, `${label}: heading height is stable`);
    assert.ok(Math.abs(sample.actionsY - samples[0].actionsY) < 1, `${label}: action buttons do not shift`);
    assert.equal(sample.overflow, false, `${label}: typing does not overflow`);
  }
}

async function assertFeatureContent(page, locale) {
  assert.equal(await page.locator(".feature-section").count(), 4);
  assert.equal(await page.locator(".feature-section.feature-reverse").count(), 2);
  for (const id of featureIds) {
    const section = page.locator(`#${id}.feature-section`);
    assert.equal(await section.count(), 1, `${locale}: ${id} feature exists`);
    assert.equal(await section.locator(".feature-copy").count(), 1);
    assert.equal(await section.locator(".feature-visual").count(), 1);
    assert.equal(await section.locator("h2").count(), 1);
    assert.equal(
      await section.locator(".feature-copy").evaluate((element) => {
        const style = getComputedStyle(element);
        return (
          element.textContent.trim().length > 50 &&
          style.visibility === "visible" &&
          Number(style.opacity) > 0
        );
      }),
      true,
      `${locale}: ${id} copy remains readable before scrolling`,
    );
    assert.equal(await section.locator("[data-motion-scene]").count(), 1);
  }
  const native = await page.locator("#native").innerText();
  assert.match(native, /Rust/);
  assert.match(native, /GPUI/);
  assert.match(native, /WebView/i);
  const local = await page.locator("#local").innerText();
  assert.match(local, /Git/);
  assert.match(local, /YAML/);
  const protocols = await page.locator("#workbench").innerText();
  for (const name of ["HTTP", "GraphQL", "SSE", "WebSocket"])
    assert.ok(protocols.includes(name), `${locale}: ${name} is described`);
  const agents = await page.locator("#agents").innerText();
  for (const name of ["MCP", "Claude Code", "Codex", "OpenCode", "Pi"])
    assert.ok(agents.includes(name), `${locale}: ${name} integration is described`);
  assert.match(agents, locale === "zh" ? /设置/ : /settings/i);
  assert.doesNotMatch(agents, /MCP.{0,30}(coming soon|planned|即将上线|规划中)/i);
  assert.equal(
    await page.locator('#agents a[href$="/docs/mcp.md"]').count(),
    1,
    `${locale}: available MCP integration links to its setup guide`,
  );
  // Agent marks are lazy-loaded: bring them into view before checking the
  // loaded result, with a bounded wait rather than an unbounded decode().
  await page.locator("#agents .feature-visual").scrollIntoViewIfNeeded();
  await page.waitForFunction(
    () => [...document.querySelectorAll("#agents img")].every(
      (image) => image.complete && image.naturalWidth > 0 && image.naturalHeight > 0,
    ),
  );
  await page.evaluate(() => scrollTo({ top: 0, behavior: "instant" }));
}

async function assertFeatureLayout(page, width) {
  const rows = await page.locator(".feature-section").evaluateAll((sections) =>
    sections.map((section) => {
      const copy = section.querySelector(".feature-copy").getBoundingClientRect();
      const visual = section.querySelector(".feature-visual").getBoundingClientRect();
      return {
        id: section.id,
        reversed: section.classList.contains("feature-reverse"),
        copy: { left: copy.left, right: copy.right, bottom: copy.bottom },
        visual: { left: visual.left, right: visual.right, top: visual.top },
      };
    }),
  );
  for (const row of rows) {
    if (width >= 1024)
      assert.ok(
        row.reversed
          ? row.visual.right <= row.copy.left + 1
          : row.copy.right <= row.visual.left + 1,
        `${width}: ${row.id} uses the expected alternating columns`,
      );
    if (width <= 375)
      assert.ok(
        row.copy.bottom <= row.visual.top + 1,
        `${width}: ${row.id} puts readable copy above its illustration`,
      );
  }
}

async function animationsAreStopped(page, selector = ".animated") {
  return page.locator(selector).evaluateAll((elements) =>
    elements.every((element) => {
      const style = getComputedStyle(element);
      return (
        style.animationName === "none" ||
        style.animationDuration.split(",").every((value) => parseFloat(value) === 0) ||
        style.animationPlayState.split(",").every((value) => value.trim() === "paused")
      );
    }),
  );
}
try {
  for (const locale of ["zh", "en"])
    for (const colorScheme of ["light", "dark"]) {
      const context = await browser.newContext({
        colorScheme,
        reducedMotion: "reduce",
      });
      await context.route(endpoint, (route) =>
        route.fulfill({ json: { tag_name: "v0.0.8", assets: [] } }),
      );
      // An old unversioned asset must not be reused after the page updates.
      await context.route(
        (url) => url.pathname === "/styles.css" && !url.search,
        (route) =>
          route.fulfill({
            contentType: "text/css",
            body: ".workbench-screenshot { width: 2560px !important; }",
          }),
      );
      const page = await context.newPage();
      recordErrors(page);
      const path = locale === "en" ? "/en/" : "/";
      for (const width of [320, 375, 768, 1024, 1440]) {
        await page.setViewportSize({ width, height: 900 });
        assert.equal((await page.goto(origin + path)).status(), 200);
        await page.waitForFunction(
          () => document.querySelector(".theme-toggle").hidden === false,
        );
        assert.equal(
          await page.locator("html").getAttribute("lang"),
          locale === "zh" ? "zh-CN" : "en",
        );
        assert.equal(
          await page.evaluate(
            () => getComputedStyle(document.documentElement).colorScheme,
          ),
          colorScheme,
        );
        assert.equal(
          await page.evaluate(
            () => document.documentElement.scrollWidth <= innerWidth,
          ),
          true,
          `${locale}/${colorScheme}/${width}: page overflow`,
        );
        // Critical UI must fit its own box, not merely avoid document overflow.
        for (const selector of [
          ".header-inner",
          ".hero-copy",
          "[data-typewriter]",
          ".typewriter-reserve",
          ".preview-wrap",
          ".protocol-tabs",
          ".download-card",
          ".feature-section",
          ".feature-copy",
          ".feature-visual",
          "[data-motion-scene]",
          ".protocol-panel",
          ".demo-result",
          "[data-send-demo]",
          ".motion-toggle",
        ]) {
          assert.equal(
            await page
              .locator(selector)
              .evaluateAll((els) =>
                els.every((el) => el.scrollWidth <= el.clientWidth + 1),
              ),
            true,
            `${width}: overflow in ${selector}`,
          );
        }
        for (const id of [...featureIds, "download"])
          assert.equal(await page.locator(`#${id}`).count(), 1);
        await assertFeatureContent(page, locale);
        await assertFeatureLayout(page, width);
        await assertTypewriterMarkup(page, locale);
        await assertFeatureBackgrounds(page, locale, colorScheme, width);
        if (locale === "zh")
          assert.deepEqual(
            await page.locator("#navigation a").allTextContents(),
            ["原生体验", "Git 友好", "多种协议", "MCP", "下载"],
            "Chinese navigation names match the feature sections",
          );
        assert.equal(
          await page.locator("[data-motion-scene].is-active").count(),
          0,
          "reduced motion never starts a decorative scene",
        );
        assert.equal(await animationsAreStopped(page), true);
        assert.equal(await page.locator(".motion-toggle").isVisible(), true);
        assert.equal(await page.locator("h1").count(), 1);
        const screenshot = page.locator(".workbench-screenshot");
        await screenshot.evaluate((image) => image.decode());
        assert.equal(
          await screenshot.evaluate((image) => image.naturalWidth),
          2560,
        );
        assert.equal(
          await screenshot.evaluate((image) => image.naturalHeight),
          1680,
        );
        assert.equal(
          await screenshot.evaluate(
            (image) =>
              Math.abs(image.width / image.height - 2560 / 1680) < 0.01,
          ),
          true,
          "screenshot retains its full aspect ratio",
        );
        const imageUrl = await screenshot.getAttribute("src");
        assert.equal(
          await page.locator(".screenshot-link").getAttribute("href"),
          imageUrl,
        );
        assert.equal(
          new URL(imageUrl, origin).pathname,
          "/workbench-screenshot.png",
        );
        assert.equal(
          await screenshot.evaluate((image) => {
            const imageRect = image.getBoundingClientRect();
            const containerRect = image
              .closest("figure")
              .getBoundingClientRect();
            return (
              imageRect.left >= containerRect.left &&
              imageRect.right <= containerRect.right + 1 &&
              imageRect.right <= innerWidth
            );
          }),
          true,
          "entire screenshot fits inside its container and viewport",
        );
        assert.equal(await page.locator("[data-platform]").count(), 3);
        for (const link of await page.locator("[data-platform]").all())
          assert.equal(
            await link.getAttribute("href"),
            "https://github.com/tyuan511/apifuck/releases",
          );
        assert.equal(
          await page
            .locator('a[href^="#"]')
            .evaluateAll((els) =>
              els.every(
                (el) =>
                  document.querySelector(el.getAttribute("href")) !== null,
              ),
            ),
          true,
        );
        if (width < 681) {
          await page.locator(".menu-toggle").click();
          assert.equal(
            await page.locator(".menu-toggle").getAttribute("aria-expanded"),
            "true",
          );
          await page.keyboard.press("Escape");
          assert.equal(
            await page.locator(".menu-toggle").getAttribute("aria-expanded"),
            "false",
          );
        }
        console.log(
          `PASS ${locale}/${colorScheme}/${width}: content, layout, navigation, downloads`,
        );
      }
      await page.locator(".theme-toggle").click();
      const opposite = colorScheme === "light" ? "dark" : "light";
      assert.equal(
        await page.evaluate(
          () => getComputedStyle(document.documentElement).colorScheme,
        ),
        opposite,
      );
      await page.reload();
      assert.equal(
        await page.evaluate(
          () => getComputedStyle(document.documentElement).colorScheme,
        ),
        opposite,
      );
      await page.locator('[data-protocol-tab="0"]').focus();
      await page.keyboard.press("ArrowRight");
      assert.equal(await page.locator("#protocol-panel-1").isVisible(), true);
      assert.equal(
        await page.locator('[data-protocol-tab="1"]').getAttribute("aria-selected"),
        "true",
      );
      await page.keyboard.press("End");
      assert.equal(await page.locator("#protocol-panel-3").isVisible(), true);
      await page.keyboard.press("Home");
      assert.equal(await page.locator("#protocol-panel-0").isVisible(), true);
      const demoRequests = [];
      const trackDemoRequest = (request) => {
        if (["fetch", "xhr"].includes(request.resourceType()))
          demoRequests.push(request.url());
      };
      const trackDemoSocket = (socket) => demoRequests.push(socket.url());
      page.on("request", trackDemoRequest);
      page.on("websocket", trackDemoSocket);
      for (let index = 0; index < 4; index++) {
        const tab = page.locator(`[data-protocol-tab="${index}"]`);
        const panel = page.locator(`#protocol-panel-${index}`);
        await tab.click();
        assert.equal(await panel.isVisible(), true);
        assert.equal(await tab.getAttribute("aria-selected"), "true");
        assert.equal(await page.locator('.protocol-panel:visible').count(), 1);
        const result = panel.locator(".demo-result");
        assert.equal(await result.getAttribute("aria-live"), "polite");
        assert.ok((await result.innerText()).trim().length > 0);
        for (let replay = 0; replay < 2; replay++) {
          const previousResult = await result.innerText();
          await panel.locator("[data-send-demo]").click();
          assert.equal(
            await panel.evaluate((element) => element.classList.contains("is-playing")),
            false,
            "reduced-motion demo completes immediately",
          );
          assert.ok((await result.innerText()).trim().length > 0);
          if (replay === 0)
            assert.notEqual(await result.innerText(), previousResult, "demo announces completion");
        }
      }
      page.off("request", trackDemoRequest);
      page.off("websocket", trackDemoSocket);
      assert.deepEqual(demoRequests, [], "protocol demonstrations do not send requests");
      await page.locator("summary").first().click();
      assert.equal(
        await page.locator("details").first().getAttribute("open"),
        "",
      );
      await context.close();
    }
  for (let index = 0; index < featureIds.length; index++)
    assert.notEqual(
      featureBackgrounds.get("light")[index].paint,
      featureBackgrounds.get("dark")[index].paint,
      `${featureIds[index]} background adapts to the color theme`,
    );

  // Observe real text mutations and layout at every typing step. These pages
  // use normal motion separately from the reduced-motion responsive matrix.
  for (const locale of ["zh", "en"]) {
    for (const width of [320, 375, 1440]) {
      const typingContext = await browser.newContext({
        reducedMotion: "no-preference",
        viewport: { width, height: 900 },
      });
      await typingContext.route(endpoint, (route) =>
        route.fulfill({ json: { tag_name: "v0.0.8", assets: [] } }),
      );
      const typingPage = await typingContext.newPage();
      recordErrors(typingPage);
      await typingPage.goto(origin + (locale === "en" ? "/en/" : "/"));
      const phrases = await assertTypewriterMarkup(typingPage, locale);
      const typewriterText = typingPage.locator("[data-typewriter-text]");
      await startTypewriterSampling(typingPage);
      await typingPage.waitForFunction(
        (phrase) => window.__apifuckTypewriterCheck.samples.some(({ text }) => text === phrase),
        phrases.at(-1),
        { timeout: 45000 },
      );
      if (width === 1440) {
        // Pause in the middle of a change, so completing the active phrase is
        // observable instead of accidentally passing during its normal hold.
        await typingPage.waitForFunction(
          (phrases) => !phrases.includes(document.querySelector("[data-typewriter-text]").textContent),
          phrases,
        );
        const toggle = typingPage.locator(".motion-toggle");
        await toggle.evaluate((element) => element.click());
        assert.equal(await toggle.getAttribute("aria-pressed"), "true");
        const pausedText = await typewriterText.textContent();
        assert.ok(phrases.includes(pausedText), `${locale}: pausing completes the active phrase`);
        await typingPage.waitForTimeout(2600);
        assert.equal(await typewriterText.textContent(), pausedText, "paused typing remains still");
        await toggle.evaluate((element) => element.click());
        await typingPage.waitForFunction(
          (previous) => document.querySelector("[data-typewriter-text]").textContent !== previous,
          pausedText,
        );
        await typingPage.locator("#local").evaluate((element) =>
          element.scrollIntoView({ block: "center", behavior: "instant" }),
        );
        await typingPage.waitForFunction(
          () => document.querySelector("[data-typewriter]").dataset.typingActive === "false",
        );
        const offscreenText = await typewriterText.textContent();
        await typingPage.waitForTimeout(2600);
        assert.equal(await typewriterText.textContent(), offscreenText, "offscreen typing is paused");
        await typingPage.evaluate(() => scrollTo({ top: 0, behavior: "instant" }));
        await typingPage.waitForFunction(
          (previous) => document.querySelector("[data-typewriter-text]").textContent !== previous,
          offscreenText,
        );
        await typingPage.emulateMedia({ reducedMotion: "reduce" });
        await typingPage.waitForFunction(
          (first) => document.querySelector("[data-typewriter-text]").textContent === first,
          phrases[0],
        );
        await typingPage.waitForTimeout(2600);
        assert.equal(await typewriterText.textContent(), phrases[0], "reduced motion keeps the first phrase still");
      }
      await assertTypewriterSamples(typingPage, phrases, `${locale}/${width}`);
      await typingContext.close();
      console.log(`PASS ${locale}/${width}: character-by-character hero, reserved layout and motion controls`);
    }
  }

  // Motion assertions have their own context so animation cannot obscure the
  // responsive geometry checks above, which run with reduced motion enabled.
  const motionContext = await browser.newContext({
    reducedMotion: "no-preference",
    viewport: { width: 1440, height: 900 },
  });
  await motionContext.route(endpoint, (route) =>
    route.fulfill({ json: { tag_name: "v0.0.8", assets: [] } }),
  );
  const motionPage = await motionContext.newPage();
  recordErrors(motionPage);
  await motionPage.goto(origin);
  await motionPage.waitForFunction(
    () => document.querySelector(".motion-toggle").hidden === false,
  );
  for (const id of featureIds) {
    const selector = `#${id} [data-motion-scene]`;
    const scene = motionPage.locator(selector);
    await scene.evaluate((element) =>
      element.scrollIntoView({ block: "center", behavior: "instant" }),
    );
    await motionPage.waitForFunction(
      (selector) => document.querySelector(selector).classList.contains("is-active"),
      selector,
    );
    assert.equal(
      await scene.evaluate((element) => element.classList.contains("is-visible")),
      true,
      `${id}: scrolling reveals the scene`,
    );
    assert.ok(await scene.locator(".animated").count(), `${id}: scene contains animation`);
    assert.equal(await animationsAreStopped(motionPage, `${selector} .animated`), false);
  }
  const motionToggle = motionPage.locator(".motion-toggle");
  // Trigger the control without moving the current scene out of the viewport.
  await motionToggle.evaluate((element) => element.click());
  assert.equal(await motionToggle.getAttribute("aria-pressed"), "true");
  assert.equal(await motionPage.locator("[data-motion-scene].is-active").count(), 0);
  assert.equal(await animationsAreStopped(motionPage), true, "global pause stops motion");
  assert.equal(
    await motionPage.locator(".feature-copy, .feature-visual").evaluateAll((elements) =>
      elements.every((element) => {
        const style = getComputedStyle(element);
        return style.transform === "none" &&
          style.transitionDuration.split(",").every((value) => parseFloat(value) === 0);
      }),
    ),
    true,
    "global pause also stops scroll-reveal transitions",
  );
  await motionToggle.evaluate((element) => element.click());
  assert.equal(await motionToggle.getAttribute("aria-pressed"), "false");
  await motionPage.waitForFunction(
    () => document.querySelector("#agents [data-motion-scene]").classList.contains("is-active"),
  );
  await motionPage.evaluate(() => scrollTo({ top: 0, behavior: "instant" }));
  await motionPage.waitForFunction(
    () => !document.querySelector("#agents [data-motion-scene]").classList.contains("is-active"),
  );
  assert.equal(
    await animationsAreStopped(motionPage, "#agents .animated"),
    true,
    "offscreen scenes stop their animations",
  );
  for (let index = 0; index < 4; index++) {
    await motionPage.locator(`[data-protocol-tab="${index}"]`).click();
    const panel = motionPage.locator(`#protocol-panel-${index}`);
    for (let replay = 0; replay < 2; replay++) {
      const button = panel.locator("[data-send-demo]");
      const result = panel.locator(".demo-result");
      const previousResult = await result.innerText();
      await button.click();
      assert.equal(
        await panel.evaluate((element) => element.classList.contains("is-playing")),
        true,
        `${index}: request demo starts and can be replayed`,
      );
      assert.equal(await button.isDisabled(), true);
      const playingResult = await result.innerText();
      assert.notEqual(playingResult, previousResult, "demo announces playback");
      await motionPage.waitForFunction(
        (index) => !document.getElementById(`protocol-panel-${index}`).classList.contains("is-playing"),
        index,
      );
      assert.equal(await button.isEnabled(), true);
      assert.ok((await result.innerText()).trim().length > 0);
      assert.notEqual(await result.innerText(), playingResult, "demo announces completion");
    }
  }
  await motionPage.locator("#protocol-panel-3 [data-send-demo]").click();
  await motionPage.emulateMedia({ reducedMotion: "reduce" });
  await motionPage.waitForFunction(
    () => document.querySelectorAll("[data-motion-scene].is-active").length === 0,
  );
  assert.equal(await motionPage.locator(".protocol-panel.is-playing").count(), 0);
  assert.equal(await motionPage.locator("#protocol-panel-3 [data-send-demo]").isEnabled(), true);
  assert.equal(await animationsAreStopped(motionPage), true, "preference changes stop motion");
  await motionContext.close();
  console.log("PASS scroll reveal, viewport motion, pause/resume, demo replay and reduced-motion changes");

  const context = await browser.newContext();
  const page = await context.newPage();
  recordErrors(page);
  await context.route(endpoint, (route) =>
    route.fulfill({
      json: {
        tag_name: "v0.1.0",
        assets: Object.values(assetNames("0.1.0")).map((name) => ({
          name,
          size: 1024,
          browser_download_url: `https://github.com/tyuan511/apifuck/releases/download/v0.1.0/${name}`,
        })),
      },
    }),
  );
  await page.goto(origin);
  await page.waitForFunction(() =>
    document
      .querySelector('[data-platform="mac"]')
      .href.includes("/download/v0.1.0/"),
  );
  for (const [platform, name] of Object.entries(assetNames("0.1.0")))
    assert.equal(
      await page.locator(`[data-platform="${platform}"]`).getAttribute("href"),
      `https://github.com/tyuan511/apifuck/releases/download/v0.1.0/${name}`,
    );
  await context.unroute(endpoint);
  await context.route(endpoint, (route) =>
    route.fulfill({
      status: 403,
      json: { message: "API rate limit exceeded" },
    }),
  );
  await page.reload();
  await page.waitForFunction(() =>
    document
      .querySelector(".release-status")
      .textContent.includes("暂时无法检查"),
  );
  assert.equal(
    await page.locator('[data-platform="mac"]').getAttribute("href"),
    "https://github.com/tyuan511/apifuck/releases",
  );
  await context.close();
  for (const locale of ["zh", "en"]) {
    const noJS = await browser.newContext({
      javaScriptEnabled: false,
      viewport: { width: 375, height: 900 },
    });
    const staticPage = await noJS.newPage();
    recordErrors(staticPage);
    await staticPage.goto(origin + (locale === "en" ? "/en/" : "/"));
    assert.equal(await staticPage.locator("h1").isVisible(), true);
    await assertTypewriterMarkup(staticPage, locale);
    await assertFeatureContent(staticPage, locale);
    await assertFeatureLayout(staticPage, 375);
    for (const id of featureIds)
      assert.equal(await staticPage.locator(`#${id} .feature-copy`).isVisible(), true);
    for (let index = 0; index < 4; index++) {
      const panel = staticPage.locator(`#protocol-panel-${index}`);
      assert.equal(await panel.isVisible(), true);
      assert.ok((await panel.locator(".demo-result").innerText()).trim().length > 0);
      assert.equal(await panel.locator("[data-send-demo]").isVisible(), false);
    }
    assert.equal(await staticPage.locator(".motion-toggle").isVisible(), false);
    assert.equal(await animationsAreStopped(staticPage), true);
    assert.equal(
      await staticPage.evaluate(
        () => document.documentElement.scrollWidth <= innerWidth,
      ),
      true,
      `${locale}: no-JS overflow`,
    );
    assert.equal(
      (await staticPage.request.get(origin + "/missing-page")).status(),
      404,
    );
    assert.equal(
      (await staticPage.request.get(origin + "/health.json")).status(),
      200,
    );
    await noJS.close();
  }
  assert.deepEqual(errors, []);
  console.log(
    "PASS real screenshot, keyboard tabs, FAQ, theme persistence, release success/fallback, no-JS and HTTP checks",
  );
} finally {
  await browser.close();
}
