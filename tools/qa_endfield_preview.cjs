// Browser validation of the HTML proposal, not native game verification.
const {chromium}=require('C:/Users/j9010/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright');
const path=require('path'),fs=require('fs');
const out=path.resolve(__dirname,'../research/endfield-preview');fs.mkdirSync(out,{recursive:true});
(async()=>{
 const browser=await chromium.launch({executablePath:'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',headless:true});
 const page=await browser.newPage({viewport:{width:1920,height:1080},deviceScaleFactor:1});
 const errors=[],resources=[];page.on('pageerror',e=>errors.push(e.message));page.on('response',r=>{if(r.status()>=400&&!r.url().endsWith('/favicon.ico'))resources.push(r.url()+' '+r.status());});
 const settle=async p=>{await p.evaluate(async()=>{const docs=[document,...[...document.querySelectorAll('iframe')].map(f=>f.contentDocument).filter(Boolean)];await Promise.all(docs.map(d=>d.fonts.ready));await Promise.all(docs.flatMap(d=>d.getAnimations()).filter(a=>Number.isFinite(a.effect.getComputedTiming().iterations)).map(a=>a.finished.catch(()=>{})));});};
 await page.goto('http://127.0.0.1:8766/compare-endfield.html');
 await page.waitForFunction(()=>document.querySelector('#status').textContent.startsWith('All previews loaded'));
 await settle(page);await page.screenshot({path:path.join(out,'comparison-playing.png'),fullPage:true});
 for(const scene of ['skill','item','menu','unknown','before','tab','low','dead','recall']){
   await page.evaluate(s=>window.comparison.setScene(s),scene);
   await settle(page);await page.screenshot({path:path.join(out,'comparison-'+scene+'.png'),fullPage:true});
 }
 await page.evaluate(()=>window.comparison.setScene('menu'));
 const next=page.frame({url:/review-endfield/});
 await next.locator('#stage input[data-slider]').fill('49');
 if(await next.locator('#stage .slider strong').innerText()!=='49 px')throw Error('Cursor value did not update');
 await next.locator('#stage [data-action="vision-other"]').click();
 if(await next.locator('#stage [data-action="vision-other"]').getAttribute('aria-pressed')!=='true')throw Error('Vision selection did not update');
 await next.locator('#stage [data-action="effect"]').click();
 if(await next.locator('#stage [data-action="effect"]').getAttribute('aria-pressed')!=='false')throw Error('Effect toggle did not update');
 await next.locator('#stage [data-action="reset"]').click();
 if(await next.locator('#stage .slider strong').innerText()!=='32 px')throw Error('Cursor reset failed');
 for(const value of ['24','64']){
   await next.locator('#stage input[data-slider]').fill(value);
   const bounds=await next.locator('#stage .slider').evaluate(el=>{const thumb=el.querySelector('b').getBoundingClientRect(),track=el.querySelector('.slider-track').getBoundingClientRect();return {left:thumb.left>=track.left-.5,right:thumb.right<=track.right+.5};});
   if(!bounds.left||!bounds.right)throw Error('Cursor capsule overflows track at '+value);
 }
 await page.evaluate(()=>window.comparison.setScene('skill'));
 await page.locator('#zh').check();
 await settle(page);await page.screenshot({path:path.join(out,'comparison-chinese.png'),fullPage:true});
 await page.locator('#zh').uncheck();
 await page.setViewportSize({width:1401,height:790});
 await settle(page);await page.screenshot({path:path.join(out,'comparison-1401.png'),fullPage:true});
 const fonts=await next.evaluate(async()=>{await document.fonts.ready;return {manrope:document.fonts.check('500 27px Manrope'),chinese:document.fonts.check('500 20px "Noto Sans TC"')};});
 const standalone=await browser.newPage({viewport:{width:1920,height:1080},deviceScaleFactor:1});
 await standalone.goto('http://127.0.0.1:8766/review-endfield.html');await standalone.waitForFunction(()=>window.hudReview);
 await standalone.evaluate(()=>{window.hudReview.setPreview({phase:'fight',motion:false});window.hudReview.showTip('Q');});
 await settle(standalone);await standalone.locator('#stage .ef-tooltip').screenshot({path:path.join(out,'new-tooltip.png')});
 await standalone.evaluate(()=>window.hudReview.setPreview({phase:'fight',menu:true}));
 await settle(standalone);await standalone.locator('#stage .ef-menu').screenshot({path:path.join(out,'new-menu.png')});
 console.log(JSON.stringify({fonts,errors,resources,states:9,interactions:['slider','vision','effect','reset','Chinese','1401 viewport']},null,2));
 fs.writeFileSync(path.join(out,'results.json'),JSON.stringify({fonts,errors,resources},null,2));
 await browser.close();if(errors.length||resources.length)process.exitCode=1;
})().catch(e=>{console.error(e);process.exitCode=1;});
