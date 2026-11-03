import {test,expect} from '@playwright/test';
test.use({browserName:'webkit'});
for (const viewport of [{width:1024,height:768},{width:768,height:1024},{width:1180,height:820},{width:390,height:844},{width:844,height:390},{width:1440,height:900},{width:900,height:1440}]) {
 for(const layout of ['horizontal','vertical']) test(`Guest ${layout} navigation and composer fit ${viewport.width}×${viewport.height}`,async({page})=>{
  await page.setViewportSize(viewport);
  await page.addInitScript(value=>localStorage.setItem('cafe-layout',value),layout);
  await page.goto('/');
  if(viewport.height>600) await expect(page.locator('.panda-stage')).toBeVisible();
  await expect(page.getByRole('button',{name:'More options',exact:true})).toBeVisible();
  await expect(page.locator('.topbar .cafe-options')).toBeHidden();
  const input=await page.getByLabel('Message Panda',{exact:true}).boundingBox();
  const voice=await page.getByRole('button',{name:/Speak to order/}).boundingBox();
  expect(input.y+input.height).toBeLessThanOrEqual(viewport.height);
  expect(voice.y+voice.height).toBeLessThanOrEqual(viewport.height);
  expect(await page.evaluate(()=>document.documentElement.scrollHeight<=innerHeight+2)).toBe(true);
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
  const transcript=page.locator('.chat-messages');
  await transcript.evaluate(el=>el.scrollTop=el.scrollHeight);
  expect(await page.evaluate(()=>scrollY)).toBe(0);
  await page.screenshot({path:`docs/previews/guest-${viewport.width}x${viewport.height}-${layout}.png`});
 });
}
