/**
 * RAM-Stream-Browser — Preload Injection Script
 * Injected into every page via Tauri's initialization_script.
 *
 * Tauri 2.0 requires `dangerousRemoteDomainIpcAccess` in tauri.conf.json
 * for the IPC bridge to be available on external domains (bilibili.com).
 *
 * Sprint 1: Polls __playinfo__, hooks fetch, provides debug toolbar.
 */

(function () {
  'use strict';

  // ==================== Tauri IPC Bridge Detection ====================
  // In Tauri 2.0, window.__TAURI_INTERNALS__ is the IPC bridge.
  // It's injected by Tauri's runtime BEFORE our initialization_script runs.
  // But only on domains configured in dangerousRemoteDomainIpcAccess.

  function getInvoke() {
    // Try Tauri 2.0 internal API
    if (window.__TAURI_INTERNALS__ && typeof window.__TAURI_INTERNALS__.invoke === 'function') {
      return window.__TAURI_INTERNALS__.invoke.bind(window.__TAURI_INTERNALS__);
    }
    // Try Tauri 2.0 public API
    if (window.__TAURI__?.core?.invoke) {
      return window.__TAURI__.core.invoke.bind(window.__TAURI__.core);
    }
    return null;
  }

  function callRust(cmd, args) {
    const invoke = getInvoke();
    if (!invoke) {
      console.error('[RAM-Stream] ❌ Tauri IPC bridge not available!');
      console.error('[RAM-Stream] __TAURI_INTERNALS__:', window.__TAURI_INTERNALS__);
      console.error('[RAM-Stream] __TAURI__:', window.__TAURI__);
      setStatus('❌ IPC未连接 (检查 dangerousRemoteDomainIpcAccess)', '#f44');
      return Promise.reject(new Error('IPC bridge not available'));
    }
    return invoke(cmd, args).catch(e => {
      console.error('[RAM-Stream] invoke error:', cmd, e);
      throw e;
    });
  }

  // ==================== Debug Toolbar ====================
  var statusEl = null;
  var playinfoStatusEl = null;

  function injectToolbar() {
    if (document.getElementById('ram-debug-toolbar')) return;

    var bar = document.createElement('div');
    bar.id = 'ram-debug-toolbar';
    bar.innerHTML =
      '<div style="' +
        'position:fixed;top:0;left:0;right:0;z-index:99999;' +
        'background:#1a1a2e;color:#e0e0e0;padding:6px 12px;' +
        'font-size:12px;font-family:Consolas,monospace;' +
        'display:flex;align-items:center;gap:10px;' +
        'border-bottom:2px solid #00d4ff;' +
      '">' +
        '<strong style="color:#00d4ff;">RAM-Stream</strong>' +
        '<span id="ram-ipc-status" style="color:#888;">⏳</span>' +
        '<span id="ram-status" style="color:#888;">就绪</span>' +
        '<button id="ram-btn-cookie" style="' +
          'background:#333;color:#fff;border:1px solid #555;' +
          'padding:3px 8px;cursor:pointer;border-radius:3px;' +
        '">🍪 Cookie</button>' +
        '<button id="ram-btn-playinfo" style="' +
          'background:#333;color:#fff;border:1px solid #555;' +
          'padding:3px 8px;cursor:pointer;border-radius:3px;' +
        '">📡 PlayInfo</button>' +
        '<span id="ram-playinfo-status" style="font-size:11px;color:#666;"></span>' +
      '</div>';
    document.body.appendChild(bar);

    statusEl = document.getElementById('ram-status');
    playinfoStatusEl = document.getElementById('ram-playinfo-status');

    // Wire up buttons
    document.getElementById('ram-btn-cookie').addEventListener('click', function () {
      var cookies = document.cookie;
      console.log('[RAM-Stream] 🍪 Sending cookies to Rust... Cookie length:', cookies.length);
      callRust('submit_cookies', { cookies: cookies })
        .then(function () { setStatus('✅ Cookie已发送', '#4f4'); })
        .catch(function (e) { setStatus('❌ ' + e, '#f44'); });
    });

    document.getElementById('ram-btn-playinfo').addEventListener('click', function () {
      if (window.__ram_playinfo__) {
        var jsonStr = JSON.stringify(window.__ram_playinfo__);
        console.log('[RAM-Stream] 📡 Sending PlayInfo to Rust... Size:', jsonStr.length);
        callRust('submit_playinfo', { jsonStr: jsonStr })
          .then(function () { setStatus('✅ PlayInfo已发送', '#4f4'); })
          .catch(function (e) { setStatus('❌ ' + e, '#f44'); });
      } else {
        setStatus('⏳ __playinfo__ 未就绪', '#f90');
      }
    });
  }

  function setStatus(msg, color) {
    if (statusEl) { statusEl.textContent = msg; statusEl.style.color = color || '#888'; }
    console.log('[RAM-Stream] Status:', msg);
  }

  function setPlayinfoStatus(msg) {
    if (playinfoStatusEl) { playinfoStatusEl.textContent = msg; }
  }

  // ==================== IPC Detection ====================
  function checkIPC() {
    var ipcEl = document.getElementById('ram-ipc-status');
    if (!ipcEl) return;
    var invoke = getInvoke();
    if (invoke) {
      ipcEl.textContent = '🟢';
      ipcEl.title = 'Tauri IPC connected';
      console.log('[RAM-Stream] ✅ Tauri IPC bridge found');
      return true;
    } else {
      ipcEl.textContent = '🔴';
      ipcEl.title = 'Tauri IPC NOT connected! Check dangerousRemoteDomainIpcAccess';
      console.error('[RAM-Stream] ❌ Tauri IPC bridge NOT found');
      console.log('[RAM-Stream] Available globals:', Object.keys(window).filter(function(k) {
        return k.startsWith('__TAURI');
      }));
      return false;
    }
  }

  // ==================== __playinfo__ Poller ====================
  var playinfoPollTimer = null;
  var MAX_POLL_MS = 30000;
  var POLL_INTERVAL = 300;
  var pollElapsed = 0;

  function startPlayinfoPoll() {
    if (playinfoPollTimer) return;
    setPlayinfoStatus('⏳ 等待 __playinfo__...');
    pollElapsed = 0;

    playinfoPollTimer = setInterval(function () {
      pollElapsed += POLL_INTERVAL;

      if (window.__playinfo__) {
        window.__ram_playinfo__ = window.__playinfo__;
        clearInterval(playinfoPollTimer);
        playinfoPollTimer = null;

        var dash = window.__playinfo__?.data?.dash;
        var desc = dash
          ? 'video:' + (dash.video?.length || 0) + ' audio:' + (dash.audio?.length || 0)
          : 'no dash';
        setPlayinfoStatus('✅ ' + desc);
        setStatus('PlayInfo已捕获', '#4f4');
        console.log('[RAM-Stream] ✅ __playinfo__ captured:', desc);

        // Auto-send to Rust if IPC is available
        if (getInvoke()) {
          callRust('submit_playinfo', { jsonStr: JSON.stringify(window.__playinfo__) })
            .catch(function () {});
        }
      }

      if (pollElapsed >= MAX_POLL_MS) {
        clearInterval(playinfoPollTimer);
        playinfoPollTimer = null;
        setPlayinfoStatus('⏰ 超时');
        setStatus('⚠️ 非视频页或 __playinfo__ 未出现', '#f90');
      }
    }, POLL_INTERVAL);
  }

  // ==================== Fetch Hook ====================
  var origFetch = window.fetch;
  window.fetch = function () {
    var args = arguments;
    var url = typeof args[0] === 'string' ? args[0] : (args[0]?.url || '');
    var isPlayurl = url && (
      url.indexOf('/playurl') !== -1 ||
      url.indexOf('api.bilibili.com/x/player/') !== -1 ||
      url.indexOf('api.bilibili.com/pgc/player/') !== -1
    );

    return origFetch.apply(this, args).then(function (response) {
      if (isPlayurl && response.ok) {
        response.clone().text().then(function (body) {
          try {
            var json = JSON.parse(body);
            if (getInvoke()) {
              callRust('submit_playurl_response', { url: url, body: body }).catch(function () {});
            }
            if (json.data?.dash) {
              window.__ram_playinfo__ = json;
              setStatus('✅ Fetch截获PlayInfo', '#4f4');
              console.log('[RAM-Stream] ✅ PlayInfo via fetch hook');
            }
          } catch (_) {}
        }).catch(function () {});
      }
      return response;
    });
  };

  // ==================== SPA Navigation Detection ====================
  var lastUrl = location.href;
  setInterval(function () {
    var currentUrl = location.href;
    if (currentUrl !== lastUrl) {
      lastUrl = currentUrl;
      console.log('[RAM-Stream] 🔄 SPA navigation detected:', currentUrl);
      pollElapsed = 0;
      if (!playinfoPollTimer) {
        startPlayinfoPoll();
      }
    }
  }, 500);

  // ==================== Init ====================
  function init() {
    console.log('[RAM-Stream] ====== Preload Script v0.1.0 ======');
    console.log('[RAM-Stream] URL:', location.href);
    console.log('[RAM-Stream] __TAURI_INTERNALS__:', !!window.__TAURI_INTERNALS__);
    console.log('[RAM-Stream] __TAURI__:', !!window.__TAURI__);

    injectToolbar();

    // Delayed IPC check (bridge might initialize after our script)
    setTimeout(function () {
      var ok = checkIPC();
      if (!ok) {
        // Retry after a delay
        setTimeout(checkIPC, 1000);
        setTimeout(checkIPC, 3000);
      }
      if (ok) setStatus('IPC已连接 ✓', '#4f4');
    }, 100);

    startPlayinfoPoll();
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
})();
