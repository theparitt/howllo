import { expect, test } from "@playwright/test";
import { newActorContext, verifyDistinctActorStates } from "../lib/personas";

test("HARNESS-001 independent actors have independent browser storage", async ({ browser }) => {
  const actors = ["A-OWNER", "A-MOD", "U-FEATURE", "U-FOREIGN"];
  const contexts = await Promise.all(actors.map(() => browser.newContext()));
  try {
    const pages = await Promise.all(contexts.map(async (context) => {
      await context.route("http://127.0.0.1:7799/*", (route) => route.fulfill({
        status: 200,
        contentType: "text/html",
        body: "<!doctype html><title>Context isolation fixture</title>",
      }));
      const page = await context.newPage();
      await page.goto("http://127.0.0.1:7799/persona");
      return page;
    }));
    for (const [index, page] of pages.entries()) {
      await page.evaluate((actor) => {
        localStorage.setItem("howllo-test-actor", actor);
        sessionStorage.setItem("howllo-test-actor", actor);
        document.cookie = `howllo_test_actor=${actor}; path=/`;
      }, actors[index]);
    }
    for (const [index, page] of pages.entries()) {
      const visible = await page.evaluate(() => ({
        local: localStorage.getItem("howllo-test-actor"),
        session: sessionStorage.getItem("howllo-test-actor"),
        cookie: document.cookie,
      }));
      expect(visible.local).toBe(actors[index]);
      expect(visible.session).toBe(actors[index]);
      expect(visible.cookie).toContain(`howllo_test_actor=${actors[index]}`);
      for (const other of actors.filter((_, otherIndex) => otherIndex !== index)) {
        expect(visible.cookie).not.toContain(`howllo_test_actor=${other}`);
      }
    }
  } finally {
    await Promise.all(contexts.map((context) => context.close()));
  }
});

test("HARNESS-002 configured actor credentials are distinct", async () => {
  verifyDistinctActorStates(["A-OWNER", "A-MOD", "U-FEATURE", "U-FOREIGN"]);
});

test("HARNESS-003 actor context loads only its own storage state", async ({ browser }) => {
  verifyDistinctActorStates(["A-OWNER", "U-FEATURE"]);
  const owner = await newActorContext(browser, "A-OWNER");
  const member = await newActorContext(browser, "U-FEATURE");
  try {
    const ownerState = await owner.storageState();
    const memberState = await member.storageState();
    expect(ownerState).not.toEqual(memberState);
  } finally {
    await Promise.all([owner.close(), member.close()]);
  }
});
