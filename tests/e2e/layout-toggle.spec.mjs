import {test,expect} from '@playwright/test';
test('Topbar layout switches and survives reload without opening settings',async({page})=>{
 await page.goto('/');
 const toggle=page.locator('.quick-preferences .layout-toggle');
 await expect(toggle).toBeVisible();
 await expect(page.locator('.topbar .cafe-options')).toBeHidden();
 await toggle.click();
 await expect(page.locator('.workspace')).toHaveClass(/vertical/);
 await expect(toggle).toHaveAttribute('aria-pressed','true');
 await page.reload();
 await expect(page.locator('.workspace')).toHaveClass(/vertical/);
 await toggle.click();
 await expect(page.locator('.workspace')).not.toHaveClass(/vertical/);
 await page.setViewportSize({width:390,height:844});
 await expect(toggle).toBeVisible();
 expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
});
