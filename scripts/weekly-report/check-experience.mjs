import {chromium} from 'playwright';
import fs from 'node:fs/promises';
const browser=await chromium.launch({headless:true,channel:'msedge'});
const page=await browser.newPage({viewport:{width:1440,height:1050}});
const errors=[];page.on('pageerror',e=>errors.push(e.message));
await page.goto('http://127.0.0.1:1420/');
await page.getByRole('heading',{name:'继续工作',exact:true}).waitFor();
await fs.mkdir('.verification/experience',{recursive:true});
const checks=[];
for(const theme of ['light','dark']){
 for(const width of [375,760,1060,1440]){
  await page.setViewportSize({width,height:1050});
  for(const route of ['首页','全部项目','开发状态中心','AI 智能周报']){
   await page.getByRole('navigation',{name:'主导航'}).getByRole('button',{name:route,exact:true}).click();
   await page.evaluate(theme=>{document.documentElement.dataset.theme=theme;},theme);
   const result=await page.evaluate(()=>({width:innerWidth,scroll:document.documentElement.scrollWidth}));
   checks.push({...result,theme,route});
   if(result.scroll>width)throw new Error(`横向溢出 ${JSON.stringify(checks.at(-1))}`);
   if(route==='AI 智能周报'){
    if(await page.getByRole('textbox',{name:'搜索项目',exact:true}).count())throw new Error('周报页仍显示无效项目搜索');
    await page.getByRole('tab',{name:/^编辑周报/}).click();
    if(await page.getByLabel('成果 1',{exact:true}).count()===0){await page.getByRole('button',{name:'新增成果',exact:true}).click();await page.getByLabel('成果 1',{exact:true}).fill('整理项目接口与异常处理，保留实际业务规则并补齐回归验证。');}
    const size=await page.evaluate(()=>document.documentElement.scrollWidth);
    if(size>width)throw new Error('编辑视图横向溢出');
    if(width===1440)await page.screenshot({path:`.verification/experience/weekly-${theme}.png`});
    await page.getByRole('tab',{name:'准备数据',exact:true}).click();
   }
   if(width===1440&&route==='首页')await page.screenshot({path:`.verification/experience/home-${theme}.png`});
   if(width===1440&&route==='开发状态中心')await page.screenshot({path:`.verification/experience/development-${theme}.png`});
  }
 }
}
await page.getByRole('navigation',{name:'主导航'}).getByRole('button',{name:'全部项目',exact:true}).click();
await page.getByRole('textbox',{name:'搜索项目',exact:true}).fill('env');
await page.getByRole('navigation',{name:'主导航'}).getByRole('button',{name:'AI 智能周报',exact:true}).click();
await page.keyboard.press('Control+k');
if(await page.getByRole('textbox',{name:'搜索项目',exact:true}).inputValue()!=='env')throw new Error('搜索未恢复');
if(!await page.getByRole('textbox',{name:'搜索项目',exact:true}).evaluate(el=>el===document.activeElement))throw new Error('Ctrl+K 未聚焦');
await page.getByRole('navigation',{name:'主导航'}).getByRole('button',{name:'设置',exact:true}).click();
await page.getByRole('dialog',{name:'设置',exact:true}).waitFor();
await page.screenshot({path:'.verification/experience/settings-dark.png'});
if(errors.length)throw new Error(errors.join('\n'));
await fs.writeFile('.verification/experience/layout.json',JSON.stringify({checks,errors,searchRestored:true,keyboardFocus:true},null,2));
console.log(JSON.stringify({views:checks.length,errors,searchRestored:true,keyboardFocus:true}));
await browser.close();
