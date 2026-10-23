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
  equal(app.run('Object.keys(TYPES).length'), 12, 'All preview types retained, plus Custom (pieces)');
  // Custom previews built from pieces (1.5.2-dev2).
  app.run("NUMBERS={tester:{W:{width:60000,rush:4200,ticks:20}}};USES={tester:{W:{speed:[7,4200]}}}");
  equal(app.run("sourceValue({name:'width',recorded:1},'tester','W',TABLE.champions.tester.W.cast)"), 60, 'A game number is a distance in world units');
  equal(app.run("sourceValue({name:'rush',by:'ticks',recorded:1},'tester','W',TABLE.champions.tester.W.cast)"), 84, 'Two numbers multiplied: a charge of 4.2 x 20');
  equal(app.run("sourceValue({name:'gone',recorded:12},'tester','W',TABLE.champions.tester.W.cast)"), 12, 'An unknown number falls back to its recorded value');
  equal(app.run("view.level=1;sourceValue({cast_range:true},'tester','W',TABLE.champions.tester.W.cast)"), 90, 'Cast range source');
  app.run(`assigned={'tester W':{type:'custom',pieces:[
    {kind:'shot',radius:{name:'width',recorded:60},length:{fixed:30},pierce:true,at:'caster'},
    {kind:'strip',radius:{fixed:10},length:{name:'rush',by:'ticks',recorded:84},at:'caster'},
    {kind:'dash',at:{end:{name:'rush',by:'ticks',recorded:84}}}]}}`);
  const pick = app.run("chosen('tester','W',TABLE.champions.tester.W)");
  equal([pick.type, pick.exact, pick.shapes.length], ['custom', true, 3], 'A custom preview is chosen and drawn as listed');
  equal(pick.shapes[2], {kind: 'dash', at: {end: 84}}, 'A dash of a fixed length from two numbers');
  equal(app.run('invalidKeys()'), [], 'A complete custom preview can be saved');
  const ops = [];
  const graphics = new Proxy({measureText: text => ({width: text.length * 6})}, {get(target, key) { return key in target ? target[key] : (...args) => ops.push([key, ...args]); }, set(target, key, value) { ops.push(['set', key, value]); target[key] = value; return true; }});
  app.context.canvas = {clientWidth: 320, getContext: () => graphics};
  app.run("draw(canvas,TABLE.champions.tester.W,chosen('tester','W',TABLE.champions.tester.W),null,'tester')");
  const scale = 320 / app.run('view.span');
  const width = w => ops.some(op => op[0] === 'set' && op[1] === 'lineWidth' && Math.abs(op[2] - w * scale) < 1e-6);
  equal([width(10), width(13), width(2.5), width(5.5), width(1.5)], [true, true, true, true, true], 'The dash beside an area is the bullet: body, nose steps and edges at their world widths');
  equal(ops.some(op => op[0] === 'set' && op[1] === 'strokeStyle' && op[2] === 'rgba(255,255,255,0.95)'), true, 'The bullet ends in a white point');
  equal(ops.some(op => op[0] === 'fillText' && String(op[1]).startsWith('dash 84')), true, 'The dash is labelled with its length');
  app.run("assigned['tester W'].pieces[0].radius={fixed:NaN}");
  equal(app.run("customProblems('tester W',assigned['tester W'])"), ['piece 1: radius is not a size'], 'An unfinished size is named');
  equal(app.run('invalidKeys()'), ['tester W'], 'An unfinished custom preview is not saved');
  app.run("assigned['tester W'].pieces=Array.from({length:9},()=>newPiece('circle'))");
  equal(app.run("customProblems('tester W',assigned['tester W'])"), ['at most 8 pieces'], 'At most eight pieces');
  app.run("assigned['tester W']={type:'custom',pieces:[{kind:'shot',radius:{name:'width'},length:{cast_range:true},at:'caster'}]};stampPieces(assigned['tester W'],'tester','W',TABLE.champions.tester.W.cast)");
  equal(app.run("assigned['tester W'].pieces[0]"), {kind: 'shot', radius: {name: 'width', recorded: 60}, length: {cast_range: true, recorded: 90}, at: 'caster'}, 'Recorded values follow the numbers for the mod to fall back on');
  equal(app.run("newPiece('cone').cosine"), {fixed: 0.866}, 'A new cone opens 60 degrees');
  equal(app.run("newPiece('circle',{at:{end:{fixed:40}},allies:true})"), {kind: 'circle', at: {end: {fixed: 40}}, radius: {fixed: 20}, allies: true}, 'A changed block keeps where it starts and its side');
  equal(app.run("newPiece('shot',{kind:'strip',radius:{name:'w',recorded:11},length:{fixed:0},at:'caster',projectile:true})"), {kind: 'shot', at: 'caster', radius: {name: 'w', recorded: 11}, length: {fixed: 0}, pierce: true, projectile: true}, 'Line to Shot keeps its width and length with their sources, and its flat ends');
  equal(app.run("[newPiece('strip').rounded,newPiece('shot').rounded,newPiece('shot',{kind:'circle',radius:{fixed:5},at:'aim'}).rounded,newPiece('spread').rounded]"), [true, true, true, true], 'A new line, shot or fan has rounded ends by default');
  equal(app.run("describe({kind:'spread',count:10,radius:8,length:120,degrees:45,rounded:true},120)"), 'fan: 10 lines, 16 wide, 120 long, 45° spread, rounded tips', 'Rounded fan tips in words');
  equal(app.run("newPiece('circle',{kind:'strip',radius:{fixed:6},length:{fixed:90},at:'caster'})"), {kind: 'circle', at: 'caster', radius: {fixed: 6}}, 'Only the sizes the new block has are kept');
  app.run('assigned={}');
  equal(app.run("toPieces('tester','W',TABLE.champions.tester.W)"), [{kind: 'strip', radius: {fixed: 5}, length: {fixed: 0}, at: 'caster'}], 'Custom starts as a copy of what the card draws');
  app.run("SOURCES={tester:{W:{sizes:[{key:'0.radius',value:5,source:{name:'width',times:0.5,recorded:5}},{key:'0.length',value:7,source:{name:'other',recorded:7}}]}}}");
  equal(app.run("toPieces('tester','W',TABLE.champions.tester.W)[0]"), {kind: 'strip', radius: {name: 'width', times: 0.5, recorded: 5}, length: {fixed: 0}, at: 'caster'}, 'A copied size keeps its source only where the source gives the same value');
  app.run("SOURCES={};NUMBERS={};USES={};assigned={}");
  // Two parts of a skill (2026-10-22): the cast, from the game, and what it hits.
  equal(app.run("castSummary(TABLE.champions.tester.R.cast)"), 'Cast on a unit · on enemies · range 100', 'The cast in words, from the game');
  equal(app.run("castSummary({casting:2,target:2,range:90,growth:5})"), 'Aimed in a direction · on allies · range 90 (+5 per level)', 'An ally skill aimed in a direction');
  equal(app.run("castSummary({casting:3,target:13,range:0})"), 'No aim', 'A skill without aiming');
  // Old preview types become the pieces that draw the same.
  const legacy = (slot, a) => app.run(`legacyPieces('tester','${slot}',TABLE.champions.tester.${slot},${JSON.stringify(a)})`);
  equal(legacy('Q', {type: 'line', width: 10}), [{kind: 'strip', radius: {fixed: 5}, length: {fixed: 0}, at: 'caster'}], 'A line: half its width each side, the cast range long');
  equal(legacy('Q', {type: 'aim_circle', radius: 12, allies: true}), [{kind: 'circle', radius: {fixed: 12}, at: 'aim', allies: true}], 'A circle at the aim for allies');
  equal(legacy('Q', {type: 'dash', distance: 30}), [{kind: 'dash', at: {end: {fixed: 30}}}], 'A dash of a fixed length on a point cast');
  equal(legacy('R', {type: 'dash'}), [], 'A dash on a unit cast stays hidden, as before');
  equal(legacy('Q', {type: 'jump_back', distance: 30, add: true}), [{kind: 'circle', radius: {fixed: 20}, at: 'aim'}, {kind: 'backstep', length: {fixed: 30}, at: 'caster'}], 'Added to what the game reads: both');
  equal(legacy('Q', {type: 'no_preview'}), null, 'No preview stays as it is');
  app.run("SOURCES={tester:{W:{sizes:[{key:'assigned.width',value:10,source:{name:'w',times:2,recorded:10}}]}}}");
  equal(legacy('W', {type: 'line', width: 10}), [{kind: 'strip', radius: {name: 'w', recorded: 5}, length: {fixed: 0}, at: 'caster'}], 'A named width keeps following its number, halved');
  app.run("SOURCES={};assigned={'tester Q':{type:'self_circle',radius:8},'gone Q':{type:'aim_circle',radius:5},'tester R':{type:'no_preview'}}");
  equal(app.run('convertLegacy()'), [1, 1], 'Loaded champions convert; others are kept as saved');
  equal(app.run("assigned['tester Q']"), {type: 'custom', pieces: [{kind: 'circle', radius: {fixed: 8}, at: 'caster'}]}, 'Converted in memory');
  equal(app.run("[assigned['gone Q'].type,assigned['tester R'].type]"), ['aim_circle', 'no_preview'], 'Kept as saved');
  // The game's recorded answer is used for a name it holds twice.
  app.run("NUMBERS={tester:{W:{speed:7,tick:1}}};USES={tester:{W:{speed:[7,4200]}}};TABLE.numbers={tester:{W:{values:{speed:4200}}}}");
  equal(app.run("rawNumbers('tester','W')"), {speed: 4200, tick: 1}, 'Recorded answers come before the file');
  equal(app.run("sourceValue({name:'speed',by:'tick',recorded:0},'tester','W',TABLE.champions.tester.W.cast)"), 4.2, 'A repeated name sized by what the game answers');
  // The ring of a skill aimed in a direction shows how far it reaches.
  // Rounded ends: one capsule, and the ring reaches past the line's end.
  app.run("delete TABLE.numbers;NUMBERS={};USES={};assigned={'tester W':{type:'custom',pieces:[{kind:'strip',radius:{fixed:35},length:{fixed:35},rounded:true,at:'caster'}]}}");
  ops.length = 0;
  app.run("draw(canvas,TABLE.champions.tester.W,chosen('tester','W',TABLE.champions.tester.W),null,'tester')");
  equal(ops.filter(op => op[0] === 'arc' && Math.abs(op[3] - 35 * scale) < 1e-6).length, 1, 'A rounded line is an Aurum lane: one filled half circle past its end, none behind');
  equal(ops.some(op => op[0] === 'set' && op[1] === 'lineWidth' && Math.abs(op[2] - 2.4) < 1e-6), true, 'Its bright core edge');
  equal(ops.some(op => op[0] === 'arc' && Math.abs(op[3] - 90 * scale) < 1e-6), true, 'Its ring stays at the cast range when 35 + 35 is shorter');
  equal(app.run("describe({kind:'strip',radius:35,length:35,rounded:true},90)"), 'line 70 wide, 35 long, rounded ends', 'Rounded ends in words');
  equal(app.run("newPiece('shot',{kind:'strip',radius:{fixed:35},length:{fixed:35},rounded:true,at:'caster'}).rounded"), true, 'Line to Shot keeps rounded ends');
  equal(app.run("'rounded' in newPiece('circle',{kind:'strip',radius:{fixed:35},length:{fixed:35},rounded:true,at:'caster'})"), false, 'A circle has no rounded ends');
  app.run("assigned={'tester W':{type:'custom',pieces:[{kind:'strip',radius:{fixed:30},length:{fixed:80},rounded:true,at:'caster'}]}}");
  ops.length = 0;
  app.run("draw(canvas,TABLE.champions.tester.W,chosen('tester','W',TABLE.champions.tester.W),null,'tester')");
  equal(ops.some(op => op[0] === 'arc' && Math.abs(op[3] - 110 * scale) < 1e-6), true, 'A rounded line reaches 80 + 30: the ring follows');
  app.run("delete TABLE.numbers;NUMBERS={};USES={};assigned={'tester W':{type:'custom',pieces:[{kind:'strip',radius:{fixed:5},length:{fixed:150},at:'caster'}]}}");
  ops.length = 0;
  app.run("draw(canvas,TABLE.champions.tester.W,chosen('tester','W',TABLE.champions.tester.W),null,'tester')");
  equal(ops.some(op => op[0] === 'arc' && Math.abs(op[3] - 150 * scale) < 1e-6), true, 'The ring reaches the 150 line, past the cast range of 90');
  equal(ops.some(op => op[0] === 'fillText' && String(op[1]).startsWith('reach 150 (cast range 90)')), true, 'The label says reach and cast range');
  app.run("assigned={}");
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
  equal(app.run("matches('tester','Q',{q:'',status:'all',hits:'line',mod:''})"), false, 'Hits filter: Q hits a circle, not a line');
  equal(app.run("matches('tester','Q',{q:'',status:'all',hits:'circle',mod:''})"), true, 'Hits filter: Q hits a circle');
  equal(app.run("matches('tester','W',{q:'',status:'all',cast:'direction',mod:''})"), true, 'Cast filter: W is aimed in a direction');
  equal(app.run("matches('tester','W',{q:'',status:'all',cast:'unit',mod:''})"), false, 'Cast filter: W is not cast on a unit');
  equal(app.run("[castKind(TABLE.champions.tester.R.cast),castKind({casting:9}),hitKinds(chosen('tester','R',TABLE.champions.tester.R),TABLE.champions.tester.R.cast)]"), ['unit', 'passive', ['nothing']], 'Cast kinds; an unreadable unit-cast skill hits the target only');
  equal(app.run("hitsSummary(chosen('tester','Q',TABLE.champions.tester.Q),TABLE.champions.tester.Q.cast)"), {text: 'circle, radius 20, at the aim', from: 'what the game reads'}, 'What it hits, in words, and where from');
  equal(app.run("assigned={'tester W':{type:'custom',pieces:[{kind:'strip',radius:{fixed:5},length:{fixed:0},at:'caster'},{kind:'dash',at:'aim'}]}};[hitKinds(chosen('tester','W',TABLE.champions.tester.W),TABLE.champions.tester.W.cast),hitsSummary(chosen('tester','W',TABLE.champions.tester.W),TABLE.champions.tester.W.cast).from]"), [['line', 'dash'], 'your pieces'], 'A skill with two kinds of piece counts under both');
  equal(app.run("assigned={'tester R':{type:'no_preview'}};hitsSummary(chosen('tester','R',TABLE.champions.tester.R),TABLE.champions.tester.R.cast)"), {text: 'Nothing', from: 'your choice: nothing'}, 'Nothing, by choice');
  app.run("assigned={}");
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
