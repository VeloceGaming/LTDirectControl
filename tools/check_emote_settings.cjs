// Layout approximation from the exported native template and Rust geometry fixtures.
// This checks bounds/asset paths; it does not verify native game rendering or input.
const fs = require('fs'), path = require('path');
const { chromium } = require('C:/Users/j9010/.cache/codex-runtimes/codex-primary-runtime/dependencies/node/node_modules/playwright');
const root = path.resolve(__dirname, '..');
const out = process.env.LT_HUD_EXPORT_DIR || path.join(root, 'research/emote-settings-80.0');
function close(s, start) {
  let depth = 0, quote = false, escape = false;
  for (let i = start; i < s.length; i++) {
    const c = s[i];
    if (quote) { if (escape) escape = false; else if (c === '\\') escape = true; else if (c === '"') quote = false; continue; }
    if (c === '"') quote = true;
    else if (c === '{') depth++;
    else if (c === '}' && --depth === 0) return i;
  }
  throw Error('Unbalanced native template');
}
function parse(s) {
  const nodes = []; let own = '', at = 0; const re = /#(\w+):(\w+)\s*\{/g;
  for (;;) {
    re.lastIndex = at; const m = re.exec(s); if (!m) break;
    own += s.slice(at, m.index); const open = re.lastIndex - 1, end = close(s, open);
    nodes.push({ name: m[1], type: m[2], ...parse(s.slice(open + 1, end)) }); at = end + 1;
  }
  return { own: own + s.slice(at), nodes };
}
function top(s) {
  let result = '', quote = false, escape = false;
  for (let i = 0; i < s.length; i++) {
    const c = s[i];
    if (quote) { result += c; if (escape) escape = false; else if (c === '\\') escape = true; else if (c === '"') quote = false; }
    else if (c === '"') { quote = true; result += c; }
    else if (c === '{') { i = close(s, i); result += ' '; }
    else result += c;
  }
  return result;
}
function prop(s, key) {
  const m = [...top(s).matchAll(new RegExp('(?:^|[;\\s])' + key + ':\\s*("(?:\\\\.|[^"\\\\])*"|[^;]+);', 'g'))].pop();
  return m ? m[1].trim() : null;
}
function block(s, key) {
  const m = [...s.matchAll(new RegExp('\\b' + key + ':\\s*\\{', 'g'))].pop();
  return m ? s.slice(m.index + m[0].length, close(s, m.index + m[0].length - 1)) : '';
}
const source = fs.readFileSync(path.join(out, 'settings.ui'), 'utf8');
const views = JSON.parse(fs.readFileSync(path.join(out, 'emote-previews.json'), 'utf8'));
const escapeHtml = s => s.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('"', '&quot;');
function document(view) {
  const tree = parse(source.slice(source.indexOf('{') + 1, source.lastIndexOf('}')));
  const window = tree.nodes.find(n => n.name === 'window');
  const get = (node, name) => { const n = node.nodes.find(n => n.name === name); if (!n) throw Error('Missing ' + name); return n; };
  const set = (node, key, value) => { node.own += ` ${key}: ${value};`; };
  const text = (node, copy) => set(node, 'text', JSON.stringify(copy));
  set(window, 'visible', 'true');
  for (const n of window.nodes) {
    if (/^(row\d+|sec\d+|head\d+|hintbox|cursor_ex|acquisition|advanced_nav|popup|conflict|replace|dismiss|scroll)/.test(n.name)) set(n, 'visible', 'false');
  }
  const titles = ['Combat & casting', 'Keybinds', 'Camera', 'Interface', 'Emotes', 'Advanced'];
  titles.forEach((title, i) => { const nav = get(window, 'nav' + i); text(get(nav, 'label'), title); nav.own += `btn: { back_color: ${i === 4 ? '#fdee00ff' : '#1c1a18ff'}; color: #00000000; stroke: 0; }`; set(get(nav, 'label'), 'color', i === 4 ? '#393939ff' : '#cbc9c7ff'); });
  text(get(window, 'page_title'), 'Emotes');
  text(get(window, 'page_hint'), 'Adjust emote placement, size and timing. Preview changes before applying.');
  text(get(window, 'position'), '05 / 06');
  text(get(window, 'pause'), 'Paused for settings');
  text(get(window, 'return'), 'Match resumes when you close this window');
  const panel = get(window, 'emote_panel'); set(panel, 'visible', 'true');
  const library = get(panel, 'library'), display = get(panel, 'display');
  set(library, 'visible', view.library ? 'true' : 'false');
  set(display, 'visible', view.library ? 'false' : 'true');
  for(let i=0;i<2;i++) get(panel,'tab'+i).own += `btn: {back_color: ${i===(view.library?0:1)?'#fdee00ff':'#393939ff'}; color: #6d6c6aff; stroke: 1;} text: {text: ${JSON.stringify(i?'Display':'Library')}; size:17; color: ${i===(view.library?0:1)?'#393939ff':'#eeececff'};}`;
  if(view.library) {
    const names=['gg','nice','hype','oops','focus'];
    for(let i=0;i<24;i++) {
      const cell=get(library,'cell'+i); set(cell,'visible',i<5?'true':'false');
      set(get(cell,'face'),'source',JSON.stringify('asset/lt_direct_control/ui/emote_'+names[i%5]));
      set(get(cell,'pending'),'visible','false');
    }
    for(let i=0;i<5;i++) set(get(get(library,'slot'+i),'face'),'source',JSON.stringify('asset/lt_direct_control/ui/emote_'+names[i]));
    text(get(library,'selected'),'GG'); text(get(library,'count'),'5 emotes · page 1 / 1');
    text(get(library,'status'),'Assignment is in your draft; Apply saves it');
  }
  for (const d of view.options) {
    const row = get(display, 'opt' + d.index);
    if (d.slider) {
      const ratio = (d.value - d.slider[0]) / (d.slider[1] - d.slider[0]);
      const slider = get(row, 'slider'); set(get(slider, 'fill'), 'width', ratio * 100 + '%'); set(get(slider, 'thumb'), 'x', Math.max(9, Math.min(369, ratio * 378)) + 'px');
      text(get(row, 'value'), d.value + (d.key === 'emote_height' ? ' px' : d.key === 'emote_scale' ? '%' : ' s'));
    } else {
      for (let i = 0; i < 2; i++) { const choice = get(row, 'choice' + i); choice.own += ` btn: {back_color: ${d.value === i ? '#eeececff' : '#393939ff'}; color: #6d6c6aff; stroke: 1;} text: {text: ${JSON.stringify(d.choices[i])}; size: 15; color: ${d.value === i ? '#393939ff' : '#eeececff'};}`; }
    }
  }
  const scene = get(display, 'scene'), emote = get(scene, 'emote');
  ['x', 'y', 'width', 'height'].forEach((key, i) => set(emote, key, view.rect[i] + 'px'));
  const champion = get(scene, 'champion'), factor = view.zoom * view.fit;
  set(champion,'visible',view.pan>0?'false':'true');
  set(get(scene,'ground'),'visible',view.pan>0?'false':'true');
  for (const [name, rect] of [['head', [18, 0, 28, 28]], ['body', [8, 32, 48, 44]], ['health', [0, -12, 64, 5]]]) {
    const coords = [view.anchor[0] + (rect[0] - 32) * factor, view.anchor[1] + (rect[1] - 76) * factor, rect[2] * factor, rect[3] * factor];
    ['x', 'y', 'width', 'height'].forEach((key, i) => set(get(champion, name), key, coords[i] + 'px'));
  }
  text(get(display, 'metrics'), `Height ${view.height} px · scale ${view.scale}%${view.pan>0?' · champion below preview':''}`);
  function render(n, parent = '') {
    const id = parent ? parent + '.' + n.name : n.name; let css = 'position:absolute;box-sizing:border-box;', content = '';
    for (const key of ['x', 'y', 'width', 'height', 'z']) { const value = prop(n.own, key); if (value) css += `${({ x: 'left', y: 'top', z: 'z-index' })[key] || key}:${value};`; }
    if (!prop(n.own, 'width')) css += 'width:100%;'; if (!prop(n.own, 'height')) css += 'height:100%;';
    if (prop(n.own, 'visible') === 'false') css += 'display:none;';
    if (prop(n.own, 'pivot_x')) css += `transform:translateX(-${Number(prop(n.own, 'pivot_x')) * 100}%);`;
    const color = value => (value || 'transparent').replace('#~', '#');
    if (n.type === 'color') css += `background:${color(prop(n.own, 'color'))};`;
    if (n.type === 'label' || n.type === 'color_icon_button') {
      const copy = n.type === 'label' ? n.own : block(n.own, 'text'); const raw = prop(copy, 'text'); content = raw ? escapeHtml(JSON.parse(raw)) : '';
      css += `display:flex;align-items:center;font-family:Native,Arial;font-size:${prop(copy, 'size') || 16}px;color:${color(prop(copy, 'color'))};white-space:pre-wrap;`;
      if (prop(copy, 'align_x') === 'Center' || n.type === 'color_icon_button') css += 'justify-content:center;';
      if (prop(copy, 'align_x') === 'Right') css += 'justify-content:flex-end;';
      if (n.type === 'color_icon_button') { const btn = block(n.own, 'btn'); css += `background:${color(prop(btn, 'back_color'))};border:${prop(btn, 'stroke') || 0}px solid ${color(prop(btn, 'color'))};border-radius:4px;`; }
    }
    if (n.type === 'image') { const raw = prop(n.own, 'source'); if (raw) { const name = JSON.parse(raw).split('/').pop(); const file = path.join(root, 'probe/ui', name + '.png'); content = `<img src="data:image/png;base64,${fs.readFileSync(file).toString('base64')}"/>`; } }
    if (prop(n.own, 'visible') === 'false') css += 'display:none;';
    return `<div data-node="${id}" style="${css}">${content}${n.nodes.map(c => render(c, id)).join('')}</div>`;
  }
  const font = fs.readFileSync(path.join(root, 'probe/font/manrope_medium.ttf')).toString('base64');
  return `<meta charset="utf-8"><style>@font-face{font-family:Native;src:url(data:font/ttf;base64,${font})}body{margin:0;background:#202928}img{width:100%;height:100%}</style>${tree.nodes.map(n => render(n)).join('')}`;
}
(async () => {
  const browser = await chromium.launch({ channel: 'msedge', headless: true });
  const page = await browser.newPage({ viewport: { width: 1920, height: 1080 } });
  views.push({...views[0],name:'library',library:true});
  const zoom2=views.find(v=>v.name==='follow-zoom-200'), zoom3=views.find(v=>v.name==='follow-zoom-in');
  if(Math.abs(zoom3.rect[2]/zoom2.rect[2]-1.5)>0.001) throw Error('200% and 300% must have different image sizes');
  for (const view of views) {
    const html = document(view); fs.writeFileSync(path.join(out, view.name + '.html'), html);
    await page.setContent(html); await page.evaluate(() => window.document.fonts.ready);
    const overflow = await page.evaluate(() => {
      const panel = document.querySelector('[data-node="window.emote_panel"]'); const box = panel.getBoundingClientRect();
      return [...panel.querySelectorAll('[data-node]')].filter(n => n.offsetParent !== null).filter(n => { const r = n.getBoundingClientRect(); return r.x < box.x - 2 || r.y < box.y - 2 || r.right > box.right + 2 || r.bottom > box.bottom + 2; }).map(n => n.dataset.node);
    });
    if (overflow.length) throw Error(view.name + ' overflow: ' + overflow);
    await page.screenshot({ path: path.join(out, view.name + '.png') });
    console.log(view.name + ': template bounds and asset paths checked');
  }
  await browser.close();
})().catch(e => { console.error(e); process.exit(1); });
