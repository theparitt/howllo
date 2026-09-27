import { expect, test } from "@playwright/test";
import { local, requireLocalService, requireValue, workspaceA } from "../lib/environment";
import { newActorContext } from "../lib/personas";

test("APP-001 signed-out visitor sees the staff sign-in surface", async ({ page }) => {
  await requireLocalService("Howllo App", local.app);
  const response = await page.goto(local.app, { waitUntil: "domcontentloaded" });
  expect(response?.status()).toBe(200);
  await expect(page.getByRole("link", { name: "Howllo Staff App" })).toBeVisible();
  await expect(page.getByRole("heading", { name: /Workspace staff sign in|Sign in to get started/ })).toBeVisible();
});

test("APP-002 owner can open their workspace management UI", async ({ browser }) => {
  const slug = requireValue(workspaceA, "HOWLLO_TEST_WORKSPACE_A");
  await requireLocalService("Howllo App", local.app);
  const context = await newActorContext(browser, "A-OWNER");
  try {
    const page = await context.newPage();
    const response = await page.goto(`${local.app}/app/${encodeURIComponent(slug)}`, { waitUntil: "domcontentloaded" });
    expect(response?.status()).toBe(200);
    await expect(page.getByRole("complementary", { name: "Workspace navigation" })).toBeVisible();
    await expect(page.getByRole("button", { name: "Staff", exact: true })).toBeVisible();
  } finally {
    await context.close();
  }
});
