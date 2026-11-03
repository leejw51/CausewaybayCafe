import {test,expect} from '@playwright/test';
test.use({browserName:'webkit'});
for(const viewport of [{width:1440,height:900},{width:1024,height:768},{width:768,height:1024},{width:390,height:844},{width:844,height:390}]) for(const layout of ['horizontal','vertical']) {
 test(`Order sheet ${layout} ${viewport.width}x${viewport.height}`,async({page})=>{
  await page.setViewportSize(viewport);
  await page.addInitScript(mode=>localStorage.setItem('cafe-layout',mode),layout);
  await page.goto('/');
  await page.getByRole('button',{name:'Add Hong Kong milk tea',exact:true}).click();
  const sheet=page.getByRole('complementary',{name:'Your order',exact:true});
  await expect(sheet.locator('.basket-line')).toContainText('Hong Kong milk tea');
  const box=await sheet.boundingBox(),chat=await page.locator('.chat-panel').boundingBox();
  if(layout==='horizontal') expect(box.x).toBeGreaterThanOrEqual(chat.x+chat.width-1);
  else expect(box.y).toBeGreaterThanOrEqual(chat.y+chat.height-1);
  expect(box.y+box.height).toBeLessThanOrEqual(viewport.height);
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
  const input=await page.getByLabel('Message Panda',{exact:true}).boundingBox();
  expect(input.y+input.height).toBeLessThanOrEqual(viewport.height);
  await page.screenshot({path:`docs/previews/order-sheet-${viewport.width}x${viewport.height}-${layout}.png`});
  await sheet.getByRole('button',{name:'Remove Hong Kong milk tea',exact:true}).click();
  await expect(sheet.locator('.basket-line')).toHaveCount(0);
 });
}
