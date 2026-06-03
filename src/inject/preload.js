/**
 * RAM-Stream-Browser — Preload Injection Script
 * Injected into every page via Tauri's initialization_script.
 * On B站 video pages: intercepts __playinfo__, hooks fetch, provides debug UI.
 *
 * Sprint 1 goals:
 * - Poll and capture window.__playinfo__
 * - Hook fetch to intercept /playurl API responses
 * - Read document.cookie
 * - Inject a floating debug toolbar into the page
 * - Relay everything to Rust via Tauri IPC
 */

(function () {
  'use strict';

  // ==================== Debug Toolbar ====================
  function injectToolbar() {
    if (document.getElementById('ram-debug-toolbar')) return;

    const bar = document.createElement('div');
    bar.id = 'ram-debug-toolbar';
    bar.innerHTML = `
      <div style="
        position:fixed; top:0; left:0; right:0; z-index:99999;
        background:#1a1a2e; color:#e0e0e0; padding:6px 12px;
        font-size:12px; font-family:monospace;
        display:flex; align-items:center; gap:10px;
        border-bottom:1px solid #333;
      ">
        <strong style="color:#00d4ff;">RAM-Stream</strong>
        <span id="ram-status" style="color:#888;">就绪</span>
        <button id="ram-btn-cookie" style="
          background:#333; color:#fff; border:1px solid #555;
          padding:3px 8px; cursor:pointer; border-radius:3px;
        ">🍪 提取Cookie</button>
        <button id="ram-btn-playinfo" style="
          background:#333; color:#fff; border:1px solid #555;
          padding:3px 8px; cursor:pointer; border-radius:3px;
        ">📡 提取PlayInfo</button>
        <span id="ram-playinfo-status" style="font-size:11px; color:#666;"></span>
      </div>
    `;
    document.body.appendChild(bar);

    // Wire up debug buttons
    document.getElementById('ram-btn-cookie').addEventListener('click', () => {
      const cookies = document.cookie;
      window.__TAURI_INTERNALS__?.invoke?.('submit_cookies', { cookies })
        .then(() => setStatus('✅ Cookie已发送到Rust', '#4f4'))
        .catch(e => setStatus('❌ ' + e, '#f44'));
    });

    document.getElementById('ram-btn-playinfo').addEventListener('click', () => {
      if (window.__ram_playinfo__) {
        window.__TAURI_INTERNALS__?.invoke?.('submit_playinfo', {
          jsonStr: JSON.stringify(window.__ram_playinfo__)
        }).then(() => setStatus('✅ PlayInfo已发送到Rust', '#4f4'))
          .catch(e => setStatus('❌ ' + e, '#f44'));
      } else {
        setStatus('⏳ __playinfo__ 尚未就绪', '#f90');
      }
    });
  }

  function setStatus(msg, color) {
    const el = document.getElementById('ram-status');
    if (el) { el.textContent = msg; el.style.color = color || '#888'; }
  }

  function setPlayinfoStatus(msg) {
    const el = document.getElementById('ram-playinfo-status');
    if (el) { el.textContent = msg; }
  }

  // ==================== __playinfo__ Poller ====================
  let playinfoPollTimer = null;
  const MAX_POLL_MS = 30000; // 30s max wait
  const POLL_INTERVAL = 300;
  let pollElapsed = 0;

  function startPlayinfoPoll() {
    if (playinfoPollTimer) return; // already polling

    setPlayinfoStatus('等待 __playinfo__...');
    playinfoPollTimer = setInterval(() => {
      pollElapsed += POLL_INTERVAL;

      if (window.__playinfo__) {
        window.__ram_playinfo__ = window.__playinfo__;
        clearInterval(playinfoPollTimer);
        playinfoPollTimer = null;
        pollElapsed = 0;

        const dash = window.__playinfo__?.data?.dash;
        const qualityDesc = dash
          ? `video tracks: ${dash.video?.length || 0}, audio: ${dash.audio?.length || 0}`
          : 'no dash';
        setPlayinfoStatus('✅ 就绪 | ' + qualityDesc);
        setStatus('PlayInfo已捕获', '#4f4');

        // Auto-send to Rust
        window.__TAURI_INTERNALS__?.invoke?.('submit_playinfo', {
          jsonStr: JSON.stringify(window.__playinfo__)
        }).catch(() => {});
      }

      if (pollElapsed >= MAX_POLL_MS) {
        clearInterval(playinfoPollTimer);
        playinfoPollTimer = null;
        setPlayinfoStatus('超时未捕获');
        setStatus('⚠️ __playinfo__ 超时 (可能非视频页)', '#f90');
      }
    }, POLL_INTERVAL);
  }

  // ==================== Fetch Hook ====================
  const origFetch = window.fetch;
  window.fetch = function (...args) {
    const url = typeof args[0] === 'string' ? args[0] : args[0]?.url;
    const isPlayurl = url && (
      url.includes('/playurl') ||
      url.includes('api.bilibili.com/x/player/') ||
      url.includes('api.bilibili.com/pgc/player/')
    );

    return origFetch.apply(this, args).then(response => {
      if (isPlayurl && response.ok) {
        // Clone response to read body without consuming it
        response.clone().text().then(body => {
          try {
            const json = JSON.parse(body);
            window.__TAURI_INTERNALS__?.invoke?.('submit_playurl_response', {
              url: url,
              body: body
            }).catch(() => {});

            // Also update __ram_playinfo__ if we got playinfo-like data
            if (json.data?.dash) {
              window.__ram_playinfo__ = json;
              setStatus('✅ 通过Fetch截获PlayInfo', '#4f4');
            }
          } catch (_) { /* ignore parse errors */ }
        }).catch(() => {});
      }
      return response;
    });
  };

  // ==================== Periodic Playinfo Check (for SPA navigation) ====================
  // B站 uses SPA routing, so __playinfo__ may appear without a full page reload
  let lastUrl = location.href;
  setInterval(() => {
    const currentUrl = location.href;
    if (currentUrl !== lastUrl) {
      lastUrl = currentUrl;
      pollElapsed = 0;
      if (!playinfoPollTimer) {
        startPlayinfoPoll();
      }
    }
  }, 500);

  // ==================== Init ====================
  function init() {
    injectToolbar();
    startPlayinfoPoll();
    setStatus('就绪 — 等待视频页面', '#888');
    console.log('[RAM-Stream] Preload script injected. __TAURI_INTERNALS__:', !!window.__TAURI_INTERNALS__);
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
})();
