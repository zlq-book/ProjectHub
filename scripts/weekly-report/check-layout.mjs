import {chromium} from 'playwright';
import fs from 'node:fs/promises';
const browser=await chromium.launch({headless:true,channel:'msedge'});
const page=await browser.newPage();
const errors=[];page.on('pageerror',e=>errors.push(e.message));
await page.goto('http://127.0.0.1:1420/');
await page.getByRole('button',{name:'AI 智能周报',exact:true}).click();
await page.getByRole('heading',{name:'AI 智能周报',exact:true}).waitFor();
const checks=[];
for(const width of [375,760,1060,1440]){
 await page.setViewportSize({width,height:1000});
 for(const view of ['准备数据','编辑周报']){
  await page.getByRole('tab',{name:new RegExp(view)}).click();
  const result=await page.evaluate(()=>({width:innerWidth,scroll:document.documentElement.scrollWidth}));
  checks.push({...result,view});if(result.scroll>result.width)throw new Error(`页面横向溢出 ${JSON.stringify(result)}`);
 }
}
await page.setViewportSize({width:1440,height:1050});
await page.getByRole('tab',{name:'准备数据',exact:true}).click();
await fs.mkdir('.verification/weekly-report',{recursive:true});
await page.screenshot({path:'.verification/weekly-report/prepare-desktop.png',fullPage:true});
await page.getByRole('tab',{name:'编辑周报',exact:true}).click();
await page.getByRole('button',{name:'新增成果',exact:true}).click();
await page.getByLabel('成果 1',{exact:true}).fill('人工补充的开发成果');
await page.getByRole('button',{name:'全部项目',exact:false}).first().click();
await page.getByRole('button',{name:'AI 智能周报',exact:true}).click();
if(await page.getByLabel('成果 1',{exact:true}).inputValue()!=='人工补充的开发成果')throw new Error('导航切换丢失草稿');
await page.screenshot({path:'.verification/weekly-report/editor-desktop.png',fullPage:true});
if(errors.length)throw new Error(errors.join('\n'));
await fs.writeFile('.verification/weekly-report/layout.json',JSON.stringify({checks,errors,navigationPreservesDraft:true},null,2));
console.log(JSON.stringify({checks,errors,navigationPreservesDraft:true}));await browser.close();
