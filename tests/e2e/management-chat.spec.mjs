import { test, expect } from '@playwright/test';

test('Owner can manage through chat with usage always visible', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Wallet login', exact: true }).click();
  await page.getByLabel('Recovery phrase', { exact: true }).fill('abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about');
  await page.getByRole('button', { name: 'Unlock café', exact: true }).click();
  await expect(page.getByRole('dialog', { name: 'Wallet login' })).toBeHidden();
  await page.getByRole('button',{name:'More options',exact:true}).click();
  await page.getByRole('button', { name: 'Shop management', exact: true }).click();
  await page.getByRole('button', { name: 'Manage through chat', exact: true }).click();
  await expect(page.locator('.content-panel')).toBeHidden();
  await expect(page.getByTestId('token-summary-hour')).toBeVisible();
  await expect(page.getByTestId('token-summary-day')).toBeVisible();
  await page.getByLabel('Message Panda', { exact: true }).fill('menu list');
  await expect(page.getByLabel('Message Panda', { exact: true })).toHaveValue('menu list');
  await expect(page.locator('.chat-panel')).toBeVisible();
});
