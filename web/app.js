'use strict';

/* ---------------------------------------------------------------- 演示模式
   演示数据是**全游戏可获取干员**（429 名，含星级/职业/合成练度），烤在 assets/demo_roster.js 里，
   由 work/20260925-roll-squad-repo/make_demo_roster.py 生成。
   演示模式**不带干员立绘**：卡片走职业色块 + 名字首字，头像开关固定关闭（见 demoState 的 avatars_locked）。

   什么时候进演示：
     · 地址带 ?demo=1（**双击 index.html?demo=1 也能看**，不需要 Python、不需要 box 数据）
     · 没有后端又没写 ?demo=1 —— GitHub Pages 在线试用、或直接双击 index.html：
       boot() 里探测失败会自动切过来，免得只看到一句报错。 */
let DEMO = new URLSearchParams(location.search).has('demo');
let autoDemo = false;       // 是不是"连不上后端才退到演示"的（界面上要说清楚）
// 三种宿主共用这一份前端：?demo=1 演示 / Tauri 桌面版 / 浏览器 + Python 服务
const TAURI = typeof window !== 'undefined' && !!window.__TAURI__;
const ASSET = 'assets';     // 一律相对路径：file:// 下也能用

const PROF_COLOR = {
  先锋: '#7d3a34', 近卫: '#2f4a72', 重装: '#6b4a1f', 狙击: '#2f5f4c',
  术师: '#4a3a72', 医疗: '#2b5560', 辅助: '#6b3350', 特种: '#454c56',
};

/** 职业图标（PRTS 来的游戏原素材）：默认白色图标，'_白' 是黑色变体（配白方块用） */
const PROF_ICON = (p, v = '') => `${ASSET}/图标_职业_${p}_大图${v}.png`;
const ELITE_ICON = (e) => `${ASSET}/图标_升级_精英化${e}.png`;
let railFilter = null;      // 右侧竖栏选中哪个职业（= 只高亮这个职业的卡）

/** 演示干员池：assets/demo_roster.js 里的紧凑数组 → 统一形状（字段名跟后端返回的一致） */
const DEMO_ROSTER = (typeof window !== 'undefined' && window.DEMO_ROSTER) || { profs: [], ops: [] };
const DEMO_PROFS = DEMO_ROSTER.profs.length
  ? DEMO_ROSTER.profs
  : ['先锋', '近卫', '重装', '狙击', '术师', '医疗', '辅助', '特种'];
const DEMO_OPS = (DEMO_ROSTER.ops || []).map(([name, rarity, elite, level, prof, potential]) => ({
  id: name, name, rarity, elite, level, potential, profession: DEMO_PROFS[prof],
}));

/** 演示干员库对应的游戏版本（由 app/make_demo_roster.py 写进 demo_roster.js）——
    页脚、BOX 下拉、徽章提示、演示横幅都用它，别在别处硬编码版本号。 */
const DEMO_VER = DEMO_ROSTER.version
  ? `${DEMO_ROSTER.version}${DEMO_ROSTER.versionName ? `「${DEMO_ROSTER.versionName}」` : ''}`
  : '';
/** 演示数据的两个前提，界面上要说清楚：对应哪个游戏版本、练度是合成值 */
const DEMO_VER_SHORT = DEMO_VER ? `演示数据 ${DEMO_ROSTER.version} 版 · 练度为随机演示值` : '';
const DEMO_VER_LONG = DEMO_VER
  ? `演示干员库对应 ${DEMO_VER}：该批次新增 ${(DEMO_ROSTER.newest || []).join('、')}；` +
    `解包主表快照抓取于 ${DEMO_ROSTER.snapshot}。` +
    '精英化与等级是随机分配的演示值（不是任何人的真实练度），' +
    '这样六个档位的人数才互不相同、能看出筛选差别。'
  : '';

/* 档位判定 —— 与 Rust 侧 roster.rs 的 tier_pass 同一套规则，演示模式也得跟着走。
   其中 2 = 精一满级、4 = 模组线，这两条线**按星级变化**，数值取自官方数据：
   E1_CAP = 主表 phases[1].maxLevel；MODULE_GATE = uniequip_table.json 的解锁条件。 */
const E1_CAP = { 3: 55, 4: 60, 5: 70, 6: 80 };
const MODULE_GATE = { 4: 40, 5: 50, 6: 60 };
const TIER_MAX = 5;
const TIER_DESC = {
  0: '全部持有', 1: '精一及以上', 2: '精一满级（按星级）',
  3: '精二', 4: '模组线（按星级）', 5: '精二且 Lv≥80',
};

function tierPass(op, tier) {
  const line = (v) => (v === undefined ? Infinity : v);   // 该星级没有这条线 → 永远过不去
  // 「精一满级」：精英化会重置等级（有人 E2 却不到 E1 上限），但升精二的前提就是精一满级 → E2 一律算过
  if (tier === 2) return op.elite >= 2 || (op.elite >= 1 && op.level >= line(E1_CAP[op.rarity]));
  if (tier === 4) return op.elite >= 2 && op.level >= line(MODULE_GATE[op.rarity]);
  const [e, l] = { 1: [1, 0], 3: [2, 0], 5: [2, 80] }[tier] || [0, 0];
  return op.elite >= e && op.level >= l;
}

function demoState() {
  return {
    demo: true, roster: DEMO_OPS.length,
    box: {
      file: `演示数据（全游戏干员${DEMO_ROSTER.version ? ` · ${DEMO_ROSTER.version}` : ''}）`,
      path: '', sync: null, age_days: null, stale: false,
    },
    box_options: [],
    // 演示/在线模式不带干员立绘：头像固定关闭，开关也锁住（见 syncControls）
    avatars_enabled: false, avatars_locked: true,
    tiers: Object.fromEntries(Array.from({ length: TIER_MAX + 1 }, (_, t) => {
      const pool = DEMO_OPS.filter((o) => tierPass(o, t));
      const prof = {};
      pool.forEach((o) => { prof[o.profession] = (prof[o.profession] || 0) + 1; });
      return [t, { n: pool.length, desc: TIER_DESC[t], prof }];
    })),
    professions: DEMO_PROFS, floor: { 先锋: 1, 医疗: 1, 重装: 1 }, history: [],
    defaults: { tier: 0, mode: 'quota', n: 12, avoid_last: 1, exclude: '', rarities: [1, 2, 3, 4, 5, 6], quota: { 先锋: 1, 医疗: 1, 重装: 1 } },
    modes: { floor: '保底队形', pure: '纯随机', quota: '精确配额' },
  };
}

function demoApi(path, body) {
  if (path === '/api/state' || path === '/api/settings') return demoState();
  if (path !== '/api/roll') throw new Error(`演示模式没有这个接口：${path}`);
  if (!DEMO_OPS.length) throw new Error('演示干员池没加载（assets/demo_roster.js 缺失或被挡了）');
  const n = body.n || 12;
  const byId = Object.fromEntries(DEMO_OPS.map((o) => [o.id, o]));
  const rar = (body.rarities && body.rarities.length) ? body.rarities : [1, 2, 3, 4, 5, 6];
  const tier = body.tier || 0;
  const slots = (body.slots && body.slots.length === n) ? body.slots.slice() : new Array(n).fill(null);
  const used = new Set(slots.filter((id) => id && byId[id]));
  const pool = DEMO_OPS.filter((o) => rar.includes(o.rarity) && tierPass(o, tier) && !used.has(o.id));
  const squad = slots.map((id) => {
    if (id && byId[id]) return byId[id];
    return pool.splice(Math.floor(Math.random() * pool.length), 1)[0];
  }).filter(Boolean);
  return {
    squad, seed: Math.floor(Math.random() * 1e9), mode: body.mode || 'floor', tier, n,
    quota: body.quota || null,
    pool: {
      roster: DEMO_OPS.length,
      after_tier: DEMO_OPS.filter((o) => tierPass(o, tier)).length,
      after_filter: pool.length + squad.length,
    },
    time: new Date().toISOString().slice(0, 19),
    warnings: ['演示模式：内置假数据，不连后端、不写历史'], history_file: null, session: null,
  };
}

const $ = (s) => document.querySelector(s);
const esc = (s) => String(s ?? '').replace(/[&<>"]/g,
  (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' }[c]));

let ST = null;          // /api/state 的快照
let squad = [];         // 当前这队（槽位顺序）
let locked = new Set(); // 被锁住的槽位下标
let busy = false;

async function api(path, body) {
  if (DEMO) return demoApi(path, body || {});      // 演示模式：不发任何请求
  if (TAURI) {                                     // 桌面版：走 Tauri 命令，不经过 HTTP
    const cmd = { '/api/state': 'get_state', '/api/roll': 'roll', '/api/settings': 'save_settings' }[path];
    if (!cmd) throw new Error(`桌面版没有这个接口：${path}`);
    try {
      // Rust 侧返回 Err(String) 时，invoke 的 reject 拿到的是**字符串**而不是 Error，
      // 这里统一包成 Error，保持与浏览器版 fetch 抛错的行为一致。
      return await window.__TAURI__.core.invoke(cmd, body ? { body } : {});
    } catch (e) {
      throw new Error(typeof e === 'string' ? e : (e && e.message) || String(e));
    }
  }
  const opt = body
    ? { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) }
    : {};
  const r = await fetch(path, opt);
  const data = await r.json().catch(() => ({}));
  if (!r.ok) throw new Error(data.error || `HTTP ${r.status}`);
  return data;
}

/** 头像地址：演示模式没有立绘（返回空串，卡片露出职业色块），桌面版走自定义协议，浏览器走本地服务 */
function avatarSrc(op) {
  if (DEMO) return op.img || '';
  if (TAURI) return window.__TAURI__.core.convertFileSrc(op.name, 'avatar');
  return `/api/avatar?name=${encodeURIComponent(op.name)}`;
}

// ---------------------------------------------------------------- 控件

function readQuota() {
  const q = {};
  document.querySelectorAll('#quota-inputs input').forEach((inp) => {
    const v = parseInt(inp.value, 10) || 0;
    if (v > 0) q[inp.dataset.prof] = v;
  });
  return q;
}

function fillQuota(q) {
  document.querySelectorAll('#quota-inputs input').forEach((inp) => {
    inp.value = (q && q[inp.dataset.prof]) || 0;
  });
}

function syncStarChips() {
  document.querySelectorAll('#stars label').forEach((l) => {
    l.classList.toggle('off', !l.querySelector('input').checked);
  });
}

function readRarities() {
  const on = [...document.querySelectorAll('#stars input:checked')].map((i) => Number(i.dataset.rarity));
  return on.length === 6 ? null : on;     // 全勾 = 不限制，就别发这个参数了
}

function renderBadge() {
  const b = ST.box;
  const el = $('#box-badge');
  const age = b.age_days === null ? '同步时间未知' : `${b.age_days} 天前同步`;
  el.textContent = DEMO ? `${ST.roster} 名干员 · 离线演示` : `${ST.roster} 名干员 · ${age}`;
  el.classList.toggle('stale', !!b.stale);
  if (b.stale) el.textContent += ' · 请重新同步';
  if (DEMO && DEMO_VER_LONG) el.title = DEMO_VER_LONG;      // 悬停看演示池对应的游戏版本
}

function renderBoxPick() {
  const sel = $('#box');
  const opts = (ST.box_options || []).slice();
  if (!opts.some((o) => o.file === ST.box.path)) {
    opts.unshift({ file: ST.box.path, name: ST.box.file, sync: ST.box.sync });
  }
  sel.innerHTML = opts.map((o) =>
    `<option value="${esc(o.file)}"${o.file === ST.box.path ? ' selected' : ''}>` +
    `${esc(o.name)}${o.sync ? ` · ${o.sync}` : ''}${o.size_kb ? '' : '（当前）'}</option>`).join('');
}

function updatePoolInfo() {
  const t = $('#tier').value;
  const info = ST.tiers[t];
  const parts = ST.professions.filter((p) => info.prof[p]).map((p) => `${p} ${info.prof[p]}`);
  $('#pool-info').textContent = `当前档位池子 ${info.n} 人（${parts.join(' · ')}）`;
}

function renderHistory() {
  const ol = $('#history');
  if (!ST.history.length) {
    ol.innerHTML = `<li class="history-empty">${uiIcon('history')}<strong>暂无行动记录</strong><p>${DEMO ? '演示模式不写入历史；正式模式由原后端保存。' : '完成一次编队后，记录会保存在这里。'}</p></li>`;
    return;
  }
  ol.innerHTML = ST.history.map((h, i) => `
    <li data-i="${i}" role="button" tabindex="0" aria-label="重放 ${esc(h.seed)} 号编队">
      <div class="h-top">
        <span>${esc((h.time || '').slice(5, 16).replace('T', ' '))} · ${esc(ST.modes[h.mode] || h.mode)} / 档位 ${esc(h.tier)}</span>
        <span class="h-seed">SEED ${esc(h.seed)}</span>
      </div>
      <div class="h-names">${h.names.map(esc).join(' / ')}</div>
    </li>`).join('');
}

// ---------------------------------------------------------------- 渲染

function renderSquad() {
  const showAv = $('#avatars').checked;
  const target = Math.max(1, Math.min(13, parseInt($('#n').value, 10) || 12));
  const positions = Math.max(12, target, squad.length) > 12 ? 14 : 12;
  $('#squad').classList.toggle('has-extra', positions > 12);
  let cards = squad.map((op, i) => {
    const e = Math.max(0, Math.min(Number(op.elite) || 0, 2));
    const isLocked = locked.has(i);
    return `
    <div class="card rar${op.rarity}${isLocked ? ' locked' : ''}" data-i="${i}" role="group" aria-label="${esc(op.name)}，${op.rarity}星${esc(op.profession)}，精英化${op.elite}，等级${op.level}${isLocked ? '，已锁定' : ''}">
      <div class="thumb" style="--c:${PROF_COLOR[op.profession] || '#454c56'}">
        <img class="fallback-prof" src="${PROF_ICON(op.profession)}" alt="" aria-hidden="true">
        ${showAv ? `<img class="portrait" src="${avatarSrc(op)}" alt=""
             referrerpolicy="no-referrer" loading="lazy" onerror="this.remove()">` : ''}
        <span class="initial">${esc(op.name.slice(0, 2))}</span>
        <div class="cap">
          <span class="clsbox"><img src="${PROF_ICON(op.profession)}" alt="${esc(op.profession)}"></span>
          <span class="stars" aria-label="${op.rarity}星">${'★'.repeat(op.rarity)}</span>
        </div>
        <span class="card-index" aria-hidden="true">${String(i + 1).padStart(2, '0')}</span>
        <div class="operator-bottom" aria-hidden="true"></div>
        <div class="lv"><i>LV</i><b>${esc(op.level)}</b></div>
        <span class="elite e${e}"><img src="${ELITE_ICON(e)}" alt="精英化${e}"></span>
        <div class="op-details" aria-label="精英化${op.elite}，潜能${op.potential}"><span>ELITE<b>0${esc(op.elite)}</b></span><div class="potential-mark"><span>${esc(op.potential)}</span></div></div>
      </div>
      <div class="name">${uiIcon('lock', 'name-lock')}<span class="name-text">${esc(op.name)}</span></div>
      <div class="sub"><span class="sub-meta">E${op.elite} · 潜${op.potential}</span><span>${esc(op.profession)}${op.converted ? ' · 转职' : ''}</span></div>
      <span class="lock-tag">${uiIcon('lock')}已锁定</span>
      <div class="acts">
        <button type="button" data-act="lock" aria-pressed="${isLocked}" aria-label="${isLocked ? '解锁' : '锁定'}${esc(op.name)}" title="${isLocked ? '取消锁定' : '锁定：整队重抽时保留'}">${uiIcon(isLocked ? 'unlock' : 'lock')}<span>${isLocked ? '解锁' : '锁定'}</span></button>
        <button type="button" data-act="respin" aria-label="只重抽${esc(op.name)}所在槽位" title="只换这一格，其余干员不变">${uiIcon('refresh')}<span>重抽</span></button>
      </div>
    </div>`;
  }).join('');
  for (let i = squad.length; i < positions; i++) cards += emptyPosition(i);
  if (!squad.length) cards += `<div class="empty-overlay"><div class="empty"><span class="empty-label">SQUAD / STANDBY</span><h3>等待干员编入</h3><p>选择抽取规则，点击右下角 <b>开始编队</b>。</p></div></div>`;
  $('#squad').innerHTML = cards;
  if (ST) renderRail();
  syncThemeStatus();
}

/** 右侧职业竖栏：显示这队里每个职业几个人，点一下只高亮该职业（照游戏的职业筛选栏做） */
function renderRail() {
  const cnt = {};
  squad.forEach((op) => { cnt[op.profession] = (cnt[op.profession] || 0) + 1; });
  $('#rail').innerHTML = `<span class="rail-heading">职业</span><button class="${!railFilter ? 'on' : ''}" data-rail="" type="button" title="显示全部职业" aria-label="显示全部职业" aria-pressed="${!railFilter}"><span class="all-text">ALL</span><span class="n">${squad.length}</span></button>` + ST.professions.map((p) => `
    <button type="button" class="${railFilter === p ? 'on' : ''}${cnt[p] ? '' : ' zero'}" data-rail="${p}" title="${p} · ${cnt[p] || 0} 人" aria-label="高亮${p}，${cnt[p] || 0}人" aria-pressed="${railFilter === p}">
      <img src="${PROF_ICON(p)}" alt="${p}"><span class="n">${cnt[p] || 0}</span>
    </button>`).join('') + `<span class="rail-end">CLASS</span>`;
  document.querySelectorAll('#squad .card').forEach((card, i) => {
    card.classList.toggle('dimmed', !!railFilter && squad[i] && squad[i].profession !== railFilter);
  });
}

function renderResult(res) {
  if (!res) return;
  const comp = {};
  res.squad.forEach((op) => { comp[op.profession] = (comp[op.profession] || 0) + 1; });
  const compText = ST.professions.filter((p) => comp[p]).map((p) => `${p} ${comp[p]}`).join(' · ');
  $('#summary').innerHTML =
    `<div class="result-meta"><div class="result-seed">SEED<b>${esc(res.seed)}</b></div>` +
    `<div class="result-pool">干员池 ${esc(res.pool.roster)} → 档位 ${esc(res.pool.after_tier)} → 可选 ${esc(res.pool.after_filter)}</div></div>` +
    `<span class="result-comp">${esc(compText)}</span>` +
    `<button id="copy" type="button" title="复制当前编队名单" aria-label="复制当前编队名单">${uiIcon('copy')}<span>复制名单</span></button>`;
  const copyButton = $('#copy');
  copyButton.onclick = async () => {
    const text = res.squad.map((o) => o.name).join(' ');
    let copied = false;
    try { await navigator.clipboard.writeText(text); copied = true; } catch {
      // file:// and some local deployments do not expose the Clipboard API.
      const input = document.createElement('textarea');
      input.value = text; input.setAttribute('readonly', '');
      input.style.cssText = 'position:fixed;left:-9999px;top:0';
      document.body.appendChild(input); input.select();
      try { copied = document.execCommand('copy'); } catch { /* Show the failure below. */ }
      input.remove(); copyButton.focus();
    }
    copyButton.querySelector('span').textContent = copied ? '已复制' : '复制失败';
    copyButton.setAttribute('aria-label', copied ? '名单已复制' : '复制失败，请检查浏览器剪贴板权限');
    copyButton.title = copied ? '名单已复制' : '复制失败，请检查浏览器剪贴板权限';
    setTimeout(() => {
      if (!copyButton.isConnected) return;
      copyButton.querySelector('span').textContent = '复制名单';
      copyButton.setAttribute('aria-label', '复制当前编队名单');
    }, 1800);
  };
  const warnings = DEMO ? ['演示模式：仅供界面预览；保底、配额、排除与种子重放以原后端为准。'] : (res.warnings || []);
  $('#warnings').innerHTML = warnings.map((w) => `<div>${esc(w)}</div>`).join('');
}

function renderError(msg) {
  $('#warnings').innerHTML = `<div class="err">${esc(msg)}</div>`;
  if (!ST) {
    $('#selection-state').textContent = '后端未连接';
    $('#box-badge').textContent = '无法读取档案';
    $('#pool-info').textContent = '离线预览：请在地址末尾添加 ?demo=1';
  }
}

function renderEmpty() {
  squad = [];
  locked.clear();
  $('#summary').innerHTML = '';
  $('#warnings').innerHTML = '';
  renderSquad();
}

/** 把 config.json 里记住的那套参数铺到控件上（在 syncControls 之后调，否则会被重建的 option 冲掉）。 */
function applyDefaults() {
  const d = ST.defaults || {};
  $('#tier').value = String(d.tier ?? 0);
  // 模式只剩两个：按职业配额（面板可编辑，默认值就是老三样保底）与纯随机。
  // 老配置/老历史里的 "floor" 直接并入配额——它俩本来就是同一个机制，分成两个选项只会让人以为切换无效。
  $('#mode').value = d.mode === 'pure' ? 'pure' : 'quota';
  $('#n').value = d.n || 12;
  $('#avoid').checked = !!d.avoid_last;
  $('#exclude').value = d.exclude || '';
  fillQuota(d.quota);
  const rs = d.rarities || [1, 2, 3, 4, 5, 6];
  document.querySelectorAll('#stars input').forEach((i) => {
    i.checked = rs.includes(Number(i.dataset.rarity));
  });
  $('#quota-panel').hidden = $('#mode').value !== 'quota';
  syncStarChips();
  updatePoolInfo();
}

// ---------------------------------------------------------------- 抽取

function buildSlots(n, respin) {
  const arr = new Array(n).fill(null);
  for (let i = 0; i < n; i++) {
    const op = squad[i];
    if (!op) continue;
    if (respin === null) { if (locked.has(i)) arr[i] = op.id; }
    else if (i !== respin) arr[i] = op.id;   // 单抽一格 = 其余全锁
  }
  return arr;
}

async function roll({ respin = null, over = null } = {}) {
  if (busy) return;
  busy = true;
  $('#roll').disabled = true;
  try {
    const n = Math.max(1, parseInt($('#n').value, 10) || 12);
    const seedText = $('#seed').value.trim();
    const body = {
      tier: parseInt($('#tier').value, 10),
      mode: $('#mode').value,
      n,
      seed: seedText ? parseInt(seedText, 10) : null,
      avoid_last: $('#avoid').checked ? 1 : 0,
      exclude: $('#exclude').value.split(/[,，\s]+/).map((s) => s.trim()).filter(Boolean),
      quota: $('#mode').value === 'quota' ? readQuota() : null,
      rarities: readRarities(),
      slots: buildSlots(n, respin),
      ...over,
    };
    const res = await api('/api/roll', body);
    squad = res.squad;
    locked = new Set([...locked].filter((i) => i < squad.length));
    renderSquad();
    renderResult(res);
    ST = await api('/api/state');     // 刷新历史
    renderHistory();
  } catch (e) {
    renderError(e.message);
  } finally {
    busy = false;
    $('#roll').disabled = false;
  }
}

// ---------------------------------------------------------------- 启动

/** 跟 ST 走的那几块重画（换 box 之后要整体刷一遍）。 */
function syncControls() {
  const keepTier = $('#tier').value;          // 重建 option 会把选中项冲掉，先记下
  $('#tier').innerHTML = Object.entries(ST.tiers)
    .map(([t, v]) => `<option value="${t}">${t} · ${v.desc}（${v.n} 人）</option>`).join('');
  if (keepTier && ST.tiers[keepTier]) $('#tier').value = keepTier;
  const av = $('#avatars');
  av.checked = ST.avatars_enabled !== false;
  $('#avatars-label').classList.toggle('off', !av.checked);
  // 演示 / 在线模式没有立绘可发：开关锁死，免得让人以为点开就能出图
  if (ST.avatars_locked) {
    av.disabled = true;
    $('#avatars-label').title = '在线演示不含干员立绘，头像固定关闭';
    const row = $('#avatars-label').closest('.setting-row');
    const desc = row && row.querySelector('.setting-description p');
    if (desc) desc.textContent = '在线演示不含干员立绘：卡片用职业色块 + 名字首字。装桌面版后才有真头像。';
  }
  renderBadge();
  renderBoxPick();
  renderHistory();
  updatePoolInfo();
  renderDemoVersion();
}

/** 页脚常驻一行：演示干员库对应的游戏版本 + 练度是演示值（在线试用时一眼能看出数据性质） */
function renderDemoVersion() {
  const src = document.querySelector('.footer-source');
  if (!src || !DEMO || !DEMO_ROSTER.version || src.dataset.demoVer) return;
  src.dataset.demoVer = '1';
  src.insertAdjacentHTML('beforeend',
    ` <span class="footer-sep">/</span> <span title="${esc(DEMO_VER_LONG)}">` +
    `${esc(DEMO_VER_SHORT)}</span>`);
}

async function onAvatarsToggle(e) {
  if (e.target.disabled) return;          // 演示/在线模式里这个开关是锁死的
  const want = e.target.checked;
  try {
    ST = await api('/api/settings', { avatars: want });
    $('#avatars-label').classList.toggle('off', !want);
    renderSquad();               // 立刻按新设置重画：关掉就不再发图片请求
  } catch (err) {
    renderError(err.message);
    e.target.checked = !want;
  }
}

async function onBoxChange(e) {
  try {
    ST = await api('/api/settings', { box: e.target.value });
  } catch (err) {
    renderError(err.message);
    return;
  }
  locked.clear();                // 换了池子，手里这队可能不在新 box 里
  syncControls();
  applyDefaults();               // 新 box 的档位人数变了，顺手把记住的参数重新铺一遍
  renderEmpty();                 // 不自动抽：什么时候抽由你按 Roll 决定
}

async function boot() {
  try {
    ST = await api('/api/state');
  } catch (e) {
    if (DEMO || TAURI) {                    // 显式演示 / 桌面版：那就是真出错，照实报
      renderError(`读不到后端状态：${e.message}（服务还在跑吗？）`);
      return;
    }
    // 既没写 ?demo=1、又不是桌面版、后端还连不上 —— 多半是把 index.html 直接打开、
    // 或者挂在 GitHub Pages 上：自动退到内置演示，并明确告诉用户看的是假数据。
    DEMO = true;
    autoDemo = true;
    ST = demoState();
  }
  // 配额输入框只建一次（跟 box 无关，重建会冲掉你填的值）
  $('#quota-inputs').innerHTML = ST.professions
    .map((p) => `<label title="${p}"><img src="${PROF_ICON(p)}" alt="${p}">` +
      `<span>${esc(p)}</span><input type="number" min="0" max="13" value="${ST.floor[p] || 0}" data-prof="${p}" aria-label="${esc(p)}配额"></label>`)
    .join('');
  syncControls();
  applyDefaults();
  if (autoDemo) {
    $('#warnings').innerHTML = '<div>没连上本地服务，已切到<b>内置演示数据</b>' +
      `（全游戏 ${DEMO_OPS.length} 名${DEMO_VER ? ` · ${esc(DEMO_VER)}` : ''}）。` +
      '精英化与等级是<b>随机分配的演示值</b>，用来看出六个档位的筛选差别；' +
      '这里也没有立绘，卡片走职业色块。要抽自己的干员池，请下载桌面版。</div>';
  }

  $('#mode').onchange = () => { $('#quota-panel').hidden = $('#mode').value !== 'quota'; };
  $('#tier').onchange = updatePoolInfo;
  $('#roll').onclick = () => roll();
  $('#avatars').onchange = onAvatarsToggle;
  $('#box').onchange = onBoxChange;
  $('#stars').onchange = syncStarChips;
  $('#rail').onclick = (e) => {
    const b = e.target.closest('button[data-rail]');
    if (!b) return;
    railFilter = railFilter === b.dataset.rail ? null : b.dataset.rail;   // 再点一次取消
    renderRail();
  };
  $('#squad').onclick = (e) => {
    const card = e.target.closest('.card');
    const btn = e.target.closest('button[data-act]');
    if (!card || !btn) return;
    const i = Number(card.dataset.i);
    if (btn.dataset.act === 'lock') {
      locked.has(i) ? locked.delete(i) : locked.add(i);
      renderSquad();
    } else if (btn.dataset.act === 'respin') {
      roll({ respin: i });
    }
  };
  $('#history').onclick = (e) => {
    const li = e.target.closest('li[data-i]');
    if (!li) return;
    const h = ST.history[Number(li.dataset.i)];
    $('#tier').value = String(h.tier);
    $('#mode').value = h.mode;
    $('#mode').onchange();
    $('#n').value = h.n;
    $('#seed').value = h.seed;
    $('#avoid').checked = false;
    fillQuota(h.quota);
    locked.clear();
    squad = [];
    updatePoolInfo();
    roll();
  };

  $('#roll').disabled = false;
  renderEmpty();      // 开页面**不自动抽**：想抽就按 Roll
}


// ---------------------------------------------------------------- Theme-only interactions
// The API, randomization, slot handling, settings payloads and history replay remain above.
function uiIcon(name, cls = '') {
  return `<svg class="${cls}" aria-hidden="true"><use href="#ico-${name}"/></svg>`;
}

function emptyPosition(i) {
  return `<div class="slot-placeholder" aria-hidden="true"><span class="slot-number">${String(i + 1).padStart(2, '0')}</span></div>`;
}

function syncThemeStatus() {
  const target = Math.max(1, Math.min(13, parseInt($('#n').value, 10) || 12));
  $('#squad-count').textContent = String(squad.length).padStart(2, '0');
  $('#squad-target').textContent = String(target).padStart(2, '0');
  $('#lock-count').textContent = squad.length ? `锁定 ${locked.size} 名 · 其余可重抽` : '尚未选择干员';
  $('#selection-state').textContent = squad.length ? `已编入 ${squad.length} 名干员${railFilter ? ` · 高亮${railFilter}` : ''}` : '等待编队';
  $('#roll .roll-text strong').textContent = squad.length ? '重新编队' : '开始编队';
  const rarities = readRarities();
  $('#filter-note').textContent = !rarities ? '已选全部稀有度' : rarities.length ? `仅选择 ${rarities.join(' / ')} 星` : '尚未选择稀有度';
  $('#demo-settings-note').hidden = !DEMO;
  updateQuotaTotal();
}

function updateQuotaTotal() {
  const total = Object.values(readQuota()).reduce((a, b) => a + b, 0);
  const target = parseInt($('#n').value, 10) || 12;
  $('#quota-total').textContent = `合计 ${total} / ${target} 人`;
  $('#quota-total').classList.toggle('invalid', total !== target);
}

function selectSettingsTab(tab) {
  document.querySelectorAll('[data-settings-tab]').forEach((button) => {
    const active = button.dataset.settingsTab === tab;
    button.classList.toggle('active', active);
    button.setAttribute('aria-pressed', String(active));
  });
  document.querySelectorAll('[data-settings-page]').forEach((page) => {
    page.hidden = page.dataset.settingsPage !== tab;
  });
}

function openSettings(tab = 'rules') {
  const dialog = $('#settings-dialog');
  selectSettingsTab(tab);
  updateQuotaTotal();
  if (!dialog.open) dialog.showModal();
}

function inputsValid() {
  if (!$('#n').checkValidity()) { $('#n').reportValidity(); return false; }
  if ($('#mode').value === 'quota') {
    const badQuota = [...document.querySelectorAll('#quota-inputs input')].find((i) => !i.checkValidity());
    if (badQuota) { openSettings(); badQuota.reportValidity(); return false; }
  }
  return true;
}

function initTheme() {
  document.querySelectorAll('[data-open-settings]').forEach((b) => b.addEventListener('click', () => openSettings()));
  document.querySelectorAll('[data-open-history]').forEach((b) => b.addEventListener('click', () => $('#history-dialog').showModal()));
  document.querySelectorAll('[data-close-dialog]').forEach((b) => b.addEventListener('click', () => b.closest('dialog').close()));
  document.querySelectorAll('[data-settings-tab]').forEach((b) => b.addEventListener('click', () => selectSettingsTab(b.dataset.settingsTab)));
  document.querySelectorAll('dialog').forEach((dialog) => {
    dialog.addEventListener('click', (e) => {
      const r = dialog.getBoundingClientRect();
      if (e.target === dialog && (e.clientX < r.left || e.clientX > r.right || e.clientY < r.top || e.clientY > r.bottom)) dialog.close();
    });
  });
  $('#mode').addEventListener('change', () => {
    // The original onchange still owns #quota-panel.hidden.
    if ($('#mode').value === 'quota') openSettings();
    syncThemeStatus();
  });
  $('#n').addEventListener('change', () => { if (ST) renderSquad(); else syncThemeStatus(); });
  $('#stars').addEventListener('change', syncThemeStatus);
  $('#quota-inputs').addEventListener('input', updateQuotaTotal);
  $('#rail').addEventListener('click', () => queueMicrotask(syncThemeStatus));
  $('#history').addEventListener('click', (e) => { if (e.target.closest('li[data-i]')) $('#history-dialog').close(); });
  $('#history').addEventListener('keydown', (e) => {
    if ((e.key === 'Enter' || e.key === ' ') && e.target.matches('li[data-i]')) { e.preventDefault(); e.target.click(); }
  });
  // Native inputs' min/max constraints also apply to a mouse-triggered roll.
  $('#roll').addEventListener('click', (e) => {
    if (!inputsValid()) { e.preventDefault(); e.stopImmediatePropagation(); }
  }, true);
  $('#squad').addEventListener('click', (e) => {
    if (e.target.closest('[data-act="respin"]') && !inputsValid()) { e.preventDefault(); e.stopImmediatePropagation(); }
  }, true);
  document.addEventListener('keydown', (e) => {
    if (e.key.toLowerCase() !== 'r' || e.repeat || e.ctrlKey || e.metaKey || e.altKey) return;
    if (e.target.closest('input, select, textarea, [contenteditable="true"]') || document.querySelector('dialog[open]')) return;
    if (ST && !busy && inputsValid()) { e.preventDefault(); roll(); }
  });
  if (DEMO) document.documentElement.classList.add('is-demo');
}

initTheme();
boot();
