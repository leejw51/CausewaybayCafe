import {test,expect} from '@playwright/test';
test('Owner runs virtual service at 100x and sees table updates without AI',async({page})=>{
 await page.goto('/');await page.getByRole('button',{name:'Wallet login',exact:true}).click();
 await page.getByLabel('Recovery phrase',{exact:true}).fill('abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about');
 await page.getByRole('button',{name:'Unlock café',exact:true}).click();await page.getByRole('dialog',{name:'Wallet login',exact:true}).waitFor({state:'hidden'});
 await page.getByRole('button',{name:'More options',exact:true}).click();await page.getByRole('button',{name:'◫ Dashboard',exact:true}).click();
 await expect(page.locator('.table-node').first()).toBeVisible();
 await page.getByLabel('Simulation speed',{exact:true}).selectOption('100');
 await page.getByRole('button',{name:'Start simulation',exact:true}).click();
 // A table may also have earlier new orders; the card prioritizes ready/new states.
 await expect(page.locator('.simulation-controls')).toContainText(/100×.*[1-9][0-9]* min/,{timeout:10000});
 await expect(page.locator('.table-node.ready').first()).toBeVisible({timeout:10000});
 await page.getByRole('button',{name:'Pause simulation',exact:true}).click();
 await expect(page.getByTestId('token-stats-hour')).toContainText('Input 0 · Output 0 · Reserved 0');
 await page.screenshot({path:'docs/previews/live-tables.png'});
});
