/**
 * Solana Ledger Explorer — JavaScript Runtime
 * Apache HTTP hosting edition
 *
 * Responsibilities:
 *  - Background polling loop (configurable interval) against the Solana JSON-RPC
 *  - Render recent blocks (slots), transactions, and network stats
 *  - Search by slot, transaction signature, or account address
 *  - Slide-in detail panel for blocks and transactions
 *  - Background CLI-style injection: submit raw base64-encoded transactions via RPC
 *  - Toast notifications
 *  - Tab switching
 */

'use strict';

/* =========================================================
   Configuration
   ========================================================= */
const CONFIG = {
  /** Default RPC endpoint — user can override via the UI */
  rpcUrl: 'https://api.mainnet-beta.solana.com',

  /** How often (ms) the background loop re-fetches ledger data */
  pollInterval: 5000,

  /** Number of recent blocks shown in the table */
  recentBlocksLimit: 20,

  /** Number of recent transactions shown in the table */
  recentTxLimit: 20,

  /** Solana networks available in the selector */
  networks: {
    'mainnet-beta': 'https://api.mainnet-beta.solana.com',
    'devnet':       'https://api.devnet.solana.com',
    'testnet':      'https://api.testnet.solana.com',
    'localnet':     'http://127.0.0.1:8899',
  },
};

/* =========================================================
   State
   ========================================================= */
let state = {
  rpcUrl:     CONFIG.rpcUrl,
  polling:    false,
  pollTimer:  null,
  blocks:     [],
  txs:        [],
  slot:       0,
  epoch:      0,
  tps:        0,
  blockTime:  0,
  validators: 0,
};

/* =========================================================
   JSON-RPC helpers
   ========================================================= */

/**
 * Send a single JSON-RPC 2.0 request.
 * @param {string} method
 * @param {Array}  params
 * @returns {Promise<any>} resolved with the `result` field
 */
async function rpc(method, params = []) {
  const resp = await fetch(state.rpcUrl, {
    method:  'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      jsonrpc: '2.0',
      id:      1,
      method,
      params,
    }),
  });

  if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
  const json = await resp.json();
  if (json.error) throw new Error(json.error.message || 'RPC error');
  return json.result;
}

/**
 * Send multiple JSON-RPC requests in a single batch.
 * @param {Array<{method: string, params: Array}>} requests
 * @returns {Promise<Array<any>>} results in the same order
 */
async function rpcBatch(requests) {
  const body = requests.map((r, i) => ({
    jsonrpc: '2.0',
    id: i,
    method: r.method,
    params: r.params || [],
  }));

  const resp = await fetch(state.rpcUrl, {
    method:  'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify(body),
  });

  if (!resp.ok) throw new Error(`HTTP ${resp.status}`);
  const json = await resp.json();
  return json.sort((a, b) => a.id - b.id).map(r => {
    if (r.error) throw new Error(r.error.message);
    return r.result;
  });
}

/* =========================================================
   Data fetching
   ========================================================= */

/** Fetch the latest slot, epoch info, recent performance samples */
async function fetchNetworkStats() {
  const [slotResult, epochResult, perfResult] = await rpcBatch([
    { method: 'getSlot' },
    { method: 'getEpochInfo' },
    { method: 'getRecentPerformanceSamples', params: [1] },
  ]);

  state.slot  = slotResult;
  state.epoch = epochResult.epoch;

  if (perfResult && perfResult.length > 0) {
    const s = perfResult[0];
    state.tps = s.numTransactions
      ? Math.round(s.numTransactions / s.samplePeriodSecs)
      : 0;
  }
}

/** Fetch the last N confirmed blocks with their transaction counts */
async function fetchRecentBlocks() {
  const endSlot   = state.slot;
  const startSlot = Math.max(0, endSlot - CONFIG.recentBlocksLimit * 2);

  const slots = await rpc('getBlocksWithLimit', [
    Math.max(0, endSlot - CONFIG.recentBlocksLimit + 1),
    CONFIG.recentBlocksLimit,
  ]);

  if (!slots || slots.length === 0) return;

  // Fetch block info for each slot in a single batch
  const blockBatch = slots.map(slot => ({
    method: 'getBlock',
    params: [
      slot,
      {
        encoding:                         'json',
        maxSupportedTransactionVersion:   0,
        transactionDetails:               'signatures',
        rewards:                          false,
      },
    ],
  }));

  let blocks;
  try {
    blocks = await rpcBatch(blockBatch);
  } catch (_) {
    // Fallback: fetch one by one, skip nulls
    blocks = await Promise.all(
      slots.map(slot =>
        rpc('getBlock', [
          slot,
          {
            encoding:                       'json',
            maxSupportedTransactionVersion: 0,
            transactionDetails:             'signatures',
            rewards:                        false,
          },
        ]).catch(() => null),
      ),
    );
  }

  state.blocks = slots
    .map((slot, i) => ({ slot, ...(blocks[i] || {}) }))
    .filter(b => b.blockhash)
    .reverse()
    .slice(0, CONFIG.recentBlocksLimit);

  if (state.blocks.length > 0 && state.blocks[0].blockTime) {
    state.blockTime = state.blocks[0].blockTime;
  }
}

/** Fetch recent confirmed transaction signatures for a representative program */
async function fetchRecentTransactions() {
  try {
    const sigs = await rpc('getSignaturesForAddress', [
      'Vote111111111111111111111111111111111111111',
      { limit: CONFIG.recentTxLimit },
    ]);
    state.txs = sigs || [];
  } catch (_) {
    state.txs = [];
  }
}

/* =========================================================
   Background polling loop
   ========================================================= */

async function tick() {
  try {
    await fetchNetworkStats();
    renderStats();

    await Promise.allSettled([fetchRecentBlocks(), fetchRecentTransactions()]);
    renderBlocks();
    renderTransactions();
  } catch (err) {
    console.warn('[ledger] poll error:', err.message);
  }
}

function startPolling() {
  if (state.polling) return;
  state.polling = true;
  tick(); // immediate first run
  state.pollTimer = setInterval(tick, CONFIG.pollInterval);
  updateLiveIndicator(true);
}

function stopPolling() {
  state.polling = false;
  clearInterval(state.pollTimer);
  state.pollTimer = null;
  updateLiveIndicator(false);
}

/* =========================================================
   Render helpers
   ========================================================= */

function shortHash(hash, n = 8) {
  if (!hash) return '—';
  return `${hash.slice(0, n)}…${hash.slice(-n)}`;
}

function timeAgo(unixSeconds) {
  if (!unixSeconds) return '—';
  const diff = Math.floor(Date.now() / 1000) - unixSeconds;
  if (diff < 60)   return `${diff}s ago`;
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  return `${Math.floor(diff / 3600)}h ago`;
}

function fmtNumber(n) {
  if (n === undefined || n === null) return '—';
  return Number(n).toLocaleString();
}

function skeletonRows(cols, count = 6) {
  return Array.from({ length: count }, () =>
    `<tr class="skeleton-row">${Array.from({ length: cols }, () =>
      `<td><span class="skeleton" style="width:${40 + Math.random() * 50}%;height:14px;">&nbsp;</span></td>`,
    ).join('')}</tr>`,
  ).join('');
}

/* =========================================================
   Render: stats bar
   ========================================================= */

function renderStats() {
  setText('stat-slot',  fmtNumber(state.slot));
  setText('stat-epoch', fmtNumber(state.epoch));
  setText('stat-tps',   fmtNumber(state.tps));

  const blockTimeEl = document.getElementById('stat-blocktime');
  if (blockTimeEl && state.blockTime) {
    blockTimeEl.textContent = new Date(state.blockTime * 1000).toLocaleTimeString();
  }
}

/* =========================================================
   Render: blocks table
   ========================================================= */

function renderBlocks() {
  const tbody = document.getElementById('blocks-tbody');
  if (!tbody) return;

  if (state.blocks.length === 0) {
    tbody.innerHTML = skeletonRows(5);
    return;
  }

  tbody.innerHTML = state.blocks
    .map(b => {
      const txCount = b.transactions ? b.transactions.length : '—';
      return `
        <tr class="block-row" data-slot="${b.slot}" style="cursor:pointer">
          <td><span class="slot-num">${fmtNumber(b.slot)}</span></td>
          <td><span class="hash hash-short" title="${b.blockhash}">${shortHash(b.blockhash)}</span></td>
          <td>${fmtNumber(txCount)}</td>
          <td>${b.parentSlot !== undefined ? fmtNumber(b.parentSlot) : '—'}</td>
          <td class="text-muted">${timeAgo(b.blockTime)}</td>
        </tr>`;
    })
    .join('');

  // Row click → detail
  tbody.querySelectorAll('.block-row').forEach(row => {
    row.addEventListener('click', () => {
      const slot = Number(row.dataset.slot);
      const block = state.blocks.find(b => b.slot === slot);
      if (block) openBlockDetail(block);
    });
  });
}

/* =========================================================
   Render: transactions table
   ========================================================= */

function renderTransactions() {
  const tbody = document.getElementById('txs-tbody');
  if (!tbody) return;

  if (state.txs.length === 0) {
    tbody.innerHTML = skeletonRows(4);
    return;
  }

  tbody.innerHTML = state.txs
    .map(tx => {
      const statusBadge = tx.err
        ? '<span class="badge badge-danger">Failed</span>'
        : '<span class="badge badge-success">Success</span>';
      return `
        <tr class="tx-row" data-sig="${tx.signature}" style="cursor:pointer">
          <td><span class="hash" title="${tx.signature}">${shortHash(tx.signature, 10)}</span></td>
          <td>${tx.slot !== undefined ? fmtNumber(tx.slot) : '—'}</td>
          <td>${statusBadge}</td>
          <td class="text-muted">${timeAgo(tx.blockTime)}</td>
        </tr>`;
    })
    .join('');

  tbody.querySelectorAll('.tx-row').forEach(row => {
    row.addEventListener('click', () => {
      openTxDetail(row.dataset.sig);
    });
  });
}

/* =========================================================
   Detail panel
   ========================================================= */

function openPanel(titleText) {
  document.getElementById('detail-title').textContent = titleText;
  document.getElementById('detail-body').innerHTML =
    '<p class="text-muted" style="padding:1rem 0">Loading…</p>';
  document.getElementById('detail-panel').classList.add('open');
  document.getElementById('overlay').classList.add('show');
}

function closePanel() {
  document.getElementById('detail-panel').classList.remove('open');
  document.getElementById('overlay').classList.remove('show');
}

function renderDetailRows(rows) {
  return rows
    .map(([k, v]) => `
      <div class="detail-row">
        <span class="detail-key">${k}</span>
        <span class="detail-val">${v}</span>
      </div>`)
    .join('');
}

/** Open block detail slide-in */
function openBlockDetail(block) {
  openPanel(`Block #${fmtNumber(block.slot)}`);
  const txCount = block.transactions ? block.transactions.length : '—';
  const rows = [
    ['Slot',          fmtNumber(block.slot)],
    ['Parent Slot',   fmtNumber(block.parentSlot)],
    ['Blockhash',     block.blockhash || '—'],
    ['Previous Hash', block.previousBlockhash || '—'],
    ['Transactions',  fmtNumber(txCount)],
    ['Block Time',    block.blockTime
      ? new Date(block.blockTime * 1000).toLocaleString()
      : '—'],
    ['Block Height',  fmtNumber(block.blockHeight)],
  ];

  let html = renderDetailRows(rows);

  if (block.transactions && block.transactions.length > 0) {
    html += `<div style="margin-top:1rem;font-size:.8rem;font-weight:700;
                  text-transform:uppercase;color:var(--text-muted);
                  margin-bottom:.5rem">Signatures</div>`;
    html += block.transactions
      .slice(0, 20)
      .map(t => {
        const sig = typeof t === 'string' ? t : (t.transaction?.signatures?.[0] ?? '?');
        return `<div style="font-family:var(--font-mono);font-size:.78rem;
                      word-break:break-all;padding:.3rem 0;
                      border-bottom:1px solid var(--border);
                      color:var(--accent);cursor:pointer"
                  class="sig-link" data-sig="${sig}">${sig}</div>`;
      })
      .join('');
    if (block.transactions.length > 20) {
      html += `<div class="text-muted mt-1" style="font-size:.8rem">
                 … and ${block.transactions.length - 20} more</div>`;
    }
  }

  document.getElementById('detail-body').innerHTML = html;

  // Allow clicking signatures inside detail panel
  document.querySelectorAll('.sig-link').forEach(el => {
    el.addEventListener('click', () => openTxDetail(el.dataset.sig));
  });
}

/** Open transaction detail slide-in, fetching full details from RPC */
async function openTxDetail(sig) {
  openPanel('Transaction');
  try {
    const tx = await rpc('getTransaction', [
      sig,
      { encoding: 'jsonParsed', maxSupportedTransactionVersion: 0 },
    ]);

    if (!tx) {
      document.getElementById('detail-body').innerHTML =
        '<p class="text-muted" style="padding:1rem 0">Transaction not found.</p>';
      return;
    }

    const meta  = tx.meta  || {};
    const inner = tx.transaction || {};
    const msg   = inner.message || {};

    const statusBadge = meta.err
      ? '<span class="badge badge-danger">Failed</span>'
      : '<span class="badge badge-success">Success</span>';

    const rows = [
      ['Signature',    sig],
      ['Status',       statusBadge],
      ['Slot',         fmtNumber(tx.slot)],
      ['Block Time',   tx.blockTime ? new Date(tx.blockTime * 1000).toLocaleString() : '—'],
      ['Fee (lamports)', fmtNumber(meta.fee)],
      ['Compute Units',  fmtNumber(meta.computeUnitsConsumed)],
      ['Version',      tx.version !== undefined ? String(tx.version) : 'legacy'],
    ];

    let html = renderDetailRows(rows);

    const accounts = msg.accountKeys || [];
    if (accounts.length > 0) {
      html += `<div style="margin-top:1rem;font-size:.8rem;font-weight:700;
                    text-transform:uppercase;color:var(--text-muted);
                    margin-bottom:.5rem">Accounts</div>`;
      html += accounts.map((a, i) => {
        const key    = typeof a === 'string' ? a : a.pubkey;
        const signer = typeof a !== 'string' && a.signer ? ' 🖊' : '';
        const writer = typeof a !== 'string' && a.writable ? ' ✏️' : '';
        return `<div style="font-family:var(--font-mono);font-size:.78rem;
                      word-break:break-all;padding:.3rem 0;
                      border-bottom:1px solid var(--border)">
                  <span style="color:var(--text-muted);margin-right:.5rem">[${i}]</span>
                  <span style="color:var(--accent)">${key}</span>
                  <span style="color:var(--text-muted)">${signer}${writer}</span>
                </div>`;
      }).join('');
    }

    if (meta.logMessages && meta.logMessages.length > 0) {
      html += `<div style="margin-top:1rem;font-size:.8rem;font-weight:700;
                    text-transform:uppercase;color:var(--text-muted);
                    margin-bottom:.5rem">Program Logs</div>`;
      html += `<pre style="background:var(--bg-input);border-radius:var(--radius);
                     padding:.75rem;font-size:.78rem;overflow-x:auto;
                     color:var(--text-muted);border:1px solid var(--border)">${
        meta.logMessages.join('\n')
      }</pre>`;
    }

    document.getElementById('detail-body').innerHTML = html;
  } catch (err) {
    document.getElementById('detail-body').innerHTML =
      `<p style="color:var(--danger);padding:1rem 0">Error: ${err.message}</p>`;
  }
}

/* =========================================================
   Search
   ========================================================= */

async function handleSearch() {
  const raw = document.getElementById('search-input').value.trim();
  if (!raw) return;

  // Heuristic: 88-char base58 → signature; 32–44-char → pubkey; numeric → slot
  if (/^\d+$/.test(raw)) {
    const slotNum = parseInt(raw, 10);
    try {
      const block = await rpc('getBlock', [
        slotNum,
        {
          encoding:                       'json',
          maxSupportedTransactionVersion: 0,
          transactionDetails:             'signatures',
          rewards:                        false,
        },
      ]);
      if (block) openBlockDetail({ slot: slotNum, ...block });
      else toast('Block not found for that slot', 'error');
    } catch (e) {
      toast(`Block lookup error: ${e.message}`, 'error');
    }
  } else if (raw.length >= 86) {
    openTxDetail(raw);
  } else {
    // Treat as account address — show recent sigs
    try {
      const sigs = await rpc('getSignaturesForAddress', [raw, { limit: 10 }]);
      if (!sigs || sigs.length === 0) {
        toast('No transactions found for that address', 'info');
        return;
      }
      openPanel(`Account: ${shortHash(raw)}`);
      const html = sigs.map(s => `
        <div style="padding:.5rem 0;border-bottom:1px solid var(--border)">
          <span class="sig-link" data-sig="${s.signature}"
                style="font-family:var(--font-mono);font-size:.8rem;
                       color:var(--accent);cursor:pointer;word-break:break-all">
            ${s.signature}
          </span>
          <div class="text-muted" style="font-size:.75rem;margin-top:.2rem">
            Slot ${fmtNumber(s.slot)} · ${timeAgo(s.blockTime)}
          </div>
        </div>`).join('');
      document.getElementById('detail-body').innerHTML = html;
      document.querySelectorAll('.sig-link').forEach(el => {
        el.addEventListener('click', () => openTxDetail(el.dataset.sig));
      });
    } catch (e) {
      toast(`Address lookup error: ${e.message}`, 'error');
    }
  }
}

/* =========================================================
   Background CLI injection into ledger
   Submits a raw, base64-encoded, fully-signed transaction
   to the cluster via sendTransaction RPC.
   ========================================================= */

async function injectTransaction() {
  const rawInput  = document.getElementById('inject-tx').value.trim();
  const encoding  = document.getElementById('inject-encoding').value;
  const skipPre   = document.getElementById('inject-skip-preflight').checked;

  if (!rawInput) {
    toast('Paste a signed transaction first', 'error');
    return;
  }

  const btn = document.getElementById('inject-btn');
  btn.disabled = true;
  btn.textContent = 'Sending…';

  try {
    const sig = await rpc('sendTransaction', [
      rawInput,
      {
        encoding,
        skipPreflight:              skipPre,
        preflightCommitment:        'processed',
        maxRetries:                 3,
      },
    ]);
    toast(`✓ Injected! Signature: ${shortHash(sig)}`, 'success');
    document.getElementById('inject-result').textContent = sig;
    document.getElementById('inject-result-row').classList.remove('hidden');
  } catch (err) {
    toast(`Injection failed: ${err.message}`, 'error');
  } finally {
    btn.disabled = false;
    btn.textContent = 'Inject Transaction';
  }
}

/* =========================================================
   Network switching
   ========================================================= */

function switchNetwork(url) {
  state.rpcUrl = url;
  stopPolling();
  state.blocks = [];
  state.txs    = [];
  renderBlocks();
  renderTransactions();
  startPolling();
  toast(`Switched to ${url}`, 'info');
}

/* =========================================================
   Toast notifications
   ========================================================= */

function toast(message, type = 'info', duration = 4000) {
  const container = document.getElementById('toast-container');
  const el = document.createElement('div');
  el.className = `toast ${type}`;
  el.textContent = message;
  container.appendChild(el);
  setTimeout(() => {
    el.style.animation = 'none';
    el.style.opacity = '0';
    el.style.transition = 'opacity .25s';
    setTimeout(() => el.remove(), 280);
  }, duration);
}

/* =========================================================
   Tab switching
   ========================================================= */

function initTabs() {
  document.querySelectorAll('.tab-btn').forEach(btn => {
    btn.addEventListener('click', () => {
      const target = btn.dataset.tab;
      document.querySelectorAll('.tab-btn').forEach(b => b.classList.remove('active'));
      document.querySelectorAll('.tab-content').forEach(c => c.classList.remove('active'));
      btn.classList.add('active');
      document.getElementById(`tab-${target}`)?.classList.add('active');
    });
  });
}

/* =========================================================
   Utilities
   ========================================================= */

function setText(id, value) {
  const el = document.getElementById(id);
  if (el) el.textContent = value;
}

function updateLiveIndicator(live) {
  const dot = document.querySelector('.live-dot');
  if (dot) dot.style.background = live ? 'var(--success)' : 'var(--danger)';

  const btn = document.getElementById('toggle-poll-btn');
  if (btn) btn.textContent = live ? 'Pause' : 'Resume';
}

/* =========================================================
   Bootstrap
   ========================================================= */

document.addEventListener('DOMContentLoaded', () => {
  // Network selector
  const netSel = document.getElementById('network-select');
  if (netSel) {
    Object.entries(CONFIG.networks).forEach(([name, url]) => {
      const opt    = document.createElement('option');
      opt.value    = url;
      opt.textContent = name;
      if (url === CONFIG.rpcUrl) opt.selected = true;
      netSel.appendChild(opt);
    });
    netSel.addEventListener('change', () => switchNetwork(netSel.value));
  }

  // Search
  document.getElementById('search-btn')
    ?.addEventListener('click', handleSearch);
  document.getElementById('search-input')
    ?.addEventListener('keydown', e => { if (e.key === 'Enter') handleSearch(); });

  // Detail panel close
  document.getElementById('close-detail')?.addEventListener('click', closePanel);
  document.getElementById('overlay')?.addEventListener('click', closePanel);

  // Poll toggle
  document.getElementById('toggle-poll-btn')?.addEventListener('click', () => {
    if (state.polling) stopPolling();
    else startPolling();
  });

  // Inject button
  document.getElementById('inject-btn')?.addEventListener('click', injectTransaction);

  // Copy injected signature
  document.getElementById('copy-sig-btn')?.addEventListener('click', () => {
    const sig = document.getElementById('inject-result').textContent;
    navigator.clipboard?.writeText(sig).then(() => toast('Signature copied!', 'success'));
  });

  // Tabs
  initTabs();

  // Start background polling
  startPolling();
});
