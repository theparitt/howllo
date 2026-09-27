import { expect, test } from "@playwright/test";
import { local, requireLocalService } from "../lib/environment";

test("ADMIN-001 guest reaches the admin sign-in gate", async ({ page }) => {
  await requireLocalService("Howllo Admin", local.admin);
  const response = await page.goto(`${local.admin}/admin`, { waitUntil: "domcontentloaded" });
  expect(response?.status()).toBe(200);
  await expect(page.getByRole("heading", { name: /Admin sign in|Set up admin|Server unreachable/ })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Server unreachable" })).toHaveCount(0);
  await expect(page.getByLabel("Password", { exact: true }).or(page.getByLabel("New password", { exact: true }))).toBeVisible();
});
