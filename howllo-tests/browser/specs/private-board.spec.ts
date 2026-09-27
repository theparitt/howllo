import { expect, test } from "@playwright/test";
import { local, privateBoardA, requireLocalService, requireValue, workspaceA } from "../lib/environment";
import { newActorContext, verifyDistinctActorStates } from "../lib/personas";

test("ISOLATION-001 owner sees private board while foreign actor and guest cannot", async ({ browser }) => {
  const slug = requireValue(workspaceA, "HOWLLO_TEST_WORKSPACE_A");
  const board = requireValue(privateBoardA, "HOWLLO_TEST_PRIVATE_BOARD_A");
  await requireLocalService("Howllo Web", local.web);
  verifyDistinctActorStates(["A-OWNER", "U-FOREIGN"]);
  const url = `${local.web}/${encodeURIComponent(slug)}/boards/${encodeURIComponent(board)}`;
  const owner = await newActorContext(browser, "A-OWNER");
  const foreign = await newActorContext(browser, "U-FOREIGN");
  const guest = await browser.newContext();
  try {
    const ownerPage = await owner.newPage();
    const ownerResponse = await ownerPage.goto(url, { waitUntil: "domcontentloaded" });
    expect(ownerResponse?.status()).toBe(200);
    await expect(ownerPage.locator(".experience__heading h1")).toBeVisible();

    const foreignPage = await foreign.newPage();
    const foreignResponse = await foreignPage.goto(url, { waitUntil: "domcontentloaded" });
    expect(foreignResponse?.status()).toBe(404);
    await expect(foreignPage.locator(".experience__heading h1")).toHaveCount(0);

    const guestPage = await guest.newPage();
    const guestResponse = await guestPage.goto(url, { waitUntil: "domcontentloaded" });
    expect(guestResponse?.status()).toBe(404);
    await expect(guestPage.locator(".experience__heading h1")).toHaveCount(0);
  } finally {
    await Promise.all([owner.close(), foreign.close(), guest.close()]);
  }
});
