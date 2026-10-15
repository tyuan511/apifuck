// Real WebGL checks through headless browser APIs. No screenshots, readPixels,
// desktop automation, or assumptions about a particular shader implementation.
import assert from "node:assert/strict";
import { chromium } from "@playwright/test";

const origin = process.env.WEBSITE_URL || "http://127.0.0.1:9092";
const endpoint = "https://api.github.com/repos/tyuan511/apifuck/releases/latest";
const browser = await chromium.launch({
  headless: true,
  args: ["--use-angle=swiftshader", "--enable-unsafe-swiftshader"],
  ...(process.env.CHROME_PATH ? { executablePath: process.env.CHROME_PATH } : {}),
});
const pageErrors = [];

async function createPage({ reducedMotion = "no-preference", unavailable = false, locale = "zh" } = {}) {
  const context = await browser.newContext({
    reducedMotion,
    viewport: { width: 1440, height: 1000 },
    deviceScaleFactor: 2,
  });
  await context.route(endpoint, (route) =>
    route.fulfill({ json: { tag_name: "v0.0.8", assets: [] } }),
  );
  await context.addInitScript(({ unavailable }) => {
    const probe = {
      contextRequests: 0,
      contexts: [],
      compiles: [],
      links: [],
      draws: [],
      errors: [],
      uniforms: {},
    };
    window.__auroraProbe = probe;
    const originalGetContext = HTMLCanvasElement.prototype.getContext;
    HTMLCanvasElement.prototype.getContext = function (kind, ...args) {
      if (!this.matches(".hero-aurora") || !["webgl", "webgl2", "experimental-webgl"].includes(kind))
        return originalGetContext.call(this, kind, ...args);
      probe.contextRequests += 1;
      if (unavailable) return null;
      const gl = originalGetContext.call(this, kind, ...args);
      if (!gl || probe.contexts.some((entry) => entry.gl === gl)) return gl;
      probe.contexts.push({ gl, version: gl.getParameter(gl.VERSION), kind });
      const compileShader = gl.compileShader.bind(gl);
      gl.compileShader = (shader) => {
        compileShader(shader);
        probe.compiles.push({
          ok: gl.getShaderParameter(shader, gl.COMPILE_STATUS),
          log: gl.getShaderInfoLog(shader),
        });
      };
      const linkProgram = gl.linkProgram.bind(gl);
      gl.linkProgram = (program) => {
        linkProgram(program);
        probe.links.push({
          ok: gl.getProgramParameter(program, gl.LINK_STATUS),
          log: gl.getProgramInfoLog(program),
        });
      };
      const locations = new WeakMap();
      const getUniformLocation = gl.getUniformLocation.bind(gl);
      gl.getUniformLocation = (program, name) => {
        const location = getUniformLocation(program, name);
        if (location) locations.set(location, name);
        return location;
      };
      for (const name of ["uniform1f", "uniform2f", "uniform3f", "uniform4f", "uniform1fv", "uniform2fv", "uniform3fv", "uniform4fv"]) {
        const original = gl[name].bind(gl);
        gl[name] = (location, ...values) => {
          const key = locations.get(location);
          if (key) {
            const uniform = probe.uniforms[key] ||= { calls: 0, values: [] };
            uniform.calls += 1;
            const value = JSON.stringify(values.map((entry) => ArrayBuffer.isView(entry) ? Array.from(entry) : entry));
            if (!uniform.values.includes(value)) uniform.values.push(value);
          }
          return original(location, ...values);
        };
      }
      for (const name of ["drawArrays", "drawElements"]) {
        const draw = gl[name].bind(gl);
        gl[name] = (...values) => {
          draw(...values);
          probe.draws.push({
            at: performance.now(),
            width: gl.drawingBufferWidth,
            height: gl.drawingBufferHeight,
            cssWidth: this.getBoundingClientRect().width,
            cssHeight: this.getBoundingClientRect().height,
          });
          for (let attempt = 0; attempt < 8; attempt++) {
            const error = gl.getError();
            if (error === gl.NO_ERROR) break;
            if (error !== gl.CONTEXT_LOST_WEBGL) probe.errors.push(error);
          }
        };
      }
      return gl;
    };

    // This exercises visibilitychange handling without changing the desktop's
    // foreground app or allowing browser-level freezing to hide a timer bug.
    const hiddenGetter = Object.getOwnPropertyDescriptor(Document.prototype, "hidden").get;
    const visibilityGetter = Object.getOwnPropertyDescriptor(Document.prototype, "visibilityState").get;
    let forcedHidden = null;
    Object.defineProperty(document, "hidden", {
      configurable: true,
      get: () => forcedHidden ?? hiddenGetter.call(document),
    });
    Object.defineProperty(document, "visibilityState", {
      configurable: true,
      get: () => forcedHidden === null ? visibilityGetter.call(document) : forcedHidden ? "hidden" : "visible",
    });
    window.__setAuroraVisibility = (hidden) => {
      forcedHidden = hidden;
      document.dispatchEvent(new Event("visibilitychange"));
    };
  }, { unavailable });
  const page = await context.newPage();
  page.setDefaultTimeout(20000);
  page.on("pageerror", (error) => pageErrors.push(error.message));
  await page.goto(origin + (locale === "en" ? "/en/" : "/"));
  await page.waitForFunction(() => document.querySelector(".hero-aurora")?.dataset.state);
  return { context, page };
}

async function drawCount(page) {
  return page.evaluate(() => window.__auroraProbe.draws.length);
}

async function assertRunning(page) {
  await page.waitForFunction(() => document.querySelector(".hero-aurora").dataset.running === "true");
  const initial = await drawCount(page);
  await page.waitForFunction((initial) => window.__auroraProbe.draws.length >= initial + 3, initial);
}

async function assertNotDrawing(page, label) {
  await page.waitForFunction(() => document.querySelector(".hero-aurora").dataset.running === "false");
  // A state change can request one static redraw; allow it to settle before
  // verifying the animation loop itself has stopped.
  await page.waitForTimeout(180);
  const before = await drawCount(page);
  await page.waitForTimeout(700);
  assert.ok(await drawCount(page) <= before + 1, `${label}: no sustained draw loop`);
}

async function flowStyles(page) {
  return page.locator(".hero-flow .hero-ribbon").evaluateAll((ribbons) =>
    ribbons.map((ribbon) => {
      const style = getComputedStyle(ribbon);
      return { transform: style.transform, animation: style.animationName, state: style.animationPlayState };
    }),
  );
}

async function assertFlowLayout(page) {
  assert.equal(await page.locator(".hero-backdrop .hero-flow").count(), 1);
  assert.ok(await page.locator(".hero-flow .hero-ribbon").count() > 0);
  const layout = await page.locator(".hero-flow").evaluate((flow) => {
    const copy = document.querySelector(".hero-copy").getBoundingClientRect();
    const style = getComputedStyle(flow);
    return {
      decorative: !!flow.closest('[aria-hidden="true"]'),
      pointerEvents: style.pointerEvents,
      opacity: Number(style.opacity),
      visibility: style.visibility,
      inTextRegion: [...flow.querySelectorAll(".hero-ribbon")].some((ribbon) => {
        const rect = ribbon.getBoundingClientRect();
        const paint = getComputedStyle(ribbon);
        return rect.right > Math.max(0, copy.left) && rect.left < Math.min(innerWidth, copy.right) &&
          rect.bottom > Math.max(0, copy.top) && rect.top < Math.min(innerHeight, copy.bottom) &&
          paint.display !== "none" && paint.visibility === "visible" && Number(paint.opacity) > 0;
      }),
      overflow: document.documentElement.scrollWidth > innerWidth,
    };
  });
  assert.equal(layout.decorative, true, "flow remains hidden from assistive technology");
  assert.equal(layout.pointerEvents, "none", "flow cannot intercept controls");
  assert.ok(layout.opacity > 0 && layout.visibility === "visible", "flow is not hidden by its container");
  assert.equal(layout.inTextRegion, true, "a visible flow ribbon intersects the first-screen text region");
  assert.equal(layout.overflow, false, "moving ribbons add no horizontal overflow");
}

async function assertFlowAnimating(page, label) {
  await page.waitForFunction(() => document.querySelector(".hero-flow").dataset.active === "true");
  const initial = await flowStyles(page);
  assert.ok(initial.some(({ animation, state }) => animation !== "none" && state.split(",").some((value) => value.trim() === "running")),
    `${label}: flow has a running CSS animation`);
  await page.waitForFunction(
    (initial) => [...document.querySelectorAll(".hero-flow .hero-ribbon")].some(
      (ribbon, index) => getComputedStyle(ribbon).transform !== initial[index].transform,
    ),
    initial,
    { timeout: 5000 },
  );
}

async function measureFlowMotion(page, label) {
  const sample = () => page.locator(".hero-flow .hero-ribbon").evaluateAll((ribbons) => ({
    at: performance.now(),
    ribbons: ribbons.map((ribbon) => {
      const rect = ribbon.getBoundingClientRect();
      const value = getComputedStyle(ribbon).transform;
      const matrix = value === "none" ? new DOMMatrixReadOnly() : new DOMMatrixReadOnly(value);
      return {
        x: rect.left + rect.width / 2,
        y: rect.top + rect.height / 2,
        width: rect.width,
        height: rect.height,
        tx: matrix.m41,
        ty: matrix.m42,
      };
    }),
  }));
  const before = await sample();
  await page.waitForTimeout(1000);
  const after = await sample();
  const round = (value) => Math.round(value * 100) / 100;
  const movement = after.ribbons.map((ribbon, index) => {
    const initial = before.ribbons[index];
    return {
      ribbon: index + 1,
      centerDx: round(ribbon.x - initial.x),
      centerDy: round(ribbon.y - initial.y),
      centerDistance: round(Math.hypot(ribbon.x - initial.x, ribbon.y - initial.y)),
      matrixDx: round(ribbon.tx - initial.tx),
      matrixDy: round(ribbon.ty - initial.ty),
      widthDelta: round(ribbon.width - initial.width),
      heightDelta: round(ribbon.height - initial.height),
    };
  });
  assert.ok(movement.some(({ centerDistance, widthDelta, heightDelta }) =>
    centerDistance > 0.5 || Math.abs(widthDelta) > 0.5 || Math.abs(heightDelta) > 0.5),
  `${label}: motion exceeds a subpixel-only transform change`);
  console.log(`MOTION ${label}: ${JSON.stringify({ elapsedMs: round(after.at - before.at), movement })}`);
}

async function assertFlowStill(page, label, expectInactiveFlag = true) {
  if (expectInactiveFlag)
    await page.waitForFunction(() => document.querySelector(".hero-flow").dataset.active === "false");
  // Force the pause style to resolve, then allow its in-flight compositor
  // update to finish. This bounded browser-driver wait also works without JS.
  await flowStyles(page);
  await page.waitForTimeout(250);
  const initial = await flowStyles(page);
  assert.ok(initial.length > 0 && initial.every(({ animation, state }) =>
    animation === "none" || state.split(",").every((value) => value.trim() === "paused")),
  `${label}: flow animation is paused or disabled`);
  await page.waitForTimeout(400);
  assert.deepEqual(await flowStyles(page), initial, `${label}: flow transform remains still`);
}

async function assertCanvasLayout(page) {
  assert.equal(await page.locator(".hero-backdrop canvas.hero-aurora").count(), 1);
  const layout = await page.locator(".hero-aurora").evaluate((canvas) => {
    const rect = canvas.getBoundingClientRect();
    const backdrop = canvas.closest(".hero-backdrop").getBoundingClientRect();
    return {
      ariaHidden: canvas.getAttribute("aria-hidden"),
      pointerEvents: getComputedStyle(canvas).pointerEvents,
      width: rect.width,
      height: rect.height,
      parentWidth: backdrop.width,
      parentHeight: backdrop.height,
      bufferWidth: canvas.width,
      bufferHeight: canvas.height,
      pageOverflow: document.documentElement.scrollWidth > innerWidth,
    };
  });
  assert.equal(layout.ariaHidden, "true");
  assert.equal(layout.pointerEvents, "none");
  assert.ok(Math.abs(layout.width - layout.parentWidth) <= 1, "canvas spans the backdrop width");
  assert.ok(layout.height <= Math.min(700, layout.parentHeight) + 1, "canvas height stays bounded around the first-screen text");
  assert.ok(layout.bufferWidth <= 1440, "drawing buffer width stays capped");
  assert.ok(layout.bufferWidth * layout.bufferHeight <= 1_050_000, "drawing buffer stays near one megapixel");
  assert.ok(layout.bufferWidth <= Math.ceil(layout.width) && layout.bufferHeight <= Math.ceil(layout.height), "effective drawing DPR never exceeds one");
  assert.equal(layout.pageOverflow, false, "canvas adds no horizontal overflow");
  assert.equal(
    await page.locator(".hero-button").evaluate((button) => {
      const rect = button.getBoundingClientRect();
      const target = document.elementFromPoint(rect.left + rect.width / 2, rect.top + rect.height / 2);
      return target === button || button.contains(target);
    }),
    true,
    "canvas does not intercept the download button",
  );
}

async function assertGraphicsHealthy(page) {
  const graphics = await page.evaluate(() => ({
    versions: window.__auroraProbe.contexts.map(({ kind, version }) => ({ kind, version })),
    compiles: window.__auroraProbe.compiles,
    links: window.__auroraProbe.links,
    errors: window.__auroraProbe.errors,
    frames: window.__auroraProbe.draws,
  }));
  assert.ok(graphics.versions.some(({ kind, version }) => kind === "webgl2" && version.includes("WebGL 2.0")), "a real WebGL2 context was created");
  assert.ok(graphics.compiles.length >= 2, "actual vertex and fragment shaders compile");
  assert.ok(graphics.compiles.every(({ ok }) => ok), JSON.stringify(graphics.compiles));
  assert.ok(graphics.links.length >= 1 && graphics.links.every(({ ok }) => ok), JSON.stringify(graphics.links));
  assert.deepEqual(graphics.errors, [], "shader draws produce no WebGL errors");
  assert.ok(graphics.frames.length > 0, "the linked shader is actually drawn");
  assert.ok(graphics.frames.every(({ width, height }) => width <= 1440 && width * height <= 1_050_000),
  "all submitted frames respect drawing-buffer limits");
}

try {
  // A targeted fallback/no-JS rerun is useful after changing pause assertions.
  // The normal package command runs every case.
  if (process.env.AURORA_FALLBACK_ONLY !== "1") {
  const { context, page } = await createPage();
  await page.waitForFunction(() => document.querySelector(".hero-aurora").dataset.state === "ready");
  await assertRunning(page);
  await assertCanvasLayout(page);
  await assertFlowLayout(page);
  await assertFlowAnimating(page, "normal hero");
  await measureFlowMotion(page, "WebGL ready");
  const frameWindow = await page.evaluate(() => ({ count: window.__auroraProbe.draws.length, at: performance.now() }));
  await page.waitForTimeout(1400);
  const frameEnd = await page.evaluate(() => ({ count: window.__auroraProbe.draws.length, at: performance.now() }));
  assert.ok(frameEnd.count > frameWindow.count, "normal motion advances frames");
  assert.ok(frameEnd.count - frameWindow.count <= Math.ceil((frameEnd.at - frameWindow.at) * 30 / 1000) + 2, "animation stays at or below 30 frames per second");
  assert.equal(
    await page.evaluate(() => Object.values(window.__auroraProbe.uniforms).some(({ calls, values }) => calls > 1 && values.length > 1)),
    true,
    "changing uniforms animate the actual shader",
  );

  const pause = page.locator(".motion-toggle");
  await pause.evaluate((button) => button.click());
  await assertNotDrawing(page, "global pause");
  await assertFlowStill(page, "global pause");
  const beforeTheme = await drawCount(page);
  await page.locator(".theme-toggle").evaluate((button) => button.click());
  await page.waitForFunction((before) => window.__auroraProbe.draws.length > before, beforeTheme);
  await assertNotDrawing(page, "theme redraw while paused");
  await assertFlowStill(page, "theme redraw while paused");
  await pause.evaluate((button) => button.click());
  await assertRunning(page);
  await assertFlowAnimating(page, "resumed hero");

  await page.locator("#local").evaluate((section) => section.scrollIntoView({ block: "center", behavior: "instant" }));
  await assertNotDrawing(page, "offscreen hero");
  await assertFlowStill(page, "offscreen hero");
  await page.evaluate(() => scrollTo({ top: 0, behavior: "instant" }));
  await assertRunning(page);
  await assertFlowAnimating(page, "hero returning into view");
  await page.evaluate(() => window.__setAuroraVisibility(true));
  await assertNotDrawing(page, "simulated hidden document");
  await assertFlowStill(page, "simulated hidden document");
  await page.evaluate(() => window.__setAuroraVisibility(false));
  await assertRunning(page);
  await assertFlowAnimating(page, "visible document");
  await page.emulateMedia({ reducedMotion: "reduce" });
  await assertNotDrawing(page, "reduced motion");
  await assertFlowStill(page, "reduced motion");
  await page.emulateMedia({ reducedMotion: "no-preference" });
  await assertRunning(page);
  await assertFlowAnimating(page, "motion preference restored");

  for (const width of [320, 375, 768, 2200]) {
    await page.setViewportSize({ width, height: 1000 });
    await page.evaluate(() => scrollTo({ top: 0, behavior: "instant" }));
    await assertRunning(page);
    await assertCanvasLayout(page);
    await assertFlowLayout(page);
    await assertFlowAnimating(page, `${width}px hero`);
  }
  await page.evaluate(() => {
    const extension = window.__auroraProbe.contexts[0].gl.getExtension("WEBGL_lose_context");
    if (!extension) throw new Error("Headless WebGL lacks the context-loss testing extension");
    window.__auroraContextLoss = extension;
    extension.loseContext();
  });
  await page.waitForFunction(() => document.querySelector(".hero-aurora").dataset.state === "lost");
  await assertNotDrawing(page, "lost graphics context");
  await assertFlowAnimating(page, "CSS fallback during context loss");
  await page.evaluate(() => window.__auroraContextLoss.restoreContext());
  await page.waitForFunction(() => document.querySelector(".hero-aurora").dataset.state === "ready");
  await assertRunning(page);
  await assertGraphicsHealthy(page);
  assert.ok(await page.evaluate(() => window.__auroraProbe.links.length >= 2), "restored contexts recreate and link GPU resources");
  await context.close();
  console.log("PASS Aurora WebGL2 and visible CSS flow: animation, budgets, pause, viewport, visibility, theme, resize and context restoration");

  const reduced = await createPage({ reducedMotion: "reduce" });
  await reduced.page.waitForFunction(() => document.querySelector(".hero-aurora").dataset.state === "ready");
  await assertNotDrawing(reduced.page, "initial reduced motion");
  await assertFlowStill(reduced.page, "initial reduced motion");
  assert.ok(await drawCount(reduced.page) >= 1 && await drawCount(reduced.page) <= 4,
    "initial reduced motion only renders bounded setup/theme/resize frames");
  await assertGraphicsHealthy(reduced.page);
  await assertCanvasLayout(reduced.page);
  await assertFlowLayout(reduced.page);
  await reduced.context.close();
  }

  for (const locale of ["zh", "en"]) {
    const fallback = await createPage({ unavailable: true, locale });
    await fallback.page.waitForFunction(() => document.querySelector(".hero-aurora").dataset.state === "fallback");
    assert.equal(await fallback.page.locator("h1").isVisible(), true);
    assert.equal(await fallback.page.locator(".hero-button").isVisible(), true);
    assert.equal(await fallback.page.locator("[data-platform]").count(), 3);
    assert.equal(await drawCount(fallback.page), 0);
    assert.ok(await fallback.page.evaluate(() => window.__auroraProbe.contextRequests > 0));
    assert.equal(await fallback.page.locator(".hero-aurora").evaluate((canvas) => getComputedStyle(canvas).pointerEvents), "none");
    await assertFlowLayout(fallback.page);
    await assertFlowAnimating(fallback.page, `${locale}: no-WebGL fallback`);
    if (locale === "zh") await measureFlowMotion(fallback.page, "WebGL unavailable");
    const fallbackPause = fallback.page.locator(".motion-toggle");
    await fallbackPause.evaluate((button) => button.click());
    await assertFlowStill(fallback.page, `${locale}: paused no-WebGL fallback`);
    await fallbackPause.evaluate((button) => button.click());
    await assertFlowAnimating(fallback.page, `${locale}: resumed no-WebGL fallback`);
    await fallback.page.emulateMedia({ reducedMotion: "reduce" });
    await assertFlowStill(fallback.page, `${locale}: reduced-motion no-WebGL fallback`);
    await fallback.page.emulateMedia({ reducedMotion: "no-preference" });
    await assertFlowAnimating(fallback.page, `${locale}: normal-motion no-WebGL fallback`);
    await fallback.page.evaluate(() => window.__setAuroraVisibility(true));
    await assertFlowStill(fallback.page, `${locale}: hidden no-WebGL fallback`);
    await fallback.page.evaluate(() => window.__setAuroraVisibility(false));
    await assertFlowAnimating(fallback.page, `${locale}: visible no-WebGL fallback`);
    await fallback.page.locator(".hero-button").click();
    assert.equal(new URL(fallback.page.url()).hash, "#download");
    await assertFlowStill(fallback.page, `${locale}: offscreen no-WebGL fallback`);
    await fallback.context.close();
  }
  for (const locale of ["zh", "en"]) {
    const context = await browser.newContext({ javaScriptEnabled: false, viewport: { width: 375, height: 1000 } });
    const page = await context.newPage();
    await page.goto(origin + (locale === "en" ? "/en/" : "/"));
    assert.equal(await page.locator("h1").isVisible(), true);
    await assertFlowStill(page, `${locale}: no JavaScript`, false);
    await page.locator(".hero-button").click();
    assert.equal(new URL(page.url()).hash, "#download");
    await context.close();
  }
  assert.deepEqual(pageErrors, []);
  console.log("PASS reduced-motion static frame, animated no-WebGL fallback, motion controls and bilingual no-JS content/downloads");
} finally {
  await browser.close();
}
