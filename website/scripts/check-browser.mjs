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
      const page = await context.newPage();
    page.setDefaultTimeout(15000);
      page.on("pageerror", (error) => errors.push(error.message));
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
          ".url-row",
          ".protocol-tabs",
          ".download-card",
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
        for (const id of ["workbench", "local", "download"])
          assert.equal(await page.locator(`#${id}`).count(), 1);
        assert.equal(await page.locator("h1").count(), 1);
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
      await page.keyboard.press("End");
      assert.equal(await page.locator("#protocol-panel-3").isVisible(), true);
      await page.locator('[data-request-tab="2"]').click();
      assert.equal(await page.locator("#request-panel-2").isVisible(), true);
      await page.locator(".send-button").click();
      assert.match(
        await page.locator("#demo-status").innerText(),
        locale === "zh" ? /示例完成/ : /Example complete/,
      );
      await page.locator("summary").first().click();
      assert.equal(
        await page.locator("details").first().getAttribute("open"),
        "",
      );
      await context.close();
    }
  const context = await browser.newContext();
  const page = await context.newPage();
    page.setDefaultTimeout(15000);
  await context.route(endpoint, (route) =>
    route.fulfill({
      json: {
        tag_name: "v0.1.0",
        assets: Object.values(assetNames).map((name) => ({
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
  for (const [platform, name] of Object.entries(assetNames))
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
  const noJS = await browser.newContext({
    javaScriptEnabled: false,
    viewport: { width: 375, height: 900 },
  });
  const staticPage = await noJS.newPage();
  await staticPage.goto(origin);
  assert.equal(await staticPage.locator("h1").isVisible(), true);
  assert.equal(await staticPage.locator("#protocol-panel-3").isVisible(), true);
  assert.equal(
    await staticPage.evaluate(
      () => document.documentElement.scrollWidth <= innerWidth,
    ),
    true,
    "no-JS overflow",
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
  assert.deepEqual(errors, []);
  console.log(
    "PASS keyboard tabs, demo, FAQ, theme persistence, release success/fallback, no-JS and HTTP checks",
  );
} finally {
  await browser.close();
}
