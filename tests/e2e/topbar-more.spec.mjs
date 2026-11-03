import {test,expect} from '@playwright/test';
test.use({browserName:'webkit'});
for(const viewport of [{width:1440,height:900},{width:1024,height:768},{width:768,height:1024},{width:390,height:844},{width:844,height:390}]) {
 test(`More menu opens and navigates at ${viewport.width}x${viewport.height}`,async({page})=>{
  await page.setViewportSize(viewport);await page.goto('/');
  const toggle=page.getByRole('button',{name:'More options',exact:true});
  const menu=page.locator('#topbar-menu');
  await expect(menu).toBeHidden();await toggle.click();await expect(menu).toBeVisible();
  await expect(toggle).toHaveAttribute('aria-expanded','true');
  const box=await menu.boundingBox();expect(box.x).toBeGreaterThanOrEqual(0);expect(box.x+box.width).toBeLessThanOrEqual(viewport.width);
  await menu.getByRole('button',{name:/^⌑ /}).click();
  await expect(menu).toBeHidden();await expect(page.locator('.menu-grid')).toBeVisible();
  await toggle.click();await page.getByTestId('guest-chat-tab').click();
  await expect(menu).toBeHidden();
  await toggle.click();await page.getByRole('button',{name:'Close menu',exact:true}).click({position:{x:2,y:viewport.height-2}});
  await expect(menu).toBeHidden();
  const input=await page.getByLabel('Message Panda',{exact:true}).boundingBox();
  expect(input.y+input.height).toBeLessThanOrEqual(viewport.height);
 });
}
