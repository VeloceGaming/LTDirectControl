// Offline pixel comparison of the drawn HUD (1.6.0-perf-dev3) against the
// dev2 UI-tree HUD. A layout test in a headless browser, not the game's
// renderer: both sides go through the same canvas code, so what it checks
// is that the draw lists put the same boxes, texts and glyphs in the same
// places, colours, sizes and z order as the dev2 nodes did.
//
//   node tools/compare_draw_review.cjs [--keep]
//
// Inputs (made by `cargo test` in probe/ with LT_UI_EXPORT_DIR, see
// probe/src/ui_identity.rs):
//   probe/testdata/ui_pixels/dev2/<window>/  record.txt + <fnv>.ui (dev2,
//     the fixture: exported before any template changed)
//   research/ui-pixels/dev3/<window>/  record.txt + <fnv>.ui + draw_*.txt
// For each review state (a window and a frame of its scripted test) both
// sides replay the write record up to that frame onto a model of the tree
// (templates, property writes, texts, visibility; node layout as in
// probe/src/fake_layout.rs), dev3 adds the frame's draw commands, and each
// is rendered flat on a 1920 x 1080 canvas sorted by (z, order). Prints the
// differing pixel count and the largest differing region per state.
// --keep writes the PNGs and a diff image to research/ui-pixels/out/.
const fs=require('fs'),path=require('path');
// The author's Node modules and browser, as the other review tools use them;
// override with LT_NODE_MODULES (a folder ending in /) and LT_EDGE.
const MODULES=process.env.LT_NODE_MODULES||'C:/Users/j9010/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/';
const EDGE=process.env.LT_EDGE||'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe';
const {chromium}=require(MODULES+'playwright');
const sharp=require(MODULES+'sharp');
const pixelmatchModule=require(MODULES+'pixelmatch');
const pixelmatch=pixelmatchModule.default||pixelmatchModule;
const root=path.resolve(__dirname,'..');
const DEV2=path.join(root,'probe/testdata/ui_pixels/dev2');
const DEV3=path.join(root,'research/ui-pixels/dev3');
const OUT=path.join(root,'research/ui-pixels/out');
const keep=process.argv.includes('--keep');
const only=process.argv.slice(2).filter(a=>!a.startsWith('--'));

// Review states: [window folder, frame, label].
const STATES=[
 ['hud',5,'HUD up, waiting for Start (not playing)'],
 ['hud',8,'control started, no sample yet'],
 ['hud',14,'alive, all skills ready'],
 ['hud',60,'Q and W cooling'],
 ['hud',262,'cooling with a denied flash, R multi-use refill'],
 ['hud',160,'Q hovered (tooltip open)'],
 ['hud',195,'next purchase tile hovered'],
 ['hud',220,'shop button hovered'],
 ['hud',230,'purchase, feedback banner'],
 ['hud',250,'purchase made, level up'],
 ['hud',290,'R multi-use, W aiming'],
 ['hud',315,'low health'],
 ['hud',340,'dead (respawn count, veils, plate)'],
 ['hud',380,'respawned, recalling'],
 ['hud',400,'new background colour'],
 ['hud',430,'paused (plate)'],
 ['hud',450,'AI control (plate)'],
 ['session',5,'session strip, Loading'],
 ['session',10,'session strip, Ready (chooser)'],
 ['session',20,'Ready, lane hovered'],
 ['session',30,'Running'],
 ['session',45,'Running, Pause hovered (tooltip)'],
 ['session',49,'Running, Pause pressed'],
 ['session',66,'own-team vision hovered'],
 ['session',110,'swap column open'],
 ['session',120,'swap column folding'],
 ['session',135,'Paused'],
 ['session',145,'AI control'],
 ['session',155,'AI control, Take control hovered'],
 ['stats',20,'stats panels, champion target'],
 ['stats',70,'own panel only (target cleared)'],
 ['effect',58,'low-health tint'],
 ['effect',80,'death veil'],
];

// ---- Templates and records ------------------------------------------------
function closing(s,open){let depth=0,quoted=false,escape=false;for(let i=open;i<s.length;i++){const c=s[i];if(quoted){if(escape)escape=false;else if(c==='\\')escape=true;else if(c==='"')quoted=false;continue;}if(c==='"')quoted=true;else if(c==='{')depth++;else if(c==='}'&&--depth===0)return i;}throw Error('unbalanced');}
function fields(src){const out={};let start=0,depth=0,quoted=false,escape=false;const part=t=>{const i=t.indexOf(':');if(i>0)out[t.slice(0,i).trim()]=t.slice(i+1).trim();};for(let i=0;i<src.length;i++){const c=src[i];if(quoted){if(escape)escape=false;else if(c==='\\')escape=true;else if(c==='"')quoted=false;continue;}if(c==='"')quoted=true;else if(c==='{')depth++;else if(c==='}')depth--;else if(c===';'&&depth===0){part(src.slice(start,i));start=i+1;}}part(src.slice(start));return out;}
// A node body: own field text and children [{id, type, body}].
function split(body){let own='',children=[],i=0,quoted=false;while(i<body.length){const c=body[i];if(quoted){own+=c;if(c==='"'&&body[i-1]!=='\\')quoted=false;i++;continue;}if(c==='"'){quoted=true;own+=c;i++;continue;}if(c==='#'){const m=/^#([A-Za-z0-9_]+):([A-Za-z0-9_]+)\s*\{/.exec(body.slice(i));if(m){const open=i+m[0].length-1,close=closing(body,open);children.push({id:m[1],type:m[2],body:body.slice(open+1,close)});i=close+1;continue;}}if(c==='{'){const close=closing(body,i);own+=body.slice(i,close+1);i=close+1;continue;}own+=c;i++;}return{own,children};}
class Tree{
 constructor(){this.nodes=new Map();this.seq=0;}
 // A root's visibility at spawn follows the fake tree's rule (fake_ui.rs:
 // `visible: false` before the first `#` of the first line), which the
 // record was made against: the game asked whether a root is shown, and the
 // record holds a visibility write only where the fake said no.
 spawn(parent,source){const m=/^\s*([A-Za-z0-9_]+):([A-Za-z0-9_]+)\s*\{/.exec(source);const open=source.indexOf('{'),close=closing(source,open);const p=parent+'.'+m[1];this.add(p,m[2],source.slice(open+1,close));const header=source.split('\n')[0].split('#')[0];this.nodes.get(p).props.visible=header.includes('visible: false')?'false':'true';}
 add(p,type,body){const {own,children}=split(body);const props=fields(own);this.nodes.set(p,{path:p,type,props,text:null,seq:this.seq++});for(const c of children)this.add(p+'.'+c.id,c.type,c.body);}
 set(p,src){const n=this.nodes.get(p);if(!n)return;Object.assign(n.props,fields(src));}
 remove(p){for(const k of [...this.nodes.keys()])if(k===p||k.startsWith(p+'.'))this.nodes.delete(k);}
 visible(p){if(p==='ingame')return true;const n=this.nodes.get(p);if(!n)return true;if(n.props.visible==='false')return false;return this.visible(p.slice(0,p.lastIndexOf('.')));}
 len(v,parent){if(v==null)return 0;v=String(v).trim();if(v.endsWith('%'))return parent*parseFloat(v)/100;return parseFloat(v);}
 rect(p){if(p==='ingame'||!this.nodes.has(p))return[0,0,1920,1080];const n=this.nodes.get(p),pr=this.rect(p.slice(0,p.lastIndexOf('.'))),q=n.props;const w=this.len(q.width,pr[2]),h=this.len(q.height,pr[3]);const ax=parseFloat(q.anchor_x||0),ay=parseFloat(q.anchor_y||0),px=parseFloat(q.pivot_x||0),py=parseFloat(q.pivot_y||0);return[pr[0]+pr[2]*ax+this.len(q.x,pr[2])-w*px,pr[1]+pr[3]*ay+this.len(q.y,pr[3])-h*py,w,h];}
 z(p){const n=this.nodes.get(p);if(!n)return 0;if(n.props.z!=null)return parseInt(n.props.z);return this.z(p.slice(0,p.lastIndexOf('.')));}
}
// Replay a record (and its templates) up to and including `frame`.
function replay(dir,frame){
 const lines=fs.readFileSync(path.join(dir,'record.txt'),'utf8').replace(/\r\n/g,'\n').split('\n');
 const tree=new Tree();let index=-1,last=null;
 for(const line of lines){
  if(line.startsWith('## ')){index++;if(index>frame)break;last=null;continue;}
  if(index<0)continue;
  const k=line.slice(0,2);
  if(k==='S '){const m=/^S (\S+) len=\d+ fnv=([0-9a-f]+)$/.exec(line);const parent=m[1].slice(0,m[1].lastIndexOf('.'));tree.spawn(parent,fs.readFileSync(path.join(dir,m[2]+'.ui'),'utf8'));last=null;}
  else if(k==='P '){const i=line.indexOf(' | ');tree.set(line.slice(2,i),line.slice(i+3));last=null;}
  else if(k==='T '){const i=line.indexOf(' = ');const n=tree.nodes.get(line.slice(2,i));if(n){n.text=line.slice(i+3);last=n;}}
  else if(k==='V '){const [,p,v]=line.split(' ');const n=tree.nodes.get(p);if(n)n.props.visible=v;last=null;}
  else if(k==='X '){tree.remove(line.slice(2));last=null;}
  else if(k==='R '||line.startsWith('# ')){last=null;}
  else if(last){last.text+='\n'+line;}
 }
 if(index<frame)throw Error(dir+': no frame '+frame);
 return tree;
}
function color(v){if(!v)return null;const m=/#([0-9a-fA-F]{8})/.exec(v);return m?m[1].toLowerCase():null;}
function unquote(v){return v&&v.startsWith('"')?JSON.parse(v):v;}
const FONTS={'asset/lt_direct_control/font/numeric':'numeric','asset/lt_direct_control/font/medium':'medium'};
// The tree's visible nodes as flat primitives.
function treePrims(tree,prims){
 for(const n of [...tree.nodes.values()].sort((a,b)=>a.seq-b.seq)){
  if(!tree.visible(n.path))continue;
  const [x,y,w,h]=tree.rect(n.path),z=tree.z(n.path),q=n.props,order=n.seq;
  const rounding=/rounding:\s*(\d+)/.exec(q.rounding||'');const r=rounding?+rounding[1]:0;
  if(n.type==='color'){const c=color(q.color);if(c)prims.push({kind:'rect',x,y,w,h,z,r,c,order});}
  else if(n.type==='color_icon_button'){const b=fields((q.btn||'{}').replace(/^\{|\}$/g,''));const fill=color(b.back_color),edge=color(b.color),stroke=parseFloat(b.stroke||1);const rr=/rounding:\s*(\d+)/.exec(b.rounding||'');prims.push({kind:'button',x,y,w,h,z,r:rr?+rr[1]:2,fill,edge,stroke,order});}
  else if(n.type==='label'){const text=n.text!=null?n.text:(unquote(q.text)||'');if(!text)continue;const font=FONTS[unquote(q.font)]||'medium';prims.push({kind:'text',x,y,w,h,z,text,font,size:parseFloat(q.size||16),c:color(q.color)||'ffffffff',ax:(q.align_x||'Left')[0],ay:(q.align_y||'Top')[0],outline:q.outline==='1'?color(q.outline_color)||'000000ff':null,order});}
  else if(n.type==='image'){const src=unquote(q.source||'');const tint=color(q.color)||'ffffffff';const mod=/^asset\/lt_direct_control\/ui\/(.+)$/.exec(src||'');prims.push({kind:'image',x,y,w,h,z,file:mod&&!q.rect_tag?mod[1]:null,key:(src||'none')+'|'+(q.rect_tag||''),tint,smooth:q.sample_linear!=='false',order});}
 }
}
// A frame's draw commands as primitives (after the tree at equal z).
function drawPrims(dir,frame,prims){
 for(const f of fs.readdirSync(dir).filter(f=>/^draw_.*\.txt$/.test(f)).sort()){
  // Frames in order; `= as before` repeats the previous frame's commands.
  const frames=[];
  for(const line of fs.readFileSync(path.join(dir,f),'utf8').replace(/\r\n/g,'\n').split('\n')){
   if(line.startsWith('## '))frames.push([]);
   else if(line==='= as before')frames[frames.length-1].push(...frames[frames.length-2]);
   else if(line&&frames.length)frames[frames.length-1].push(line);
  }
  let k=0;
  for(const line of frames[frame]||[]){
   const order=1e6+(k++);const t=line.split(' ');
   if(t[0]==='R'){const [x,y,w,h]=t[1].split(',').map(Number);prims.push({kind:'rect',x,y,w,h,z:+t[2],r:+t[3].slice(2),c:t[4].slice(1),order});}
   else if(t[0]==='T'){const m=/^T (\S+) (-?\d+) (\S+) (\S+) #([0-9a-f]{8}) ([LCR])([TCB]) (".*")$/.exec(line);const [x,y,w,h]=m[1].split(',').map(Number);prims.push({kind:'text',x,y,w,h,z:+m[2],font:m[3],size:+m[4],c:m[5],ax:m[6],ay:m[7],text:JSON.parse(m[8]),outline:null,order});}
   else if(t[0]==='I'){const [x,y]=t[1].split(',').map(Number);const file=t[4].replace(/^asset\/lt_direct_control\/ui\//,'');prims.push({kind:'sprite',x,y,z:+t[2],file,smooth:t[3]==='smooth',order});}
   else throw Error('unknown draw line '+line);
  }
 }
}
function dataUrl(file,type){return 'data:'+type+';base64,'+fs.readFileSync(file).toString('base64');}

// ---- The page: one canvas, the same code for both sides --------------------
const PAGE=`<!doctype html><meta charset="utf-8"><style>
@font-face{font-family:numeric;src:url(${dataUrl(path.join(root,'probe/font/manrope_numeric.ttf'),'font/ttf')})}
@font-face{font-family:medium;src:url(${dataUrl(path.join(root,'probe/font/manrope_medium.ttf'),'font/ttf')})}
body{margin:0;background:#000}</style><canvas id=c width=1920 height=1080></canvas>`;
async function renderInPage(prims,images){
 const ctx=document.getElementById('c').getContext('2d');
 await document.fonts.load('20px numeric');await document.fonts.load('20px medium');
 const load=src=>new Promise(r=>{const i=new Image();i.onload=()=>r(i);i.src=src;});
 const imgs={};for(const [k,v] of Object.entries(images))imgs[k]=await load(v);
 const rgba=c=>'rgba('+parseInt(c.slice(0,2),16)+','+parseInt(c.slice(2,4),16)+','+parseInt(c.slice(4,6),16)+','+(parseInt(c.slice(6,8),16)/255)+')';
 // The battlefield: a fixed, textured backdrop so alpha shows.
 ctx.fillStyle='#3c4a3a';ctx.fillRect(0,0,1920,1080);for(let i=0;i<1920;i+=40){ctx.fillStyle=i%80?'#46553f':'#33402f';ctx.fillRect(i,0,20,1080);}
 const box=(x,y,w,h,r)=>{ctx.beginPath();if(r>0&&ctx.roundRect)ctx.roundRect(x,y,w,h,r);else ctx.rect(x,y,w,h);};
 const hash=s=>{let h=2166136261;for(const ch of s)h=Math.imul(h^ch.charCodeAt(0),16777619);return h>>>0;};
 const text=(p,dx,dy,c)=>{ctx.font=p.size+'px '+p.font;ctx.fillStyle=rgba(c);ctx.textAlign={L:'left',C:'center',R:'right'}[p.ax];ctx.textBaseline={T:'top',C:'middle',B:'bottom'}[p.ay];const x=p.ax==='L'?p.x:p.ax==='C'?p.x+p.w/2:p.x+p.w;const lines=p.text.split('\n');const lh=p.size*1.2;let y=p.ay==='T'?p.y:p.ay==='C'?p.y+p.h/2-(lines.length-1)*lh/2:p.y+p.h-(lines.length-1)*lh;for(const l of lines){ctx.fillText(l,x+dx,y+dy);y+=lh;}};
 const tinted=(img,w,h,tint,smooth)=>{const c=document.createElement('canvas');c.width=Math.max(1,Math.round(w));c.height=Math.max(1,Math.round(h));const g=c.getContext('2d');g.imageSmoothingEnabled=smooth;g.drawImage(img,0,0,c.width,c.height);const d=g.getImageData(0,0,c.width,c.height);const t=[0,2,4,6].map(i=>parseInt(tint.slice(i,i+2),16)/255);for(let i=0;i<d.data.length;i+=4)for(let k=0;k<4;k++)d.data[i+k]=Math.round(d.data[i+k]*t[k]);g.putImageData(d,0,0);return c;};
 prims.sort((a,b)=>a.z-b.z||a.order-b.order);
 for(const p of prims){
  if(p.kind==='rect'){ctx.fillStyle=rgba(p.c);box(p.x,p.y,p.w,p.h,p.r);ctx.fill();}
  else if(p.kind==='button'){// The runner's look as the review renders it: a border `stroke` px wide inside the box, the fill within it (an opaque fill covers a whole-box border; a see-through one gets four border strips).
   const s=p.stroke,opaque=p.fill&&parseInt(p.fill.slice(6),16)===255;
   if(p.edge&&opaque){ctx.fillStyle=rgba(p.edge);box(p.x,p.y,p.w,p.h,p.r);ctx.fill();}
   else if(p.edge&&s>0){ctx.fillStyle=rgba(p.edge);for(const [x,y,w,h] of [[p.x,p.y,p.w,s],[p.x,p.y+p.h-s,p.w,s],[p.x,p.y+s,s,p.h-2*s],[p.x+p.w-s,p.y+s,s,p.h-2*s]])ctx.fillRect(x,y,w,h);}
   if(p.fill){ctx.fillStyle=rgba(p.fill);box(p.x+s,p.y+s,p.w-2*s,p.h-2*s,Math.max(0,p.r-s));ctx.fill();}}
  else if(p.kind==='text'){if(p.outline){ctx.save();ctx.font=p.size+'px '+p.font;ctx.lineWidth=2;ctx.strokeStyle=rgba(p.outline);ctx.textAlign={L:'left',C:'center',R:'right'}[p.ax];ctx.textBaseline={T:'top',C:'middle',B:'bottom'}[p.ay];const x=p.ax==='L'?p.x:p.ax==='C'?p.x+p.w/2:p.x+p.w;const y=p.ay==='T'?p.y:p.ay==='C'?p.y+p.h/2:p.y+p.h;ctx.strokeText(p.text,x,y);ctx.restore();}text(p,0,0,p.c);}
  else if(p.kind==='image'){if(p.file&&imgs[p.file])ctx.drawImage(tinted(imgs[p.file],p.w,p.h,p.tint,p.smooth),p.x,p.y);else{const h=hash(p.key);ctx.fillStyle='rgba('+(h&255)+','+(h>>8&255)+','+(h>>16&255)+','+(parseInt(p.tint.slice(6),16)/255)+')';ctx.fillRect(p.x,p.y,p.w,p.h);}}
  else if(p.kind==='sprite'){const i=imgs[p.file];ctx.imageSmoothingEnabled=p.smooth;ctx.drawImage(i,p.x,p.y);}
 }
 return document.getElementById('c').toDataURL('image/png');
}
function images(prims){const out={};for(const p of prims){if(p.file&&!out[p.file]){const f=path.join(root,'probe/ui',p.file+'.png');if(fs.existsSync(f))out[p.file]=dataUrl(f,'image/png');}}return out;}
async function render(page,prims){const url=await page.evaluate(([p,i])=>renderInPage(p,i),[prims,images(prims)]);return Buffer.from(url.split(',')[1],'base64');}
async function raw(png){const {data,info}=await sharp(png).ensureAlpha().raw().toBuffer({resolveWithObject:true});return{data,info};}
// The largest 8-connected region of differing pixels: [pixels, x, y, w, h].
function largest(mask,w,h){const seen=new Uint8Array(w*h);let best=[0,0,0,0,0];const stack=[];for(let s=0;s<w*h;s++){if(!mask[s]||seen[s])continue;let n=0,x0=w,y0=h,x1=0,y1=0;stack.push(s);seen[s]=1;while(stack.length){const i=stack.pop();n++;const x=i%w,y=(i/w)|0;x0=Math.min(x0,x);y0=Math.min(y0,y);x1=Math.max(x1,x);y1=Math.max(y1,y);for(let dy=-1;dy<=1;dy++)for(let dx=-1;dx<=1;dx++){const X=x+dx,Y=y+dy;if(X<0||Y<0||X>=w||Y>=h)continue;const j=Y*w+X;if(mask[j]&&!seen[j]){seen[j]=1;stack.push(j);}}}if(n>best[0])best=[n,x0,y0,x1-x0+1,y1-y0+1];}return best;}

(async()=>{
 const browser=await chromium.launch({executablePath:EDGE,headless:true});
 if(keep)fs.mkdirSync(OUT,{recursive:true});
 let worst=0;const rows=[];
 try{
  const page=await browser.newPage({viewport:{width:1920,height:1080}});
  await page.setContent(PAGE);await page.addScriptTag({content:renderInPage.toString()});
  for(const [win,frame,label] of STATES.filter(s=>!only.length||only.includes(s[0]))){
   const t2=replay(path.join(DEV2,win),frame),t3=replay(path.join(DEV3,win),frame);
   const a=[];treePrims(t2,a);
   const b=[];treePrims(t3,b);
   const nodeCount=t=>[t.nodes.size,[...t.nodes.keys()].filter(k=>t.visible(k)).length];
   const [n2,v2]=nodeCount(t2),[n3,v3]=nodeCount(t3);const tree3=b.length;drawPrims(path.join(DEV3,win),frame,b);
   const [pa,pb]=[await render(page,a),await render(page,b)];
   const [ra,rb]=[await raw(pa),await raw(pb)];
   const diff=Buffer.alloc(ra.data.length);
   const count=pixelmatch(ra.data,rb.data,diff,1920,1080,{threshold:0.1,includeAA:false});
   const mask=new Uint8Array(1920*1080);for(let i=0;i<mask.length;i++)mask[i]=diff[i*4]===255&&diff[i*4+1]===0&&diff[i*4+2]===0?1:0;
   const [n,x,y,w,h]=largest(mask,1920,1080);
   worst=Math.max(worst,w,h);
   rows.push(`${win.padEnd(7)} ${String(frame).padStart(3)}  ${label.padEnd(48)} nodes ${String(n2).padStart(3)} (${String(v2).padStart(3)} shown) -> ${String(n3).padStart(3)} (${String(v3).padStart(3)} shown), ${String(b.length-tree3).padStart(3)} drawn  differing px ${String(count).padStart(5)}  largest region ${n} px ${w}x${h} at (${x},${y})`);
   if(keep){const name=`${win}-${frame}`;fs.writeFileSync(path.join(OUT,name+'-dev2.png'),pa);fs.writeFileSync(path.join(OUT,name+'-dev3.png'),pb);await sharp(diff,{raw:{width:1920,height:1080,channels:4}}).png().toFile(path.join(OUT,name+'-diff.png'));}
  }
 }finally{await browser.close();}
 console.log('window frame state                                           tree nodes dev2 -> dev3, draw commands      pixel comparison');
 for(const r of rows)console.log(r);
 console.log(`Largest differing region over all states: ${worst} px across.`);
})().catch(e=>{console.error(e);process.exitCode=1;});
