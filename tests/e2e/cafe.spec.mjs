import { test, expect } from '@playwright/test';
const OWNER = 'abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about';
const STAFF = 'test test test test test test test test test test test junk';
const STAFF_WALLET = '0x70997970c51812dc3a010c7d01b50e0d17dc79c8';
async function login(page, words, index='0') {
  await page.getByRole('button', {name:'Wallet login', exact:true}).click();
  await page.getByLabel('Recovery phrase', {exact:true}).fill(words);
  await page.getByRole('spinbutton', {name:'Wallet index', exact:true}).fill(index);
  await page.getByRole('button', {name:'Unlock café', exact:true}).click();
  await expect(page.getByRole('dialog', {name:'Wallet login'})).toBeHidden();
}
async function confirm(page) {
  const card = page.locator('.confirmation');
  await expect(card).toBeVisible();
  await card.getByRole('button', {name:'Confirm', exact:true}).click();
  await expect(card).toBeHidden();
}
async function orders(page) {
  await page.getByRole('button',{name:'More options',exact:true}).click();
  await page.getByRole('button', {name:/^▤ Orders/}).click();
  await page.getByRole('button', {name:'↻',exact:true}).click();
}

test('Owner management → Guest order → Staff service/payment → revocation', async ({browser}, info) => {
  const ownerContext = await browser.newContext(); const owner = await ownerContext.newPage();
  const staffContext = await browser.newContext(); const staff = await staffContext.newPage();
  const guestContext = await browser.newContext(); const guest = await guestContext.newPage();
  const errors=[];
  for(const page of [owner, staff, guest]) page.on('pageerror', e=>errors.push(e.message));
  try {
    await owner.goto('/'); await login(owner, OWNER);
    await expect(owner.locator('.table-tag')).toContainText('Owner');
    await expect(owner.getByRole('button',{name:'▶ Run',exact:true})).toBeVisible();
    await owner.getByRole('button',{name:'More options',exact:true}).click();
    await owner.getByRole('button',{name:'Menu photo input',exact:true}).click();
    await expect(owner.getByLabel('Menu photo',{exact:true})).toBeDisabled();
    await owner.getByRole('button',{name:'▶ Run',exact:true}).click();
    await expect(owner.getByLabel('Menu photo',{exact:true})).toBeEnabled();
    await owner.getByRole('button',{name:'Stop',exact:true}).click();
    await owner.getByRole('button',{name:'More options',exact:true}).click();
    await owner.getByRole('button',{name:'Menu templates',exact:true}).click();
    await owner.getByRole('combobox',{name:'Template',exact:true}).selectOption({label:'Breakfast'});
    await expect(owner.getByLabel('Edit the prompt',{exact:true})).toHaveValue(/Breakfast/);
    await owner.getByRole('button',{name:'More options',exact:true}).click();
    await owner.getByRole('button',{name:'Shop management',exact:true}).click();
    const team = owner.locator('form').filter({hasText:'Appoint staff'});
    await team.getByLabel('Name',{exact:true}).fill('Service teammate');
    await team.getByLabel('Wallet address',{exact:true}).fill(STAFF_WALLET);
    await team.getByRole('button',{name:'Review / 확인'}).click(); await confirm(owner);
    await expect(owner.getByText('Service teammate',{exact:true})).toBeVisible();
    await owner.getByRole('button',{name:'tables',exact:true}).click();
    const tables = owner.locator('form').filter({hasText:'Create tables'});
    await tables.getByLabel('ID prefix',{exact:true}).fill('P');
    await tables.getByLabel('Number of tables',{exact:true}).fill('2');
    await tables.getByRole('button',{name:'Review / 확인'}).click(); await confirm(owner);
    await owner.getByRole('button',{name:'Print table QR codes / QR 인쇄',exact:true}).click();
    await expect(owner.locator('.qr-grid article')).toHaveCount(10);
    await expect(owner.locator('.qr-grid img').first()).toBeVisible();
    await owner.getByRole('button',{name:'More options',exact:true}).click();
    await owner.getByRole('button',{name:'Shop management',exact:true}).click();
    await owner.getByRole('button',{name:'menu',exact:true}).click();
    const menu = owner.locator('form').filter({hasText:'Add menu item'});
    for(const [label,value] of [['Unique ID','e2e-special'],['Name','E2E special'],['Price HKD','35.50'],['Description','Test recipe']]) await menu.getByLabel(label,{exact:true}).fill(value);
    await menu.getByRole('button',{name:'Review / 확인'}).click(); await confirm(owner);
    await expect(owner.getByText('E2E special · HK$35.50',{exact:true})).toBeVisible();
    await owner.getByRole('button',{name:'More options',exact:true}).click();
    await owner.getByRole('button',{name:'◫ Dashboard',exact:true}).click();
    await expect(owner.getByRole('heading',{name:'AI token usage',exact:true})).toBeVisible();
    for (const period of ['hour','day']) { const card=owner.getByTestId(`token-stats-${period}`); await expect(card).toContainText('Total tokens / limit'); await expect(card).toContainText('Input 0 · Output 0 · Reserved 0'); }
    await expect(owner.getByLabel('Daily tokens',{exact:true})).toBeVisible();
    await owner.screenshot({path:info.outputPath('owner-dashboard.png'),fullPage:true});

    await guest.goto('/?table=P1');
    await expect(guest.locator('.table-tag')).toContainText('Guest · Table P1');
    await expect(guest.getByRole('button',{name:'Shop management',exact:true})).toHaveCount(0);
    await guest.getByRole('button',{name:'More options',exact:true}).click();
    await guest.getByRole('button',{name:/^⌑ /}).click();
    await guest.getByRole('button',{name:'Add E2E special',exact:true}).click();
    await expect(guest.locator('.basket-line')).toHaveCount(1);
    await expect(guest.getByRole('button',{name:'Send',exact:true})).toBeEnabled();
    await guest.getByRole('button',{name:'Confirm order →',exact:true}).click();
    await expect(guest.locator('.basket-line')).toHaveCount(0);
    await orders(guest); await expect(guest.locator('.order-card')).toHaveCount(1);
    await expect(guest.locator('.order-card')).toContainText('HK$35.50');
    await guest.reload(); await orders(guest); await expect(guest.locator('.order-card')).toHaveCount(1);
    await guest.screenshot({path:info.outputPath('guest-order.png'),fullPage:true});

    await staff.goto('/'); await login(staff, STAFF, '1');
    await expect(staff.locator('.table-tag')).toContainText('Staff');
    await expect(staff.getByRole('button',{name:'Shop management',exact:true})).toHaveCount(0);
    await orders(staff);
    let card = staff.locator('.order-card').filter({hasText:'E2E special'});
    await card.getByRole('button',{name:'Edit / 수정',exact:true}).click();
    await staff.getByRole('dialog',{name:'Edit order'}).getByRole('spinbutton').fill('2');
    await staff.getByRole('button',{name:'Review changes',exact:true}).click(); await confirm(staff);
    await expect(card).toContainText('HK$71');
    for(const stage of ['Preparing','Ready','Served']) {
      await card.getByRole('button',{name:`→ ${stage}`,exact:true}).click(); await confirm(staff);
      await expect(card.locator('.status')).toHaveText(stage);
    }
    await card.getByRole('button',{name:'Received · cash',exact:true}).click(); await confirm(staff);
    await staff.getByRole('button',{name:'completed',exact:true}).click();
    await expect(card.locator('.status')).toHaveText('Completed');
    await expect(card.getByRole('button',{name:'Edit / 수정',exact:true})).toHaveCount(0);
    await staff.getByRole('button',{name:'More options',exact:true}).click();
    await staff.getByRole('button',{name:'◫ Dashboard',exact:true}).click();
    await expect(staff.locator('.stats')).toContainText('HK$71');
    await expect(staff.getByRole('heading',{name:'Last 1 hour / 최근 1시간',exact:true})).toBeVisible();
    await expect(staff.getByRole('heading',{name:'Today (KST) / 오늘',exact:true})).toBeVisible();
    await expect(staff.getByLabel('Daily tokens',{exact:true})).toHaveCount(0);
    for (const period of ['hour','day']) { await expect(staff.getByTestId(`token-stats-${period}`)).toContainText('Input 0 · Output 0 · Reserved 0'); }
    await staff.screenshot({path:info.outputPath('staff-dashboard.png'),fullPage:true});
    // AI is still stopped; server events deliver Staff changes without polling.
    await expect(guest.locator('.order-card')).toHaveCount(0, {timeout:3000});
    await guest.getByRole('button',{name:'completed',exact:true}).click();
    await expect(guest.locator('.order-card .status')).toHaveText('Completed');

    await owner.getByRole('button',{name:'More options',exact:true}).click();
    await owner.getByRole('button',{name:'Shop management',exact:true}).click();
    await owner.getByRole('button',{name:'team',exact:true}).click();
    await owner.locator('.manage-card').filter({hasText:STAFF_WALLET}).getByRole('button',{name:'Remove',exact:true}).click(); await confirm(owner);
    await staff.reload(); await expect(staff.locator('.table-tag')).toContainText('Guest');
    expect(errors).toEqual([]);
  } finally { await Promise.allSettled([ownerContext.close(),staffContext.close(),guestContext.close()]); }
});

test('8 concurrent guests: isolated carts, authoritative totals and persistent orders', async ({browser})=>{
  await Promise.all(Array.from({length:8},async(_,i)=>{
    const context=await browser.newContext();const page=await context.newPage();
    try {
      await page.goto(`/?table=${i+1}`);
      await page.getByRole('button',{name:'More options',exact:true}).click();
      await page.getByRole('button',{name:/^⌑ /}).click();
      await page.getByRole('button',{name:'Add Egg tart',exact:true}).click();
      await expect(page.locator('.basket-line')).toHaveCount(1);
      await expect(page.getByRole('button',{name:'Send',exact:true})).toBeEnabled();
      await page.getByRole('button',{name:'Confirm order →',exact:true}).click();
      await expect(page.locator('.basket-line')).toHaveCount(0);
      await orders(page);
      await expect(page.locator('.order-card')).toHaveCount(1);
      await expect(page.locator('.order-card')).toContainText('HK$18');
      await page.reload();await orders(page);
      await expect(page.locator('.order-card')).toHaveCount(1);
    } finally {await context.close();}
  }));
});

test('mobile preferences, language, layout and no horizontal overflow',async({page},info)=>{
  await page.setViewportSize({width:390,height:844});await page.goto('/');
  await page.getByRole('combobox',{name:'Language',exact:true}).selectOption('ko');
  await expect(page.getByRole('button',{name:'지갑 로그인',exact:true})).toBeVisible();
  await page.getByRole('button',{name:'Toggle theme',exact:true}).click();
  await expect(page.locator('.app')).toHaveClass(/dark/);
  await page.getByRole('button',{name:'Increase font',exact:true}).click();
  await expect(page.getByLabel('Current font size',{exact:true})).toHaveText('17');
  await page.getByRole('button',{name:'Switch to vertical layout',exact:true}).click();
  await expect(page.locator('.workspace')).toHaveClass(/vertical/);
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
  await page.screenshot({path:info.outputPath('mobile-guest.png'),fullPage:true});
});

test('guest typing activates chat with Run off and never starts background AI', async ({page})=>{
  await page.goto('/');
  await expect(page.getByTestId('guest-chat-tab')).toHaveClass(/active/);
  await expect(page.getByRole('button',{name:'▶ Run',exact:true})).toHaveCount(0);
  await page.getByRole('textbox',{name:'Message Panda',exact:true}).fill('2 milk tea');
  await page.getByRole('button',{name:'Send',exact:true}).click();
  await expect(page.locator('.basket-line')).toContainText('2 × Hong Kong milk tea');
  await expect(page.getByRole('button',{name:'Send',exact:true})).toBeEnabled();
  await page.getByRole('textbox',{name:'Message Panda',exact:true}).fill('confirm order');
  await page.getByRole('button',{name:'Send',exact:true}).click();
  await expect(page.locator('.basket-line')).toHaveCount(0);
  await orders(page);
  await expect(page.locator('.order-card')).toContainText('HK$56');
  await login(page,OWNER);
  await expect(page.getByRole('button',{name:'▶ Run',exact:true})).toBeVisible();
});

test('default menu functions list, choose, remove selections and report progress',async({page},info)=>{
  await page.goto('/');
  const ask=async text=>{await page.getByLabel('Message Panda',{exact:true}).fill(text);await page.getByRole('button',{name:'Send',exact:true}).click();};
  await ask('menu list');
  await expect(page.locator('.chat-messages')).toContainText('Hong Kong milk tea · HK$28.00');
  await ask('choose menu');
  await expect(page.locator('.chat-messages')).toContainText('Choose an item and quantity');
  await page.getByRole('button',{name:'More options',exact:true}).click();
  await page.getByRole('button',{name:/^⌑ /}).click();
      await page.getByRole('button',{name:'Add Egg tart',exact:true}).click();
  await expect(page.locator('.basket-line')).toContainText('1 × Egg tart');
  await ask('cancel menu');
  await expect(page.locator('.chat-messages')).toContainText('Which item should I remove');
  await page.getByRole('textbox',{name:'Message Panda',exact:true}).fill('remove 1 egg tart');
  await page.getByRole('button',{name:'Send',exact:true}).click();
  await expect(page.locator('.basket-line')).toHaveCount(0);
  await ask('current progress');
  await expect(page.locator('.chat-messages')).toContainText('No submitted orders yet');
  await expect(page.locator('.order-card')).toHaveCount(0);
  await page.getByRole('textbox',{name:'Message Panda',exact:true}).fill('2 milk tea');
  await page.getByRole('button',{name:'Send',exact:true}).click();
  await expect(page.locator('.basket-line')).toContainText('2 × Hong Kong milk tea');
  await expect(page.getByRole('button',{name:'Send',exact:true})).toBeEnabled();
  await page.getByRole('textbox',{name:'Message Panda',exact:true}).fill('confirm order');
  await page.getByRole('button',{name:'Send',exact:true}).click();
  await expect(page.locator('.basket-line')).toHaveCount(0);
  await ask('current progress');
  await expect(page.locator('.chat-messages')).toContainText('new · unpaid · HK$56.00');
  await expect(page.locator('.order-card')).toHaveCount(1);
  await page.screenshot({path:info.outputPath('default-functions.png'),fullPage:true});
});

test('idle guest has a server event stream and makes no periodic order requests', async({page})=>{
  let rpcCount=0;
  page.on('request',r=>{if(r.url().endsWith('/api/rpc'))rpcCount++;});
  const connected=page.waitForResponse(r=>r.url().endsWith('/api/events')&&r.status()===200);
  await page.goto('/');await connected;
  await expect.poll(async()=>Number(await page.locator('.app').getAttribute('data-state-version'))).toBeGreaterThanOrEqual(2);
  const baseline=rpcCount;
  // Observe beyond the removed five-second interval: idle screens must send no order RPC.
  await page.waitForTimeout(6500);
  expect(rpcCount).toBe(baseline);
});

test('test faucet grants HK$1000 once, persists on reload and never calls AI',async({page},info)=>{
  await page.goto('/');
  await page.getByRole('button',{name:'More options',exact:true}).click();
  await page.getByRole('button',{name:'🎁 Faucet',exact:true}).click();
  await expect(page.getByRole('button',{name:'Get free HK$1000',exact:true})).toBeDisabled();
  await login(page,OWNER,'8');
  await page.getByRole('button',{name:'More options',exact:true}).click();
  await page.getByRole('button',{name:'🎁 Faucet',exact:true}).click();
  await expect(page.getByTestId('faucet-balance')).toHaveText('HK$0');
  await page.getByRole('button',{name:'Get free HK$1000',exact:true}).click();
  await expect(page.getByTestId('faucet-balance')).toHaveText('HK$1000');
  await expect(page.getByRole('button',{name:'Already claimed',exact:true})).toBeDisabled();
  await page.reload();
  await page.getByRole('button',{name:'More options',exact:true}).click();
  await page.getByRole('button',{name:'🎁 Faucet',exact:true}).click();
  await expect(page.getByTestId('faucet-balance')).toHaveText('HK$1000');
  await expect(page.getByRole('button',{name:'Already claimed',exact:true})).toBeDisabled();
  await page.screenshot({path:info.outputPath('test-faucet.png'),fullPage:true});
});

test('Owner switches simulation mode with confirmation and persistence', async ({page}) => {
  await page.goto('/'); await login(page, OWNER);
  await page.getByRole('button',{name:'More options',exact:true}).click();
  await page.getByRole('button',{name:'Shop management',exact:true}).click();
  await page.getByRole('button',{name:'shop',exact:true}).click();
  await expect(page.getByTestId('simulation-mode')).toContainText('Simulation');
  await page.getByRole('button',{name:'Switch to Live',exact:true}).click();
  await confirm(page);
  await expect(page.getByTestId('simulation-mode')).toContainText('Live');
  await page.reload();
  await page.getByRole('button',{name:'More options',exact:true}).click();
  await page.getByRole('button',{name:'Shop management',exact:true}).click();
  await page.getByRole('button',{name:'shop',exact:true}).click();
  await expect(page.getByTestId('simulation-mode')).toContainText('Live');
  await page.getByRole('button',{name:'Switch to Simulation',exact:true}).click();
  await confirm(page);
  await expect(page.getByTestId('simulation-mode')).toContainText('Simulation');
});

test('Guest simulation Ready and Pay buttons complete their meal', async ({page}) => {
  await page.goto('/');
  await page.getByRole('button',{name:'More options',exact:true}).click();
  await page.getByRole('button',{name:/^⌑ /}).click();
      await page.getByRole('button',{name:'Add Egg tart',exact:true}).click();
  await page.getByRole('button',{name:'Confirm order →',exact:true}).click();
  await expect(page.locator('.basket-line')).toHaveCount(0);
  await orders(page);
  await expect(page.getByRole('button',{name:'Pay · Finish eating',exact:true})).toBeDisabled();
  await page.getByRole('button',{name:'Ready · Food ready',exact:true}).click();
  await expect(page.locator('.order-card .status')).toHaveText('Ready');
  await page.getByRole('button',{name:'Pay · Finish eating',exact:true}).click();
  await expect(page.locator('.order-card')).toHaveCount(0);
  await page.getByRole('button',{name:'completed',exact:true}).click();
  await expect(page.locator('.order-card .status')).toHaveText('Completed');
  await page.reload(); await orders(page);
  await page.getByRole('button',{name:'completed',exact:true}).click();
  await expect(page.locator('.order-card .status')).toHaveText('Completed');
});


test('Guest starts with chat on desktop and mobile, with menu one tap away', async ({page}, info) => {
  await page.goto('/');
  await expect(page.locator('.table-tag')).toContainText('Guest');
  await expect(page.getByTestId('guest-chat-tab')).toHaveClass(/active/);
  await expect(page.locator('.content-panel')).toBeHidden();
  await expect(page.locator('.hero')).toHaveCount(0);
  const composer = page.getByRole('textbox',{name:'Message Panda',exact:true});
  await expect(composer).toBeVisible();
  await page.screenshot({path:info.outputPath('guest-chat-desktop.png'),fullPage:true});
  await page.setViewportSize({width:390,height:844});
  expect(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth)).toBe(true);
  const box=await composer.boundingBox();
  expect(box.y+box.height).toBeLessThanOrEqual(844);
  await page.screenshot({path:info.outputPath('guest-chat-mobile.png'),fullPage:true});
  await page.getByLabel('Message Panda',{exact:true}).fill('1 milk tea');
  await page.getByRole('button',{name:'Send',exact:true}).click();
  await expect(page.locator('.basket-line')).toHaveCount(1);
  await page.getByRole('button',{name:'More options',exact:true}).click();
  await page.locator('.topbar').getByRole('button',{name:/^⌑ /}).click();
  await expect(page.locator('.menu-grid')).toBeVisible();
  await page.getByRole('button',{name:'More options',exact:true}).click();
  await page.getByTestId('guest-chat-tab').click();
  await expect(page.locator('.content-panel')).toBeHidden();
  await expect(page.locator('.basket-line')).toHaveCount(1);
});

test('Guest orders for three people with three foods and three drinks through chat', async ({page}) => {
  await page.goto('/');
  const input=page.getByRole('textbox',{name:'Message Panda',exact:true});
  await input.fill('3명 메뉴 3개, 음료수 3개');
  await page.getByRole('button',{name:'Send',exact:true}).click();
  await expect(page.locator('.basket-line')).toHaveCount(6);
  await expect(page.locator('.chat-messages')).toContainText('아직 주문하지 않았습니다');
  const quotedTotal=await page.locator('.basket-total strong').last().textContent();
  expect(quotedTotal).toMatch(/HK\$/);
  await expect(page.locator('.chat-messages')).toContainText('최근 주문량');
  await input.fill('주문 확인');
  await page.getByRole('button',{name:'Send',exact:true}).click();
  await expect(page.locator('.basket-line')).toHaveCount(0);
  await orders(page);
  await expect(page.locator('.order-card')).toHaveCount(1);
  await expect(page.locator('.order-card')).toContainText(quotedTotal);
});


test('Guest wallet credits persist across independent logins', async ({browser}) => {
  for(let i=0;i<2;i++) {
    const context=await browser.newContext();
    try {
      const page=await context.newPage(); await page.goto('/');
      await login(page,STAFF);
      await expect(page.locator('.table-tag')).toContainText('Guest');
      await page.getByRole('button',{name:'More options',exact:true}).click();
      await page.getByRole('button',{name:'🎁 Faucet',exact:true}).click();
      if(i===0) await page.getByRole('button',{name:'Get free HK$1000',exact:true}).click();
      await expect(page.getByTestId('faucet-balance')).toHaveText('HK$1000');
      await expect(page.getByRole('button',{name:'Already claimed',exact:true})).toBeDisabled();
    } finally {await context.close();}
  }
});

test('Korean chat adds items and each basket row can be removed',async({page})=>{
 await page.goto('/');
 const input=page.getByRole('textbox',{name:'Message Panda',exact:true});
 for(const text of ['밀크티 2개','에그타르트 1개']) {
  await input.fill(text);await page.getByRole('button',{name:'Send',exact:true}).click();
  await expect(page.getByRole('button',{name:'Send',exact:true})).toBeEnabled();
 }
 await expect(page.locator('.basket-line')).toHaveCount(2);
 await expect(page.locator('.basket-line').first()).toContainText('2 × Hong Kong milk tea');
 await expect(page.locator('.basket-total')).toContainText('HK$74');
 await page.getByRole('button',{name:'Remove Hong Kong milk tea',exact:true}).click();
 await expect(page.locator('.basket-line')).toHaveCount(1);
 await expect(page.locator('.basket-total')).toContainText('HK$18');
 await page.getByRole('button',{name:'Remove Egg tart',exact:true}).click();
 await expect(page.locator('.basket-line')).toHaveCount(0);
 await orders(page);await expect(page.locator('.order-card')).toHaveCount(0);
});

test('Natural-language cancellation and staff call reach the owner service queue',async({browser})=>{
 const guestContext=await browser.newContext(), ownerContext=await browser.newContext();
 try {
  const guest=await guestContext.newPage(),owner=await ownerContext.newPage();
  await guest.goto('/');
  const input=guest.getByRole('textbox',{name:'Message Panda',exact:true});
  for(const text of ['밀크티 1개','주문 확인']) {
   await input.fill(text);await guest.getByRole('button',{name:'Send',exact:true}).click();
   await expect(guest.getByRole('button',{name:'Send',exact:true})).toBeEnabled();
  }
  await input.fill('주문 취소');await guest.getByRole('button',{name:'Send',exact:true}).click();
  await expect(guest.locator('.chat-messages')).toContainText('취소 완료');
  await guest.getByRole('button',{name:'More options',exact:true}).click();
  await guest.getByRole('button',{name:/^▤ Orders/}).click();
  await guest.getByRole('button',{name:'all',exact:true}).click();
  await expect(guest.locator('.order-card .status')).toHaveText('Cancelled');
  await input.fill('직원 불러주세요');await guest.getByRole('button',{name:'Send',exact:true}).click();
  await expect(guest.locator('.chat-messages')).toContainText('직원을 호출했습니다');
  await owner.goto('/');await login(owner,OWNER);
  const requests=owner.locator('.service-requests');
  await expect(requests).toContainText('직원 불러주세요');
  await requests.getByRole('button',{name:'응대 완료 / Resolved',exact:true}).click();
  await expect(requests.locator('.manage-card')).toHaveCount(0);
 } finally {await guestContext.close();await ownerContext.close();}
});
