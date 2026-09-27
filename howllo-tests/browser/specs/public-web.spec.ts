import { expect, test } from "@playwright/test";
import { local, missingCapability, requireLocalService, requireValue, workspaceA } from "../lib/environment";

async function boardDirectory(page: import("@playwright/test").Page): Promise<string[]> {
  const workspace = requireValue(workspaceA, "HOWLLO_TEST_WORKSPACE_A");
  await requireLocalService("Howllo Web", local.web);
  const response = await page.goto(`${local.web}/${encodeURIComponent(workspace)}`, { waitUntil: "domcontentloaded" });
  expect(response?.status()).toBe(200);
  await expect(page.getByRole("heading", { name: "Boards", exact: true })).toBeVisible();
  await expect(page.getByText("Could not connect to Howllo", { exact: false })).toHaveCount(0);
  const links = await page.locator('[data-howllo-slot="board.directory"] a[href*="/boards/"]').evaluateAll(
    (anchors) => anchors.map((anchor) => (anchor as HTMLAnchorElement).href),
  );
  if (!links.length) missingCapability(`${workspace} has no published boards to inspect`);
  return links;
}

test("WEB-001 guest can browse every discovered public board", async ({ page }) => {
  const links = await boardDirectory(page);
  for (const url of links) {
    expect(new URL(url).origin).toBe(local.web);
    const response = await page.goto(url, { waitUntil: "domcontentloaded" });
    expect(response?.status()).toBe(200);
    await expect(page.locator(".experience__heading h1")).toBeVisible();
    await expect(page.getByRole("search").getByPlaceholder("Search topics in this board")).toBeVisible();
    await expect(page.getByRole("navigation", { name: "Sort topics" })).toBeVisible();
    await expect(page.locator(".experience__forum-main")).toBeVisible();
  }
});

test("WEB-002 board search shows a no-match result", async ({ page }) => {
  const links = await boardDirectory(page);
  await page.goto(links[0]!, { waitUntil: "domcontentloaded" });
  const marker = `howllo-e2e-${crypto.randomUUID()}`;
  await page.getByRole("search").getByPlaceholder("Search topics in this board").fill(marker);
  await page.getByRole("search").getByRole("button", { name: "Search" }).click();
  await expect(page).toHaveURL((url) => url.searchParams.get("q") === marker);
  await expect(page.getByRole("heading", { name: "No matching topics" })).toBeVisible();
});
