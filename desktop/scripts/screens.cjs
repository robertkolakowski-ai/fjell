// Skjermbilder og axe-sjekk av galleriet (M6). Bruk: start 'npm run dev' i desktop/, og kjør
// 'node desktop/scripts/screens.cjs <mappe med node_modules/axe-core>'. Krever Playwright (global).
// Ikke koblet til CI ennå: M6-skjermbildene regenereres foreløpig manuelt.
const { chromium } = require('playwright');
const fs = require('fs');
const axeSrc = fs.readFileSync(process.argv[2] + '/node_modules/axe-core/axe.min.js', 'utf8');
const out = '/home/user/fjell-m6/docs/screens/m6';
const URL = 'http://localhost:1420/?gallery';
(async () => {
  const b = await chromium.launch();
  const combos = [];
  for (const loc of ['nb', 'pseudo', 'en']) for (const th of ['light', 'dark']) for (const sc of [100, 200]) combos.push({ loc, th, sc, hc: false });
  combos.push({ loc: 'nb', th: 'light', sc: 100, hc: true }, { loc: 'nb', th: 'dark', sc: 100, hc: true }, { loc: 'pseudo', th: 'light', sc: 200, hc: true });
  const report = [];
  for (const c of combos) {
    const ctx = await b.newContext({ viewport: { width: 1000, height: 800 }, deviceScaleFactor: 2, colorScheme: c.th, forcedColors: c.hc ? 'active' : 'none' });
    const p = await ctx.newPage();
    await p.goto(URL);
    await p.waitForSelector('.tiles');
    await p.selectOption('select >> nth=1', c.loc);
    await p.selectOption('select >> nth=2', String(c.sc));
    await p.selectOption('select >> nth=0', c.hc ? 'auto' : c.th);
    await p.waitForTimeout(300);
    const name = `gallery-${c.loc}-${c.hc ? 'hc-' : ''}${c.th}-${c.sc}.png`;
    await p.screenshot({ path: `${out}/${name}`, fullPage: true });
    const overflow = await p.evaluate(() => document.documentElement.scrollWidth > innerWidth + 1);
    await p.addScriptTag({ content: axeSrc });
    const r = await p.evaluate(async () => (await axe.run(document, { runOnly: ['wcag2a','wcag2aa','wcag21a','wcag21aa','best-practice'] })).violations.map(v => ({ id: v.id, impact: v.impact, n: v.nodes.length, t: v.nodes.slice(0,2).map(n=>n.target.join(' ')+' :: '+(n.any[0]?.message||n.all[0]?.message||'')) })));
    report.push({ name, overflow, violations: r });
    await ctx.close();
  }
  await b.close();
  console.log(JSON.stringify(report, null, 1));
})();
