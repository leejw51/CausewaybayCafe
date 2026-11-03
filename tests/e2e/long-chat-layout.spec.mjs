import {test,expect} from '@playwright/test';
test.use({browserName:'webkit'});
for(const viewport of [{width:1920,height:930},{width:1024,height:768},{width:390,height:844},{width:844,height:390}]) for(const layout of ['horizontal','vertical']) {
 test(`Long chat keeps composer visible ${layout} ${viewport.width}`,async({page})=>{
  await page.setViewportSize(viewport);
  await page.addInitScript(mode=>localStorage.setItem('cafe-layout',mode),layout);
  await page.goto('/');await expect(page.getByLabel('Message Panda',{exact:true})).toBeVisible();
  await page.locator('.chat-messages').evaluate(el=>{for(let i=0;i<80;i++){const bubble=document.createElement('div');bubble.className='bubble assistant';bubble.textContent=`주문 내역 ${i}: 밀크티 세 잔을 추가했습니다. 메뉴와 주문 내용을 확인해 주세요.`;el.appendChild(bubble);}});
  const input=page.getByLabel('Message Panda',{exact:true});const box=await input.boundingBox();
  expect(box.y+box.height).toBeLessThanOrEqual(viewport.height);
  expect(await page.evaluate(()=>document.documentElement.scrollHeight<=innerHeight+2)).toBe(true);
  await input.fill('현재 주문은?');await expect(input).toHaveValue('현재 주문은?');
  const transcript=page.locator('.chat-messages');
  expect(await transcript.evaluate(el=>el.scrollHeight>el.clientHeight)).toBe(true);
  await transcript.evaluate(el=>el.scrollTop=el.scrollHeight);
  await expect.poll(()=>transcript.evaluate(el=>el.scrollTop)).toBeGreaterThan(0);
  expect(await page.evaluate(()=>scrollY)).toBe(0);
  const after=await input.boundingBox();expect(after.y).toBe(box.y);
  await page.screenshot({path:`docs/previews/long-chat-${viewport.width}-${layout}.png`});
 });
}

test('Switching layout and theme keeps all guest sizing classes and composer',async({page})=>{
 await page.goto('/');
 for(let i=0;i<4;i++){
  await page.locator('.layout-toggle').click();
  await page.getByRole('button',{name:'Toggle theme',exact:true}).click();
  await expect(page.locator('.workspace')).toHaveClass(/guest-workspace/);
  await expect(page.locator('.workspace')).toHaveClass(/chat-home/);
  await expect(page.locator('.app')).toHaveClass(/guest-chat-app/);
  const box=await page.getByLabel('Message Panda',{exact:true}).boundingBox();
  expect(box.y+box.height).toBeLessThanOrEqual(1000);
  await expect(page.locator('.voice-options')).toBeHidden();
 }
});
