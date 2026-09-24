/**
 * 桌面版冒烟测试：用 WebView2 的调试端口驱动应用，检查关键链路。
 *
 * 用法（需要 Node 18+，不装任何依赖）：
 *   1. 带调试端口启动应用：
 *        set WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9223
 *        随机编队.exe
 *      （PowerShell 用 $env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=9223"）
 *   2. node app/smoke-test.mjs 9223 [截图输出路径]
 *
 * 也能测纯前端：浏览器打开 web\index.html?demo=1（内置 16 人演示、不联网），
 * 再用无头 Edge 开同样的调试端口指向它。
 */
const [, , portArg, shotPath] = process.argv;
const port = portArg || '9223';

const list = await (await fetch(`http://127.0.0.1:${port}/json/list`)).json();
const page = list.find((t) => t.type === 'page');
if (!page) {
  console.error(`端口 ${port} 上没有页面 —— 应用起了吗？调试端口开了吗？`);
  process.exit(1);
}

const ws = new WebSocket(page.webSocketDebuggerUrl);
let id = 0;
const pending = new Map();
const errors = [];
ws.addEventListener('message', (e) => {
  const m = JSON.parse(e.data);
  if (m.method === 'Runtime.exceptionThrown') {
    errors.push((m.params.exceptionDetails.exception?.description || m.params.exceptionDetails.text || '').split('\n')[0]);
  }
  if (m.id && pending.has(m.id)) { pending.get(m.id)(m); pending.delete(m.id); }
});
await new Promise((r) => ws.addEventListener('open', r));

const send = (method, params = {}) =>
  new Promise((res) => { const i = ++id; pending.set(i, res); ws.send(JSON.stringify({ id: i, method, params })); });
const ev = async (expr) => {
  const r = await send('Runtime.evaluate', { expression: expr, returnByValue: true, awaitPromise: true });
  if (r.result?.exceptionDetails) return `异常: ${(r.result.exceptionDetails.exception?.description || '').split('\n')[0]}`;
  return r.result?.result?.value;
};
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

await send('Page.enable');
await send('Runtime.enable');
await sleep(1200);                       // 等前端把 /api/state 拉完

let failed = 0;
const check = (label, ok, detail = '') => {
  console.log(`${ok ? '✓' : '✗'} ${label}${detail ? '  ' + detail : ''}`);
  if (!ok) failed++;
};

const host = await ev("typeof window.__TAURI__ !== 'undefined' ? 'tauri' : 'browser'");
const badge = await ev("document.querySelector('#box-badge')?.innerText || ''");
const pool = await ev("document.querySelector('#pool-info')?.innerText || ''");

console.log(`宿主：${host}`);
check('干员池已加载', /\d+/.test(badge), badge);
check('职业分布无「未知」', !pool.includes('未知'), pool.slice(0, 60));

const ids = await ev("[...document.querySelectorAll('[id]')].length");
check('页面元素已渲染', ids > 20, `${ids} 个带 id 的元素`);

// 抽一次
await ev("document.querySelector('#roll').click()");
await sleep(4500);
const cards = await ev("document.querySelectorAll('.card').length");
check('抽签出卡', cards >= 1, `${cards} 张`);

const portraits = await ev("(()=>{const p=[...document.querySelectorAll('.portrait')];return JSON.stringify({n:p.length,ok:p.filter(i=>i.naturalWidth>0).length})})()");
const pt = JSON.parse(portraits || '{"n":0,"ok":0}');
if (pt.n > 0) check('头像加载', pt.ok === pt.n, `${pt.ok}/${pt.n}`);
else console.log('· 头像开关关着（没渲染 img），跳过');

const icons = await ev("(()=>{const p=[...document.querySelectorAll('.clsbox img')];return JSON.stringify({n:p.length,ok:p.filter(i=>i.naturalWidth>0).length})})()");
const ic = JSON.parse(icons || '{"n":0,"ok":0}');
check('职业图标加载', ic.n > 0 && ic.ok === ic.n, `${ic.ok}/${ic.n}`);

// 幂等：先全解锁，再锁一张，应当正好 1 张
await ev("document.querySelectorAll('.card.locked button[data-act=lock]').forEach((b) => b.click())");
await ev("document.querySelector('.card button[data-act=lock]')?.click()");
const lockedN = await ev("document.querySelectorAll('.card.locked').length");
check('锁定按钮可用', lockedN === 1, `锁定 ${lockedN} 张`);

check('无 JS 异常', errors.length === 0, errors.slice(0, 2).join(' / '));

if (shotPath) {
  const shot = await send('Page.captureScreenshot', { format: 'png' });
  const fs = await import('node:fs');
  fs.writeFileSync(shotPath, Buffer.from(shot.result.data, 'base64'));
  console.log(`截图：${shotPath}`);
}

ws.close();
console.log(failed === 0 ? '\n全部通过 ✓' : `\n${failed} 项失败 ✗`);
process.exit(failed === 0 ? 0 : 1);
