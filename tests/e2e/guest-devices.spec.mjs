import {test,expect,devices} from '@playwright/test';
test.use({browserName:'webkit'});
for (const [name,options] of [
 ['desktop Safari',{browserName:'webkit',viewport:{width:1440,height:1000}}],
 ['iPhone Safari',{...devices['iPhone 13'],browserName:'webkit'}],
 ['iPad Safari',{...devices['iPad (gen 7)'],browserName:'webkit'}]
]) {
 test.describe(name,()=>{
  const {browserName,defaultBrowserType,...contextOptions}=options;
  test.use(contextOptions);
  test('QR guest can type, order and receive speech feedback in the transcript',async({page},info)=>{
   await page.goto('/?table=2');
   await expect(page.locator('.chat-panel')).toHaveCSS('display','flex');
   await expect(page.locator('.table-tag')).toContainText('Table 2');
   await expect(page.locator('.chat-menu')).toContainText('Hong Kong milk tea');
   const input=page.getByRole('textbox',{name:'Message Panda',exact:true});
   await input.click();await input.fill('밀크티 2개');
   await expect(input).toHaveValue('밀크티 2개');
   await page.getByRole('button',{name:'Send',exact:true}).click();
   await expect(page.locator('.chat-messages')).toContainText('밀크티 2개');
   await expect(page.locator('.basket-line')).toContainText('2 × Hong Kong milk tea');
   expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
   // Verify recognition result → same chat path. Hardware permission/audio is not mocked as a real recording.
   await page.evaluate(()=>{
    window.SpeechRecognition=class {
     start(){setTimeout(()=>{this.onresult({results:[[{transcript:'에그타르트 1개'}]]});this.onend();},20);}
     abort(){this.onend?.();}
    };
   });
   await page.getByRole('button',{name:/Speak to order/}).click();
   await expect(page.locator('.chat-messages')).toContainText('에그타르트 1개');
   await expect(page.locator('.basket-line')).toHaveCount(2);
   await page.getByRole('button',{name:'Remove Egg tart',exact:true}).click();
   await expect(page.locator('.chat-messages .notice')).toContainText('Removed');
   await expect(page.locator('.basket-line')).toHaveCount(1);
   await page.screenshot({path:info.outputPath('guest-device.png'),fullPage:true});
  });
 });
}
