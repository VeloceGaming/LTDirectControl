/* Viewer logic tests without a browser or writes to the author's data.
 * node tools/test_preview_viewer.cjs [--data snapshot.json] [--baseline old.html]
 * The optional baseline compares every recorded skill's shapes and drawing
 * coordinates before and after a UI change. It is never required for CI.
 */
'use strict';
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const option = flag => {
  const index = process.argv.indexOf(flag);
  return index < 0 ? null : process.argv[index + 1];
};
const html = fs.readFileSync(path.join(__dirname, 'preview_viewer.html'), 'utf8');
const fixture = {
  table: {game: 'test', champions: {tester: {
    Q: {cast: {casting: 1, target: 6, range: 70, growth: 2}, shapes: [{kind: 'circle', at: 'aim', radius: 20}]},
    W: {cast: {casting: 2, target: 6, range: 90}, shapes: [{kind: 'strip', at: 'caster', radius: 5, length: 0}]},
    R: {cast: {casting: 0, target: 6, range: 100}, unread: 'opaque Native'}
  }}},
  style: {ally: '#53b8e4', yellow: '#fdee00', white: '#ffffff', ink: '#000000', range_ally: '#abcabc', range_ready: '#fff6c2', range_width: 2, range_opacity: .9, area_fill: .1, range_fill: .05},
  numbers: {}, names: {tester: {name: 'Test Champion', skill_Q: 'Test Circle'}},
  source: {tester: 'Base game'}, attack_ranges: {tester: 65}, saved: {}, checked: [], picks: {}, sources: {}
};
const data = option('--data') ? JSON.parse(fs.readFileSync(path.resolve(option('--data')), 'utf8')) : fixture;
const json = value => JSON.parse(JSON.stringify(value));
let assertions = 0;
function equal(actual, expected, label) {
  assert.deepEqual(json(actual), json(expected), label);
  assertions++;
}

function runtime(source, input = data, standalone = false) {
  let script = source.match(/<script>([\s\S]*?)<\/script>/)[1].replace(/\nstart\(\);\s*$/, '');
  if (standalone) script = script.replace('/*DATA*/null', '{}');
  const elements = new Map();
  const memory = new Map();
  const calls = [], timers = [], downloads = [], events = new Map();
  const makeElement = () => {
    const classes = new Set();
    return {
      dataset: {}, style: {}, value: '', innerHTML: '', textContent: '', hidden: false,
      children: [], parentNode: null, isConnected: true, focusCount: 0,
      classList: {toggle() {}, add: value => classes.add(value), remove: value => classes.delete(value), contains: value => classes.has(value)},
      setAttribute() {}, focus() { this.focusCount++; }, querySelectorAll() { return []; },
      getBoundingClientRect() { return {left: 240, right: 580, top: 200}; },
      append(...nodes) {
        for (const node of nodes) {
          if (node.parentNode) node.parentNode.children = node.parentNode.children.filter(child => child !== node);
          node.parentNode = this;
          this.children.push(node);
        }
      },
      appendChild(node) { this.append(node); return node; },
      click() { downloads.push(this.download); },
    };
  };
  const document = {
    documentElement: makeElement(),
    getElementById(id) { if (!elements.has(id)) elements.set(id, makeElement()); return elements.get(id); },
    querySelectorAll() { return []; },
    createElement: makeElement,
  };
  const context = vm.createContext({
    document, console, Blob, devicePixelRatio: 1, innerWidth: 1440, innerHeight: 900,
    URL: {createObjectURL(blob) { downloads.push(blob); return 'blob:test'; }},
    localStorage: {getItem: key => memory.get(key) ?? null, setItem: (key, value) => memory.set(key, value), removeItem: key => memory.delete(key)},
    matchMedia: () => ({matches: false}),
    getComputedStyle: () => ({getPropertyValue: () => '#19241e'}),
    addEventListener: (name, handler) => events.set(name, handler),
    setTimeout: handler => { timers.push(handler); return timers.length; }, clearTimeout() {},
    fetch: async (url, options) => {
      const body = JSON.parse(options.body);
      calls.push({url, body});
      return {json: async () => ({ok: true, saved: Object.keys(body.skills || {}).length, sources: {}})};
    },
  });
  vm.runInContext(script, context, {filename: 'preview_viewer.js'});
  const run = code => vm.runInContext(code, context);
  context.input = json(input);
  run('D=input;TABLE=D.table;ST=D.style;NUMBERS=D.numbers||{};NAMES=D.names||{};SOURCE=D.source||{};ATTACK=D.attack_ranges||{};saved=clone(D.saved||{});assigned=clone(saved);checked=new Set(D.checked||[]);SOURCES=D.sources||{};picks=clone(D.picks||{});learn()');
  return {context, run, elements, memory, calls, timers, downloads, events};
}

(async () => {
  const app = runtime(html, fixture);
  app.run('D=undefined');
  await app.run('saveNow()');
  equal(app.calls.length, 0, 'Save cannot run before data loads');
  app.run('D=input');
  equal(app.run('rangeAt(TABLE.champions.tester.Q.cast)'), 70, 'Level 1 range');
  equal(app.run('view.level=18;rangeAt(TABLE.champions.tester.Q.cast)'), 104, 'Level 18 range');
  equal(app.run('Object.keys(TYPES).length'), 11, 'All preview types retained');
  equal(app.run("shapesOf({type:'attack_reach',extra:50})"), [{kind: 'reach', extra: 50, at: 'caster'}], 'Attack-range ring');
  equal(app.run("shapesOf({type:'dash',blink:true,distance:30})"), [{kind: 'blink', at: {end: 30}}], 'Blink');
  equal(app.run("shapesOf({type:'no_preview'})"), [], 'No preview');
  equal(app.run("assigned={'tester Q':{type:'self_circle',radius:8,add:true}};chosen('tester','Q',TABLE.champions.tester.Q).shapes.length"), 2, 'Add keeps both shapes');
  equal(app.run("delete assigned['tester Q'].add;chosen('tester','Q',TABLE.champions.tester.Q).shapes.length"), 1, 'Replace keeps assigned shape');
  equal(app.run("assigned={'tester Q':{type:'self_circle'}};invalidKeys()"), ['tester Q'], 'Missing required size refused');
  equal(app.run("assigned={'tester Q':{type:'self_circle',radius:10001}};invalidKeys()"), ['tester Q'], 'Out-of-range size refused');
  equal(app.run("assigned={'tester W':{type:'line',width:10}};invalidKeys()"), [], 'Empty optional length accepted');
  app.run("assigned={};checked=new Set(['tester Q'])");
  equal(app.run("matches('tester','Q',{q:'test circle',status:'all',type:'',mod:''})"), true, 'Search finds skill names');
  equal(app.run("matches('tester','W',{q:'',status:'checked',type:'',mod:''})"), false, 'Checked filter');
  equal(app.run("matches('tester','Q',{q:'',status:'all',type:'line',mod:''})"), false, 'Shape filter');
  equal(app.run("matches('tester','Q',{q:'',status:'all',type:'',mod:'Another mod'})"), false, 'Source filter');
  app.run("setSpan(1)"); equal(app.run('view.span'), 60, 'Minimum zoom');
  app.run("setSpan(9999)"); equal(app.run('view.span'), 1200, 'Maximum zoom');
  app.run("assigned={'offscreen Q':{type:'self_circle',radius:10},'tester W':{type:'line',width:5},'tester R':{type:'cone'}}");
  await app.run('saveNow()');
  equal(app.calls[0], {url: '/api/save', body: {skills: {'offscreen Q': {type: 'self_circle', radius: 10}, 'tester W': {type: 'line', width: 5}}}}, 'Save preserves offscreen champions and excludes invalid assignments');
  equal(app.run('dirty()'), true, 'Invalid unsaved work remains dirty');
  equal(app.elements.get('saveState').dataset.state, 'invalid', 'Invalid state remains visible');
  let prevented = false;
  app.events.get('beforeunload')({preventDefault() { prevented = true; }});
  equal(prevented, true, 'Leaving with unfinished work warns');
  app.run('saveChecked();savePicks()');
  for (const timer of app.timers) await timer();
  equal(app.calls.some(call => call.url === '/api/checked'), true, 'Checked marks use original endpoint');
  equal(app.calls.some(call => call.url === '/api/picks'), true, 'Source picks use original endpoint');
  equal(app.run('store.get("checked",[])'), ['tester Q'], 'Checked marks retained locally');
  app.run("store.set('draft',assigned)");
  equal(app.run('store.get("draft",null)'), app.run('assigned'), 'Recovery draft retains all assignments');

  const standalone = runtime(html, fixture, true);
  standalone.run("assigned={'tester Q':{type:'aim_circle',radius:12}};");
  await standalone.run('saveNow()');
  equal(standalone.calls.length, 0, 'Standalone export makes no server save');
  equal(standalone.downloads[1], 'skill-overrides.json', 'Original download filename');
  equal(JSON.parse(await standalone.downloads[0].text()).skills, {'tester Q': {type: 'aim_circle', radius: 12}}, 'Standalone export format');

  const failure = runtime(html, fixture);
  failure.run("assigned={'tester Q':{type:'aim_circle',radius:12}}");
  failure.context.fetch = async () => ({json: async () => ({ok: false, problems: ['Test save failure']})});
  await failure.run('saveNow()');
  equal(failure.elements.get('saveState').dataset.state, 'error', 'Failed save remains visible on small screens');
  equal(failure.elements.get('saveState').innerHTML.includes('Test save failure'), true, 'Failure reason is displayed');
  equal(failure.run('dirty()'), true, 'Failed save does not mark edits saved');

  // Reference panels must leave quick-edit controls on their original cards,
  // retaining unsaved values while switching skills and closing the panel.
  const inspection = runtime(html, fixture);
  const makeCard = slot => {
    const document = inspection.context.document;
    const card = {champ: 'tester', slot, s: fixture.table.champions.tester[slot]};
    for (const key of ['div', 'storage', 'editor', 'sizes', 'numbers', 'details']) card[key] = document.createElement('div');
    card.div.append(card.editor, card.sizes, card.storage);
    card.storage.append(card.numbers, card.details);
    return card;
  };
  const cardQ = makeCard('Q'), cardW = makeCard('W');
  const originButton = inspection.context.document.createElement('button');
  cardQ.editor.value = '37';
  inspection.context.cardQ = cardQ; inspection.context.cardW = cardW; inspection.context.originButton = originButton;
  inspection.run("openInspector(cardQ,'numbers',originButton)");
  equal(inspection.elements.get('inspector').hidden, false, 'Numbers inspector opens');
  equal(cardQ.numbers.parentNode === inspection.elements.get('inspectorPanel-numbers'), true, 'Numbers belong to the selected skill');
  equal(cardQ.editor.parentNode === cardQ.div, true, 'Opening numbers leaves quick editing on its card');
  equal(cardQ.sizes.parentNode === cardQ.div, true, 'Opening numbers leaves size sources on their card');
  inspection.run("inspectorTab('details')");
  equal(cardQ.editor.value, '37', 'Switching tabs preserves unfinished editor values');
  equal(inspection.elements.get('inspectorPanel-details').hidden, false, 'Details tab exposes the selected skill description');
  inspection.run("openInspector(cardW,'numbers',originButton)");
  equal(cardQ.storage.children.length, 2, 'Switching skills returns old references to their owner');
  equal(cardW.numbers.parentNode === inspection.elements.get('inspectorPanel-numbers'), true, 'Numbers belong to the newly selected skill');
  inspection.run('closeInspector()');
  equal(cardW.storage.children.length, 2, 'Closing returns all references without losing their identity');
  equal(cardQ.editor.parentNode === cardQ.div && cardW.editor.parentNode === cardW.div, true, 'Both quick editors remain in place after switching and closing');
  equal(originButton.focusCount, 1, 'Closing restores keyboard focus');
  equal(inspection.calls.length, 0, 'Inspecting numbers never writes assignments or checks');

  if (option('--baseline')) {
    const baselineHtml = fs.readFileSync(path.resolve(option('--baseline')), 'utf8');
    const ids = source => [...source.split('<script>')[0].matchAll(/\bid="([^"]+)"/g)].map(match => match[1]);
    equal(new Set(ids(html)).size, ids(html).length, 'No duplicate control IDs');
    equal(ids(baselineHtml).filter(id => !ids(html).includes(id)), [], 'Every original named control retained');
    const before = runtime(baselineHtml);
    const after = runtime(html);
    // Every assignment grammar, including flags and add/replace, on every cast kind.
    const assignments = [null, {type: 'unit_target', area_radius: 18, allies: true}, {type: 'self_circle', radius: 20}, {type: 'aim_circle', radius: 20}, {type: 'aim_rectangle', width: 25, depth: 35}, {type: 'line', width: 10}, {type: 'fan', count: 5, width: 8, spread: 70}, {type: 'cone', radius: 40, angle: 60}, {type: 'dash', distance: 35}, {type: 'dash', blink: true}, {type: 'jump_back', distance: 30, add: true}, {type: 'attack_reach', extra: 50}, {type: 'no_preview'}];
    for (const runtime of [before, after]) {
      runtime.context.assignments = assignments;
      runtime.run(`comparison=[];for(const champ of Object.keys(TABLE.champions))for(const slot of SLOTS){const key=champ+' '+slot,s=TABLE.champions[champ][slot],old=assigned[key];for(const a of assignments){if(a)assigned[key]=a;else if(old)assigned[key]=old;else delete assigned[key];comparison.push([champ,slot,chosen(champ,slot,s),suggest(champ,slot,s),nums(champ,slot)])}if(old)assigned[key]=old;else delete assigned[key]}`);
    }
    equal(after.run('comparison'), before.run('comparison'), 'All skills, all assignments, suggestions, and named sizes match baseline');
    let drawings = 0;
    for (const runtime of [before, after]) {
      const ops = [];
      const graphics = new Proxy({measureText: text => ({width: text.length * 6})}, {get(target, key) { return key in target ? target[key] : (...args) => { if (key !== 'fillRect' && key !== 'fillText') ops.push([key, ...args]); }; }, set(target, key, value) {target[key] = value; return true;}});
      runtime.context.canvas = {clientWidth: 320, getContext: () => graphics};
      runtime.context.ops = ops;
      runtime.run(`geometry=[];for(const level of [1,9,18])for(const span of [60,260,1200])for(const pointer of [null,[40,270]]){view.level=level;view.span=span;for(const champ of Object.keys(TABLE.champions))for(const slot of SLOTS){ops.length=0;const s=TABLE.champions[champ][slot];draw(canvas,s,chosen(champ,slot,s),pointer,champ);geometry.push(JSON.stringify(ops))}}`);
      drawings = runtime.run('geometry.length');
    }
    equal(after.run('geometry'), before.run('geometry'), `${drawings} drawings preserve shape coordinates across levels, zoom, and aiming`);
    console.log(`Baseline comparison: ${after.run('comparison.length')} skill/assignment cases; ${drawings} drawings.`);
  }
  console.log(`Viewer checks: ${assertions} passed.`);
})().catch(error => {console.error(error); process.exitCode = 1;});
