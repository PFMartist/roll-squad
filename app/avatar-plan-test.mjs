/**
 * 头像策略回归测试：在 Node 里把 web/app.js 真跑一遍（DOM 用桩），验三个宿主分支的取图方式。
 *
 *   分支 1  在线演示 + 有包内清单  → 走 assets/avatars/*.webp，且**不回退**（演示版页面上没有后端）
 *   分支 2  在线演示 + 没清单      → 头像开关锁死，一张图都不画（本地直开 index.html 的情形）
 *   分支 3  浏览器 + 本地服务      → 包内优先，但保留回退 /api/avatar
 *
 * 用法（零依赖）：
 *   node app/avatar-plan-test.mjs
 *
 * 桩很薄：boot() 那条渲染链可能半路抛（例如桩里没有 getComputedStyle），会被忽略并打印一行；
 * 本测试只断言 demoState() / avatarPlan() 的行为，不依赖整条渲染链。
 */
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';
import vm from 'node:vm';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');

// 桩环境缺东西时 boot() 里的链路会抛，不该让它把整轮验证带走：记一行、继续断言要测的那两个函数
process.on('unhandledRejection', (e) => console.log('  （桩环境未捕获拒绝，已忽略）' + String(e).split('\n')[0]));
process.on('uncaughtException', (e) => console.log('  （桩环境未捕获异常，已忽略）' + String(e).split('\n')[0]));
const APP = readFileSync(join(ROOT, 'web', 'app.js'), 'utf8');
const ROSTER = readFileSync(join(ROOT, 'web', 'assets', 'demo_roster.js'), 'utf8');
const MANIFEST = JSON.parse(readFileSync(join(ROOT, 'web', 'assets', 'avatars', 'manifest.json'), 'utf8'));

/** 万能 DOM 桩：任何属性访问都返回自身，任何方法调用都 no-op（够 initTheme/boot 跑完不炸） */
function makeEl() {
  const el = new Proxy(function () {}, {
    get(_t, k) {
      if (k === 'then') return undefined;                    // 别被 await 当成 thenable
      if (k === 'classList') return { add() {}, remove() {}, toggle() {} };
      if (k === 'dataset' || k === 'style') return {};
      if (k === 'value') return '0';                         // #tier 之类被读成索引，给个有效档位
      if (k === 'textContent' || k === 'innerHTML' || k === 'title') return '';
      if (k === 'hidden' || k === 'checked' || k === 'disabled') return false;
      if (k === 'closest' || k === 'querySelector') return () => el;
      if (k === 'querySelectorAll') return () => [];
      if (k === 'getBoundingClientRect') return () => ({ left: 0, top: 0, right: 0, bottom: 0 });
      if (k === Symbol.toPrimitive || k === 'toString') return () => '';
      return () => el;
    },
    set: () => true,
    apply: () => el,
  });
  return el;
}

/** 最小"后端状态"：分支 3 要假装有 Python 服务在跑，否则 boot 会自动退进演示模式 */
const PROFS = ['先锋', '近卫', '重装', '狙击', '术师', '医疗', '辅助', '特种'];
const BACKEND_STATE = {
  demo: false, roster: 2,
  box: { file: 'box_demo.json', path: 'box_demo.json', sync: null, age_days: 1, stale: false },
  box_options: [{ file: 'box_demo.json', name: '演示池', sync: null }],
  avatars_enabled: true, avatars_locked: false,
  tiers: Object.fromEntries([0, 1, 2, 3, 4, 5].map((t) => [t, { n: 2, desc: '档位', prof: {} }])),
  professions: PROFS, floor: Object.fromEntries(PROFS.map((p) => [p, 0])), history: [],
  defaults: { tier: 0, mode: 'quota', n: 12, avoid_last: 1, exclude: '', rarities: [1, 2, 3, 4, 5, 6], quota: {} },
  modes: { floor: '按职业配额', pure: '纯随机', quota: '按职业配额' },
};

async function run({ search, withManifest }) {
  const el = makeEl();
  const sandbox = {
    console,
    setTimeout,
    clearTimeout,
    queueMicrotask,
    URLSearchParams,
    Math,
    JSON,
    Promise,
    location: { search, href: 'http://x/index.html' + search },
    document: {
      querySelector: () => el,
      querySelectorAll: () => [],
      addEventListener() {},
      documentElement: el,
      body: el,
      createElement: () => el,
    },
    window: { addEventListener() {}, DEMO_ROSTER: undefined },
    fetch: async (url) => {
      const u = String(url);
      if (u.endsWith('manifest.json')) {
        return withManifest
          ? { ok: true, status: 200, json: async () => MANIFEST }
          : { ok: false, status: 404, json: async () => null };
      }
      if (u === '/api/state' && search === '') {              // 分支 3：假装本地服务在跑
        return { ok: true, status: 200, json: async () => BACKEND_STATE };
      }
      throw new Error('演示模式下不该有任何请求：' + u);
    },
  };
  sandbox.globalThis = sandbox;
  const ctx = vm.createContext(sandbox);
  vm.runInContext(ROSTER, ctx);                              // 先摆好 window.DEMO_ROSTER
  vm.runInContext(APP, ctx);                                 // app.js 末尾会 initTheme(); boot();
  await new Promise((r) => setTimeout(r, 200));              // 等 boot 的 await 链走完
  return {
    state: ctx.demoState(),
    plan: ctx.avatarPlan({ name: '阿米娅' }),
    planNoAvatar: ctx.avatarPlan({ name: '并不存在的干员' }),
  };
}

let failed = 0;
const check = (label, ok, detail = '') => {
  console.log(`${ok ? '✓' : '✗'} ${label}${detail ? '  ' + detail : ''}`);
  if (!ok) failed++;
};

console.log('—— 分支 1：在线演示（?demo=1）+ 有包内清单 ——');
{
  const r = await run({ search: '?demo=1', withManifest: true });
  check('头像默认开', r.state.avatars_enabled === true);
  check('开关没被锁', r.state.avatars_locked === false);
  check('头像走包内 WebP', r.plan.src === `assets/avatars/${encodeURIComponent('阿米娅')}.webp`, r.plan.src);
  check('演示版不给回退（fb 为空）', r.plan.fb === '');
  check('快照里没有的干员 → 空 src（走色块）', r.planNoAvatar.src === '' && r.planNoAvatar.fb === '');
}

console.log('—— 分支 2：在线演示 + 没有清单（本地直开 index.html）——');
{
  const r = await run({ search: '?demo=1', withManifest: false });
  check('头像关且锁死', r.state.avatars_enabled === false && r.state.avatars_locked === true);
  check('没清单时不画图', r.plan.src === '' && r.plan.fb === '');
}

console.log('—— 分支 3：浏览器 + 本地 Python 服务（非演示）+ 有清单 ——');
{
  const r = await run({ search: '', withManifest: true });
  check('头像走包内 WebP', r.plan.src.endsWith('.webp'), r.plan.src);
  check('带回退到本地服务', r.plan.fb === '/api/avatar?name=' + encodeURIComponent('阿米娅'), r.plan.fb);
}

console.log(failed === 0 ? '\n三个分支全部通过 ✓' : `\n${failed} 项失败 ✗`);
process.exit(failed === 0 ? 0 : 1);
