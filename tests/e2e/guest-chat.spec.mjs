import {test,expect} from '@playwright/test';
test('Guest questions and keyboard sending stay inside chat',async({page})=>{
 await page.goto('/');
 const input=page.getByLabel('Message Panda',{exact:true});
 await expect(page.locator('.simulation')).toHaveCount(0);
 await expect(page.getByRole('button',{name:'Send',exact:true})).toBeEnabled();
 await input.fill("what's menu?"); await input.press('Enter');
 await expect(page.locator('.chat-messages .bubble.assistant').last()).toContainText('Hong Kong milk tea');
 await expect(page.locator('.content-panel')).toBeHidden();
 await expect(page.getByRole('button',{name:'Send',exact:true})).toBeEnabled();
 await input.fill('2 milk tea'); await input.press('Shift+Enter');
 await expect(input).toHaveValue('2 milk tea\n');
 await input.press('Enter');
 await expect(page.locator('.basket-line')).toContainText('2 × Hong Kong milk tea');
 await expect(page.getByRole('button',{name:'Send',exact:true})).toBeEnabled();
 await input.fill("what's my order?"); await input.press('Enter');
 await expect(page.locator('.chat-messages .bubble.assistant').last()).toContainText('Basket · not yet ordered');
 await expect(page.getByRole('button',{name:'Send',exact:true})).toBeEnabled();
 await input.fill('explain milk tea'); await input.press('Enter');
 await expect(page.locator('.chat-messages .bubble.assistant').last()).toContainText('Allergens');
 await expect(page.locator('.basket-line')).toHaveCount(1);
 await expect(page.locator('.chat-messages')).not.toContainText('Simulation mode');
});

test('Korean guest sees one Korean reply per message',async({page})=>{
 await page.goto('/');
 await page.getByRole('combobox',{name:'Language',exact:true}).selectOption('ko');
 const input=page.getByLabel('Message Panda',{exact:true});
 await input.fill('현재 메뉴는?');await input.press('Enter');
 const reply=page.locator('.chat-messages .bubble.assistant').last();
 await expect(reply).toContainText('오늘 메뉴예요!');
 await expect(reply).not.toContainText('Hong Kong');
 await expect(page.locator('.chat-messages .notice')).toHaveCount(0);
 await expect(page.getByRole('button',{name:'Send',exact:true})).toBeEnabled();
 await input.fill('직원 불러주세요');await input.press('Enter');
 await expect(reply).toContainText('직원을 불렀어요');
 await expect(reply).not.toContainText('Staff');
 await expect(page.locator('.chat-messages .notice')).toHaveCount(0);
});
