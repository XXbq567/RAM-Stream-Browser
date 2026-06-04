/**
 * RAM-Stream-Browser — Preload Injection Script
 * Injected into every page via Tauri's initialization_script.
 *
 * Tauri 2.0 does NOT inject __TAURI_INTERNALS__ on external domains
 * (e.g., bilibili.com). We use a custom `ram-stream://` URI scheme
 * protocol as the fallback IPC bridge:
 *
 *   fetch('ram-stream://localhost/<command>', { method:'POST', body: JSON.stringify(args) })
 *
 * Sprint 1: Polls __playinfo__, hooks fetch, provides debug toolbar.
 */

(function () {
  'use strict';

  // ==================== Navigation Fixes ====================

  // Override window.open — redirect popups/new-tabs to current window
  var origOpen = window.open;
  window.open = function(url, target, features) {
      if (url && typeof url === 'string' && url.indexOf('http') === 0) {
          console.log('[RAM-Stream] 🔗 window.open redirected:', url);
          location.href = url;
          return null;
      }
      return origOpen.apply(window, arguments);
  };

  // Prevent target="_blank" from opening external browser
  document.addEventListener('click', function(e) {
      var el = e.target;
      while (el && el.tagName !== 'A') {
          el = el.parentElement;
      }
      if (el && el.getAttribute('target') === '_blank') {
          e.preventDefault();
          e.stopPropagation();
          var href = el.getAttribute('href');
          if (href && href.indexOf('http') === 0) {
              console.log('[RAM-Stream] 🔗 _blank link intercepted:', href);
              location.href = href;
          }
      }
  }, true);

  // ==================== Tauri IPC Bridge Detection ====================
  // In Tauri 2.0, window.__TAURI_INTERNALS__ is the IPC bridge.
  // It is only injected on App-origin pages (not external domains).
  // On bilibili.com, we fall back to the ram-stream:// custom protocol.

  function getInvoke() {
    // Try Tauri 2.0 internal API (only on App URLs)
    if (window.__TAURI_INTERNALS__ && typeof window.__TAURI_INTERNALS__.invoke === 'function') {
      return window.__TAURI_INTERNALS__.invoke.bind(window.__TAURI_INTERNALS__);
    }
    // Try Tauri 2.0 public API
    if (window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke) {
      return window.__TAURI__.core.invoke.bind(window.__TAURI__.core);
    }
    return null;
  }

  /// Primary call to Rust backend.
  /// - On localhost/bridge pages, Tauri IPC is available — use it (faster, no URL length limit).
  /// - On bilibili.com (external domain), Tauri 2.0 IPC is blocked by ACL — fall back to img beacon.
  function callRust(cmd, args) {
    if (window.location.hostname === 'localhost' || window.location.hostname === '127.0.0.1') {
      var invoke = getInvoke();
      if (invoke) {
        console.log('[RAM-Stream] callRust via IPC:', cmd);
        return invoke(cmd, args);
      }
    }
    // On external domains or if IPC unavailable, use img beacon fallback
    console.log('[RAM-Stream] 🖼️ callRust → img beacon:', cmd);
    return callRustViaProtocol(cmd, args);
  }

  /// Send command to Rust via ram-stream:// img beacon.
  /// Images are passive content — NOT subject to CORS/mixed-content blocking.
  /// Data is encoded in the URL query string to bypass all WebView2 restrictions.
  function callRustViaProtocol(cmd, args) {
    var body = JSON.stringify(args || {});
    var encoded = encodeURIComponent(body);
    var url = 'ram-stream://localhost/' + cmd + '?d=' + encoded;
    console.log('[RAM-Stream] 🖼️ Img beacon →', cmd, '(' + body.length + ' bytes)');

    return new Promise(function (resolve, reject) {
        var img = new Image();
        var done = false;
        var timeout = setTimeout(function () {
            if (!done) {
                done = true;
                console.error('[RAM-Stream] ⏰ Img beacon timeout:', cmd);
                reject(new Error('Img beacon timeout'));
            }
        }, 8000);

        img.onload = function () {
            if (!done) {
                done = true;
                clearTimeout(timeout);
                console.log('[RAM-Stream] ✅ Img beacon loaded:', cmd);
                resolve({ ok: true, via: 'img' });
            }
        };
        img.onerror = function () {
            if (!done) {
                done = true;
                clearTimeout(timeout);
                // onerror for custom protocol usually means the request was sent
                // but the response wasn't a valid image. We still count it as success
                // since the Rust handler received and processed the data.
                console.log('[RAM-Stream] ✅ Img beacon sent (non-image response):', cmd);
                resolve({ ok: true, via: 'img' });
            }
        };
        img.src = url;
    });
  }

  // ==================== Cookie Auto-Send ====================
  /**
   * Auto-send cookies to Rust on every page load and SPA navigation.
   */
  function autoSendCookies() {
    var cookies = document.cookie;
    if (!cookies || cookies.length <= 10) {
      console.log('[RAM-Stream] No cookies to send (length:', (cookies ? cookies.length : 0) + ')');
      return;
    }
    // Only auto-send on video pages or when playinfo is already detected.
    // Sending on every page (homepage, search, etc.) triggers unnecessary img beacon requests.
    var url = location.href;
    var isVideoPage = url.indexOf('/video/') !== -1 || url.indexOf('/bangumi/') !== -1;
    if (!isVideoPage && !window.__playinfo__) {
      console.log('[RAM-Stream] Skipping cookie auto-send on non-video page');
      return;
    }
    console.log('[RAM-Stream] Auto-sending cookies, length:', cookies.length);
    callRust('submit_cookies', { cookies: cookies });
  }

  // ==================== Debug Toolbar ====================
  var statusEl = null;
  var playinfoStatusEl = null;

  /**
   * Build quality selector from playinfo DASH video array.
   */
  function buildQualitySelector(playinfo) {
    var select = document.getElementById('ram-quality-select');
    if (!select) return;

    var videos = [];
    try {
      videos = (playinfo && playinfo.data && playinfo.data.dash && playinfo.data.dash.video) || [];
    } catch(e) {
      console.log('[RAM-Stream] Cannot parse quality list:', e);
      return;
    }

    if (!videos.length) return;

    select.innerHTML = '';
    var qualityMap = {
      127:'8K', 125:'4K HDR', 120:'4K', 116:'1080p60 HDR',
      112:'1080p60+', 80:'1080p', 74:'720p60', 64:'720p',
      48:'720p(low)', 32:'480p', 16:'360p'
    };
    var codecMap = {7:'AVC', 12:'HEVC', 13:'AV1'};

    videos.forEach(function(v) {
      var qid = v.id || v.quality;
      var desc = qualityMap[qid] || ('q' + qid);
      var codec = codecMap[v.codecid] || '';
      var bw = '';
      if (v.bandwidth) {
        if (v.bandwidth >= 1000000) bw = (v.bandwidth/1000000).toFixed(1) + 'M';
        else if (v.bandwidth >= 1000) bw = Math.round(v.bandwidth/1000) + 'K';
      }
      var opt = document.createElement('option');
      opt.value = qid;
      opt.textContent = desc + (codec ? ' ' + codec : '') + (bw ? ' ' + bw : '');
      select.appendChild(opt);
    });

    // Auto-select highest quality (first in the array)
    if (videos.length > 0) {
      select.value = videos[0].id || videos[0].quality;
    }
  }

  function injectToolbar() {
    if (document.getElementById('ram-debug-toolbar')) return;

    var bar = document.createElement('div');
    bar.id = 'ram-debug-toolbar';
    bar.innerHTML =
      '<div style="' +
        'position:fixed;bottom:0;left:0;right:0;z-index:99999;' +
        'background:#1a1a2e;color:#e0e0e0;padding:4px 10px;' +
        'font-size:11px;font-family:Consolas,monospace;' +
        'display:flex;align-items:center;gap:10px;' +
        'border-top:2px solid #00d4ff;' +
      '">' +
        '<button id="ram-btn-back" style="' +
          'background:#333;color:#fff;border:1px solid #555;' +
          'padding:2px 6px;cursor:pointer;border-radius:3px;font-size:11px;' +
        '" title="后退">←</button>' +
        '<button id="ram-btn-forward" style="' +
          'background:#333;color:#fff;border:1px solid #555;' +
          'padding:2px 6px;cursor:pointer;border-radius:3px;font-size:11px;' +
        '" title="前进">→</button>' +
        '<button id="ram-btn-refresh" style="' +
          'background:#333;color:#fff;border:1px solid #555;' +
          'padding:2px 6px;cursor:pointer;border-radius:3px;font-size:11px;' +
        '" title="刷新">🔄</button>' +
        '<button id="ram-btn-home" style="' +
          'background:#333;color:#fff;border:1px solid #555;' +
          'padding:2px 6px;cursor:pointer;border-radius:3px;font-size:11px;' +
        '" title="B站首页">🏠</button>' +
        '<input id="ram-url-bar" type="text" style="' +
          'flex:1;min-width:0;background:#222;color:#e0e0e0;border:1px solid #555;' +
          'padding:2px 6px;border-radius:3px;font-size:11px;font-family:Consolas,monospace;' +
        '" placeholder="https://www.bilibili.com" />' +
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
        '<span style="margin-left:8px;">' +
          'Q: <select id="ram-quality-select" style="background:#2a2a4a;color:#e0e0e0;border:1px solid #444;padding:2px 4px;font-size:11px;border-radius:3px;">' +
            '<option value="">--</option>' +
          '</select>' +
        '</span>' +
        '<button id="ram-stop-btn" style="background:#8b0000;color:#e0e0e0;border:1px solid #a00;padding:2px 8px;font-size:11px;cursor:pointer;border-radius:3px;margin-left:4px;">⏹ Stop</button>' +
        '<button id="ram-play-btn" style="background:#008000;color:#fff;border:1px solid #0a0;padding:4px 14px;font-size:12px;font-weight:bold;cursor:pointer;border-radius:3px;margin-left:8px;">▶ Play in mpv</button>' +
      '</div>';
    document.body.appendChild(bar);

    statusEl = document.getElementById('ram-status');
    playinfoStatusEl = document.getElementById('ram-playinfo-status');

    // Wire up buttons
    document.getElementById('ram-btn-cookie').addEventListener('click', function () {
      var cookies = document.cookie;
      console.log('[RAM-Stream] ===== COOKIE BUTTON CLICKED =====');
      console.log('[RAM-Stream] document.cookie length:', cookies.length);
      console.log('[RAM-Stream] document.cookie preview:', cookies.substring(0, 200));
      console.log('[RAM-Stream] IPC available:', !!getInvoke());
      console.log('[RAM-Stream] About to call callRust("submit_cookies")...');

      callRust('submit_cookies', { cookies: cookies })
        .then(function (result) {
          console.log('[RAM-Stream] ✅ submit_cookies SUCCESS:', JSON.stringify(result));
          setStatus('✅ Cookie已发送 (' + (result.count || '?') + '条)', '#4f4');
        })
        .catch(function (e) {
          console.error('[RAM-Stream] ❌ submit_cookies FAILED');
          console.error('[RAM-Stream] Error type:', typeof e);
          console.error('[RAM-Stream] Error:', e);
          console.error('[RAM-Stream] Error keys:', e ? Object.keys(e) : 'null');
          console.error('[RAM-Stream] Error message:', e && e.message);
          setStatus('❌ ' + (e && e.message ? e.message : String(e)), '#f44');
    });
    });

    document.getElementById('ram-btn-playinfo').addEventListener('click', function () {
      var playinfo = window.__ram_playinfo__ || window.__playinfo__;
      console.log('[RAM-Stream] ===== PLAYINFO BUTTON CLICKED =====');
      console.log('[RAM-Stream] playinfo found:', !!playinfo);
      console.log('[RAM-Stream] IPC available:', !!getInvoke());
      if (playinfo) {
        if (!window.__ram_playinfo__ && window.__playinfo__) {
          window.__ram_playinfo__ = window.__playinfo__;
        }
        var jsonStr = JSON.stringify(playinfo);
        console.log('[RAM-Stream] 📡 Sending PlayInfo... Size:', jsonStr.length);
        console.log('[RAM-Stream] About to call callRust("submit_playinfo")...');
        callRust('submit_playinfo', { jsonStr: jsonStr })
          .then(function (result) {
            console.log('[RAM-Stream] ✅ submit_playinfo SUCCESS:', JSON.stringify(result));
            if (result.ok) {
              setStatus('✅ PlayInfo已发送', '#4f4');
            } else {
              setStatus('⚠️ 解析失败: ' + (result.error || 'unknown'), '#f90');
            }
          })
          .catch(function (e) {
            console.error('[RAM-Stream] ❌ submit_playinfo FAILED');
            console.error('[RAM-Stream] Error type:', typeof e);
            console.error('[RAM-Stream] Error:', e);
            console.error('[RAM-Stream] Error keys:', e ? Object.keys(e) : 'null');
            console.error('[RAM-Stream] Error message:', e && e.message);
            setStatus('❌ ' + (e && e.message ? e.message : String(e)), '#f44');
          });
      } else {
        // Try to re-poll
        setStatus('🔍 重新扫描 __playinfo__...', '#f90');
        console.log('[RAM-Stream] Re-scanning for __playinfo__');
        pollElapsed = 0;
        if (!playinfoPollTimer) {
          startPlayinfoPoll();
        }
      }
    });

    // ▶ Play in mpv button — stores data in window.name, navigates to bridge page
    document.getElementById('ram-play-btn').addEventListener('click', function () {
      var cookies = document.cookie;
      var playinfo = window.__playinfo__;
      console.log('[RAM-Stream] ===== ▶ PLAY BUTTON CLICKED =====');
      console.log('[RAM-Stream] Cookies length:', cookies.length);
      console.log('[RAM-Stream] Playinfo present:', !!playinfo);

      if (!cookies || cookies.length < 10) {
        setStatus('⚠️ No cookies — please log in first', '#f90');
        return;
      }
      if (!playinfo) {
        setStatus('⚠️ No playinfo — please open a video page first', '#f90');
        return;
      }

      // Pack data into window.name (persists across navigation, ~2MB limit)
      var payload = {
        cookies: cookies,
        playinfo: JSON.stringify(playinfo)
      };
      window.name = JSON.stringify(payload);
      console.log('[RAM-Stream] Data packed into window.name:', window.name.length, 'bytes');

      // Navigate to bridge page (localhost — allowed by navigation policy)
      setStatus('🚀 Launching player...', '#0ff');
      console.log('[RAM-Stream] Navigating to bridge page...');
      window.location.href = 'http://localhost:1420/bridge.html';
    });

    // Navigation buttons
    document.getElementById('ram-btn-back').addEventListener('click', function () {
      history.back();
    });
    document.getElementById('ram-btn-forward').addEventListener('click', function () {
      history.forward();
    });
    document.getElementById('ram-btn-refresh').addEventListener('click', function () {
      location.reload();
    });
    document.getElementById('ram-btn-home').addEventListener('click', function () {
      location.href = 'https://www.bilibili.com';
    });

    // Address bar
    var urlBar = document.getElementById('ram-url-bar');
    urlBar.value = location.href;
    urlBar.addEventListener('keydown', function (e) {
      if (e.key === 'Enter') {
        var val = urlBar.value.trim();
        if (val.indexOf('http') === 0 || val.indexOf('bilibili.com') !== -1) {
          location.href = val.indexOf('http') === 0 ? val : 'https://' + val;
        } else if (val) {
          location.href = 'https://search.bilibili.com/all?keyword=' + encodeURIComponent(val);
        }
      }
    });

    // Update URL bar on navigation
    setInterval(function () {
      if (urlBar && urlBar !== document.activeElement) {
        urlBar.value = location.href;
      }
    }, 500);

    // Quality selector change handler
    var qualitySelect = document.getElementById('ram-quality-select');
    if (qualitySelect) {
      qualitySelect.addEventListener('change', function() {
        var qid = parseInt(this.value);
        if (!isNaN(qid) && qid > 0) {
          console.log('[RAM-Stream] Quality changed to:', qid);
          callRust('select_quality', { quality_id: qid });
          setStatus('切换画质: q' + qid, '#f90');
        }
      });
    }

    // Stop player button handler
    var stopBtn = document.getElementById('ram-stop-btn');
    if (stopBtn) {
      stopBtn.addEventListener('click', function() {
        console.log('[RAM-Stream] Stop player requested');
        callRust('stop_player', {});
        setStatus('Player stopped', '#0f0');
      });
    }
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
    var onLocalhost = window.location.hostname === 'localhost' || window.location.hostname === '127.0.0.1';
    if (onLocalhost && getInvoke()) {
      ipcEl.textContent = '🟢 IPC';
      ipcEl.title = 'Tauri IPC connected (App origin)';
      console.log('[RAM-Stream] ✅ Tauri IPC bridge found on localhost');
      return true;
    } else {
      ipcEl.textContent = '🟡 Proto';
      ipcEl.title = 'Using ram-stream:// protocol (IPC blocked by Tauri ACL on external domains)';
      console.log('[RAM-Stream] ⚠️ IPC blocked (external domain), using ram-stream:// protocol');
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

        var dash = (window.__playinfo__ && window.__playinfo__.data && window.__playinfo__.data.dash) ? window.__playinfo__.data.dash : null;
        var desc = dash
          ? 'video:' + (dash.video?.length || 0) + ' audio:' + (dash.audio?.length || 0)
          : 'no dash';
        setPlayinfoStatus('✅ ' + desc);
        setStatus('PlayInfo已捕获', '#4f4');
        console.log('[RAM-Stream] ✅ __playinfo__ captured:', desc);

        // Auto-send to Rust
        callRust('submit_playinfo', { jsonStr: JSON.stringify(window.__playinfo__) })
          .catch(function () {});

        // Build quality selector from playinfo data
        if (window.__playinfo__) {
          try {
            buildQualitySelector(window.__playinfo__);
          } catch(e) {
            console.log('[RAM-Stream] Error building quality selector:', e);
          }
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
    var url = '';
    try {
      url = typeof args[0] === 'string' ? args[0] : (args[0] && args[0].url ? args[0].url : '');
    } catch (_) {}

    var isPlayurl = url && (
      url.indexOf('/playurl') !== -1 ||
      url.indexOf('api.bilibili.com/x/player/') !== -1 ||
      url.indexOf('api.bilibili.com/pgc/player/') !== -1
    );

    // Don't intercept our own ram-stream:// protocol calls
    var isOurProtocol = url && url.indexOf('ram-stream://') === 0;

    return origFetch.apply(this, args).then(function (response) {
      if (isPlayurl && response.ok) {
        response.clone().text().then(function (body) {
          try {
            var json = JSON.parse(body);
            callRust('submit_playurl_response', { url: url, body: body }).catch(function () {});
            if (json.data && json.data.dash) {
              window.__ram_playinfo__ = json;
              setStatus('✅ Fetch截获PlayInfo', '#4f4');
              console.log('[RAM-Stream] ✅ PlayInfo via fetch hook');
              try {
                buildQualitySelector(json);
              } catch(e) {}
            }
          } catch (_) {}
        }).catch(function () {});
      }
      return response;
    }, function (err) {
      // Pass through errors from the original fetch
      throw err;
    });
  };

  // ==================== SPA Navigation Detection ====================
  var lastUrl = location.href;
  setInterval(function () {
    var currentUrl = location.href;
    if (currentUrl !== lastUrl) {
      lastUrl = currentUrl;
      console.log('[RAM-Stream] 🔄 SPA navigation detected:', currentUrl);
      autoSendCookies();
      pollElapsed = 0;
      if (!playinfoPollTimer) {
        startPlayinfoPoll();
      }
    }
  }, 500);

  // ==================== Init ====================
  function init() {
    console.log('[RAM-Stream] ====== Preload Script v0.4.0 ======');
    console.log('[RAM-Stream] URL:', location.href);
    console.log('[RAM-Stream] __TAURI_INTERNALS__:', !!window.__TAURI_INTERNALS__);
    console.log('[RAM-Stream] __TAURI__:', !!window.__TAURI__);
    console.log('[RAM-Stream] Protocol fallback: ram-stream:// available via fetch');

    console.log('[RAM-Stream] Navigation fixes active — window.open and _blank links redirected');

    // Bridge page guard: skip operations that would overwrite real data in AppState
    var isBridgePage = (location.href.indexOf('bridge.html') !== -1);
    if (isBridgePage) {
      console.log('[RAM-Stream] 🛑 Bridge page detected — skipping auto-send, diagnostic, and playinfo poll');
    }

    // Log when the page fully loads after navigation
    window.addEventListener('load', function() {
        console.log('[RAM-Stream] 📄 Page fully loaded:', location.href);
    });

    injectToolbar();

    // Skip auto-send cookies on bridge page — it would overwrite real cookies with empty ones
    if (!isBridgePage) {
      autoSendCookies();
    }

    // Delayed IPC check — bridge may not be injected on external domains
    setTimeout(function () {
      var onLocalhost = window.location.hostname === 'localhost' || window.location.hostname === '127.0.0.1';
      if (onLocalhost && getInvoke()) {
        setStatus('IPC已连接 ✓', '#4f4');
      } else {
        setStatus('使用自定义协议通道', '#f90');
      }
      checkIPC();
    }, 100);

    // Re-check later
    setTimeout(checkIPC, 2000);

    // Comprehensive IPC/Protocol diagnostic
    // SKIP on bridge page — the test `submit_cookies({cookies:'test=1'})` would
    // overwrite real cookies the bridge page just stored via IPC.
    if (!isBridgePage) {
      setTimeout(function () {
        console.log('[RAM-Stream] ====== DIAGNOSTIC START ======');
        console.log('[RAM-Stream] location:', location.hostname, location.pathname.substring(0, 40));
        console.log('[RAM-Stream] __TAURI_INTERNALS__ present:', !!window.__TAURI_INTERNALS__, ', invoke available:', !!(window.__TAURI_INTERNALS__ && typeof window.__TAURI_INTERNALS__.invoke === 'function'));
        console.log('[RAM-Stream] __TAURI__ present:', !!window.__TAURI__);

        var invoke = getInvoke();
        if (invoke) {
          console.log('[RAM-Stream] IPC invoke function found, testing...');

          // Test 1: Invoke ping via IPC
          invoke('submit_cookies', { cookies: 'test=1' }).then(function (result) {
            console.log('[RAM-Stream] ✅ IPC invoke SUCCESS! Result:', JSON.stringify(result));
            setStatus('🟢 IPC通道正常 (' + (result.count || '?') + '条)', '#4f4');
          }).catch(function (e) {
            console.error('[RAM-Stream] ❌ IPC invoke FAILED');
            console.error('[RAM-Stream] Error type:', typeof e);
            console.error('[RAM-Stream] Error keys:', Object.keys(e || {}));
            console.error('[RAM-Stream] Error toString:', String(e));
            console.error('[RAM-Stream] Error JSON:', JSON.stringify(e));
            console.error('[RAM-Stream] Error message:', e && e.message);
            setStatus('🔴 IPC失败: ' + (e && e.message ? e.message : String(e).substring(0, 40)), '#f44');

            // Test 2: Try img beacon fallback
            console.log('[RAM-Stream] Testing img beacon fallback...');
            callRustViaProtocol('ping', {}).then(function (r) {
              console.log('[RAM-Stream] ✅ Img beacon fallback SUCCESS:', JSON.stringify(r));
              setStatus('🟢 协议通道正常(IPC不可用,用img)', '#4f4');
            }).catch(function (pe) {
              console.error('[RAM-Stream] ❌ Protocol fallback ALSO FAILED');
              console.error('[RAM-Stream] Proto error:', String(pe));
              console.error('[RAM-Stream] Proto error type:', typeof pe);
              if (pe instanceof Error) {
                console.error('[RAM-Stream] Proto error.message:', pe.message);
              }
              setStatus('🔴 双通道均失败', '#f44');
            });
          });
        } else {
          console.log('[RAM-Stream] No IPC invoke function — trying protocol only');
          setStatus('🔴 无IPC桥接', '#f44');
          callRustViaProtocol('ping', {}).then(function (r) {
            console.log('[RAM-Stream] ✅ Protocol SUCCESS:', JSON.stringify(r));
            setStatus('🟢 仅协议通道', '#4f4');
          }).catch(function (pe) {
            console.error('[RAM-Stream] ❌ Protocol FAILED');
            console.error('[RAM-Stream] Proto error type:', typeof pe);
            console.error('[RAM-Stream] Proto error:', String(pe));
            if (pe instanceof Error) {
              console.error('[RAM-Stream] Proto error.message:', pe.message);
            }
            setStatus('🔴 所有通道均失败: ' + String(pe).substring(0, 30), '#f44');
          });
        }
        console.log('[RAM-Stream] ====== DIAGNOSTIC END ======');
      }, 2000);
    }

    // Skip playinfo poll on bridge page — no __playinfo__ here
    if (!isBridgePage) {
      startPlayinfoPoll();
    }
  }

  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', init);
  } else {
    init();
  }
})();
