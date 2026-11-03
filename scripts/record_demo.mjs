// Explicit paid Meta demo. Run against scripts/e2e_server.py --live-demo.
// Login happens before recording; all wallets/data are disposable fixtures.
import { chromium } from '@playwright/test';
import { mkdir,writeFile } from 'node:fs/promises';
const root=new URL('../',import.meta.url).pathname;
const out=root+'artifacts/demo';await mkdir(out,{recursive:true});
const browser=await chromium.launch({headless:true});
const base='http://127.0.0.1:18890';
const ownerPhrase='abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about';
const staffPhrase='test test test test test test test test test test test junk';
async function login(words,index='0'){const context=await browser.newContext();const page=await context.newPage();await page.goto(base);await page.getByRole('button',{name:'Wallet login',exact:true}).click();await page.getByLabel('Recovery phrase',{exact:true}).fill(words);await page.getByLabel('Wallet index',{exact:true}).fill(index);await page.getByRole('button',{name:'Unlock café',exact:true}).click();await page.getByRole('dialog',{name:'Wallet login',exact:true}).waitFor({state:'hidden'});return {context,page,token:await page.evaluate(()=>sessionStorage.getItem('cafe-token'))};}
async function nav(page,name){await page.getByRole('button',{name:'More options',exact:true}).click();await page.getByRole('button',{name,exact:typeof name==='string'}).click();}
async function confirm(page){await page.locator('.confirmation').getByRole('button',{name:'Confirm',exact:true}).click();await page.locator('.confirmation').waitFor({state:'hidden'});}
const owner=await login(ownerPhrase);
await nav(owner.page,'Shop management');
const team=owner.page.locator('form').filter({hasText:'Appoint staff'});
await team.getByLabel('Name',{exact:true}).fill('Alex · Service');
await team.getByLabel('Wallet address',{exact:true}).fill('0x70997970c51812dc3a010c7d01b50e0d17dc79c8');
await team.getByRole('button',{name:'Review / 확인'}).click();await confirm(owner.page);
const staff=await login(staffPhrase,'1');
const context=await browser.newContext({viewport:{width:1440,height:900},recordVideo:{dir:out,size:{width:1440,height:900}}});
const page=await context.newPage();page.setDefaultTimeout(25000);
const video=page.video();const start=Date.now();const marks=[];
function mark(text){marks.push({seconds:Math.round((Date.now()-start)/1000),text});console.log(text);}
async function hold(ms){await page.waitForTimeout(ms);}
async function chat(text){await page.getByLabel('Message Panda',{exact:true}).fill(text);await hold(1800);await page.getByRole('button',{name:'Send',exact:true}).click();await page.getByRole('button',{name:'Send',exact:true}).waitFor({state:'visible'});await page.waitForFunction(()=>!document.querySelector('button.send').disabled,{},{timeout:90000});}
try{
 await page.goto(base+'/?table=2');await page.locator('.panda-stage').waitFor();
 await page.waitForFunction(()=>!!sessionStorage.getItem('cafe-token')&&!document.querySelector('button.send').disabled);
 const guestToken=await page.evaluate(()=>sessionStorage.getItem('cafe-token'));
 mark('A Muse-powered employee for a neighborhood cafe. Locally hosted on the owner’s Mac; AI runs through Meta API.');await hold(10000);
 mark('Guest: order naturally. Muse uses the menu and calls validated backend tools.');
 await chat('Please order two Hong Kong milk teas and one egg tart for table 2.');await hold(7000);
 await chat('What did I order, and what is the total?');await hold(6000);
 mark('An optional Muse Image illustration makes the order easier to recognize. The item list is authoritative.');
 await page.getByRole('button',{name:'Picture my order',exact:true}).click();
 await page.locator('.order-visual img').waitFor({timeout:190000});await hold(10000);
 await page.screenshot({path:out+'/guest-order.png'});
 mark('The same order reaches staff. Demo fulfillment; no real payment is processed.');
 await page.evaluate(token=>sessionStorage.setItem('cafe-token',token),staff.token);await page.reload();
 await nav(page,/^▤ Orders/);await hold(5000);
 const card=page.locator('.order-card').filter({hasText:'Hong Kong milk tea'}).first();
 for(const status of ['Preparing','Ready','Served']){await card.getByRole('button',{name:`→ ${status}`,exact:true}).click();await confirm(page);await hold(4000);}
 await card.getByRole('button',{name:'Received · cash',exact:true}).click();await confirm(page);await hold(4000);
 mark('Owner: monitor hourly and daily Meta usage, with token budgets controlling costs.');
 await page.evaluate(token=>sessionStorage.setItem('cafe-token',token),owner.token);await page.reload();await nav(page,'◫ Dashboard');await hold(10000);
 await page.getByLabel('Simulation speed',{exact:true}).selectOption('100');
 await page.getByRole('button',{name:'Start simulation',exact:true}).click();await hold(12000);await page.getByRole('button',{name:'Pause simulation',exact:true}).click();
 await page.screenshot({path:out+'/owner-dashboard.png'});
 await nav(page,'Shop management');await hold(7000);
 mark('One shop, one local database, role-checked operations. People focus on hospitality.');
 await page.evaluate(token=>sessionStorage.setItem('cafe-token',token),guestToken);await page.reload();await hold(5000);
 await page.locator('.layout-toggle').click();await hold(4000);await page.locator('.layout-toggle').click();
 const remaining=180000-(Date.now()-start);if(remaining>0)await hold(remaining);
}finally{
 await page.screenshot({path:out+'/last-frame.png'}).catch(()=>{});
 await writeFile(out+'/chapters.json',JSON.stringify(marks,null,2));
 await context.close();await owner.context.close();await staff.context.close();await browser.close();
 console.log('VIDEO '+await video.path());
}
