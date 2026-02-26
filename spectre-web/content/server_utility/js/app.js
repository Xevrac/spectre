(function () {
  'use strict';

  const state = {
    servers: [
      {
        name: 'Server 1',
        port: 22000,
        use_sabre_squadron: true,
        hd2ds_path: '',
        hd2ds_sabresquadron_path: '',
        mpmaplist_path: '',
        current_config: 'Default',
        configs: [
          {
            name: 'Default',
            domain: 'local',
            session_name: 'A Spectre Session',
            style: 'Objectives',
            max_clients: 32,
            point_limit: 0,
            round_limit: 5,
            round_count: 3,
            respawn_time: 3,
            spawn_protection: 5,
            warmup: 10,
            inverse_damage: 100,
            friendly_fire: true,
            auto_team_balance: true,
            third_person_view: false,
            allow_crosshair: true,
            falling_dmg: true,
            allow_respawn: false,
            allow_vehicles: true,
            difficulty: 'Hard',
            respawn_number: 0,
            team_respawn: true,
            password: '',
            admin_pass: '',
            max_ping: 0,
            max_freq: 50,
            max_inactivity: 0,
            voice_chat: 0,
            maps: ['Alps3'],
            ban_list: [],
            enable_whitelist: false,
            whitelist: []
          }
        ]
      }
    ],
    selectedServerIndex: 0,
    selectedConfigIndex: 0,
    activeTab: 'console',
    serverStarting: false,
    serverError: false,
    playerCount: { active: '--', total: '--' },
    currentPlayerList: [],
    playerListRevealed: {},
    server_manager: {
      enable_watchdog: true,
      restart_interval_days: 0,
      log_rotation_days: 0,
      enable_forced_ban_list: true,
      forced_ban_list: []
    }
  };

  // Cache of last player list payload to avoid unnecessary re-renders while a server is running.
  // This helps keep click interactions on the players table responsive even when IPC is polling.
  let lastPlayerListJson = null;
  // Track whether we currently have a running player poll interval attached.
  let lastPlayerPollRunning = false;

  function ipcLog(msg, detail) {
    console.log('[IPC JS] ' + msg, detail !== undefined ? detail : '');
  }
  if (typeof window !== 'undefined' && window.__spectreInitialState) {
    try {
      const initial = window.__spectreInitialState;
      if (initial.servers && Array.isArray(initial.servers) && initial.servers.length > 0) {
        state.servers = initial.servers.map(function (s) {
          return Object.assign(
            { hd2ds_path: '', hd2ds_sabresquadron_path: '' },
            s,
            { hd2ds_path: s.hd2ds_path != null ? s.hd2ds_path : '', hd2ds_sabresquadron_path: s.hd2ds_sabresquadron_path != null ? s.hd2ds_sabresquadron_path : '' }
          );
        });
        ipcLog('Initial state applied', state.servers.length + ' servers');
      } else {
        ipcLog('Initial state had no servers, keeping default');
      }
      if (typeof initial.selectedServerIndex === 'number') state.selectedServerIndex = Math.min(initial.selectedServerIndex, state.servers.length - 1);
      if (typeof initial.selectedConfigIndex === 'number') state.selectedConfigIndex = Math.min(initial.selectedConfigIndex, (state.servers[state.selectedServerIndex]?.configs?.length || 1) - 1);
      if (initial.server_manager && typeof initial.server_manager === 'object') {
        state.server_manager = Object.assign({}, state.server_manager, initial.server_manager);
      }
      delete window.__spectreInitialState;
    } catch (e) {
      console.warn('[IPC JS] Failed to apply __spectreInitialState:', e);
    }
  } else {
    ipcLog('No __spectreInitialState, using built-in default');
  }

  function getSelectedServer() {
    return state.servers[state.selectedServerIndex] || null;
  }

  function getSelectedConfig() {
    const s = getSelectedServer();
    return s && s.configs[state.selectedConfigIndex] ? s.configs[state.selectedConfigIndex] : null;
  }

  function renderServerSelect() {
    const el = document.getElementById('server-select');
    if (!el) return;
    el.innerHTML = state.servers.map((s, i) =>
      `<option value="${i}" ${i === state.selectedServerIndex ? 'selected' : ''}>${escapeHtml(s.name)}</option>`
    ).join('');
    el.onchange = function () {
      state.selectedServerIndex = parseInt(this.value, 10);
      state.selectedConfigIndex = 0;
      render();
    };
  }

  function renderProfileList() {
    const list = document.getElementById('profile-list');
    if (!list) return;
    const s = getSelectedServer();
    if (!s) {
      list.innerHTML = '<div class="empty-hint">No server selected</div>';
      return;
    }
    const currentName = s.current_config;
    list.innerHTML = s.configs.map((c, i) => {
      const isSelected = i === state.selectedConfigIndex;
      const isActive = c.name === currentName;
      return `<button type="button" class="profile-item ${isSelected ? 'selected' : ''} ${isActive ? 'active' : ''}" data-index="${i}" role="option">${escapeHtml(c.name)}</button>`;
    }).join('');
    list.querySelectorAll('.profile-item').forEach(btn => {
      btn.addEventListener('click', function () {
        state.selectedConfigIndex = parseInt(this.dataset.index, 10);
        render();
      });
    });
  }

  function renderTabs() {
    document.querySelectorAll('.tab').forEach(tab => {
      const name = tab.dataset.tab;
      tab.setAttribute('aria-selected', state.activeTab === name ? 'true' : 'false');
      tab.onclick = function () {
        state.activeTab = name;
        document.querySelectorAll('.tab-panel').forEach(p => p.classList.add('hidden'));
        const panel = document.getElementById('panel-' + name);
        if (panel) panel.classList.remove('hidden');
        document.querySelectorAll('.tab').forEach(t => t.setAttribute('aria-selected', 'false'));
        tab.setAttribute('aria-selected', 'true');
        if (name === 'logs') requestLogContent();
      };
    });
    document.querySelectorAll('.tab-panel').forEach(p => {
      p.classList.toggle('hidden', p.id !== 'panel-' + state.activeTab);
    });
  }

  function bindFormToConfig() {
    const s = getSelectedServer();
    const c = getSelectedConfig();
    if (!c) return;
    const set = (id, value) => {
      const el = document.getElementById(id);
      if (el) el.value = value != null ? value : '';
    };
    const setCheck = (id, value) => {
      const el = document.getElementById(id);
      if (el) el.checked = !!value;
    };
    set('mpmaplist-path', s ? (s.mpmaplist_path || '') : '');
    set('profile-name', c.name);
    set('session-name', c.session_name);
    set('style-select', c.style);
    if (c.domain === 'LAN') c.domain = 'local';
    const domain = (c.domain === 'local' || c.domain === 'Internet') ? c.domain : 'local';
    const domainEl = document.getElementById('domain-' + domain.toLowerCase());
    if (domainEl) domainEl.checked = true;
    set('max-clients', Math.min(Math.max(c.max_clients || 32, 1), 32));
    set('point-limit', c.point_limit);
    set('round-limit', c.round_limit);
    set('round-count', c.round_count != null ? c.round_count : 1);
    set('respawn-time', c.respawn_time);
    set('spawn-protection', c.spawn_protection != null ? c.spawn_protection : 0);
    set('warmup', c.warmup != null ? c.warmup : 10);
    set('inverse-damage', c.inverse_damage != null ? c.inverse_damage : 0);
    setCheck('friendly-fire', c.friendly_fire);
    setCheck('auto-team-balance', c.auto_team_balance != null ? c.auto_team_balance : true);
    setCheck('third-person-view', c.third_person_view);
    setCheck('allow-crosshair', c.allow_crosshair != null ? c.allow_crosshair : true);
    setCheck('falling-dmg', c.falling_dmg != null ? c.falling_dmg : true);
    setCheck('allow-respawn', c.allow_respawn != null ? c.allow_respawn : false);
    setCheck('allow-vehicles', c.allow_vehicles != null ? c.allow_vehicles : true);
    set('difficulty', c.difficulty);
    set('respawn-number', c.respawn_number != null ? c.respawn_number : 0);
    setCheck('team-respawn', c.team_respawn != null ? c.team_respawn : true);
    set('password', c.password);
    set('admin-pass', c.admin_pass);
    set('max-ping', c.max_ping);
    set('max-freq', c.max_freq != null ? c.max_freq : 50);
    set('max-inactivity', c.max_inactivity != null ? c.max_inactivity : 0);
    set('voice-chat', c.voice_chat != null ? String(c.voice_chat) : '0');
    var sm = state.server_manager;
    if (sm) {
      setCheck('watchdog-restart-on-crash', sm.enable_watchdog != null ? sm.enable_watchdog : true);
      set('watchdog-restart-days', sm.restart_interval_days != null ? sm.restart_interval_days : 0);
      set('log-rotation-days', sm.log_rotation_days != null ? sm.log_rotation_days : 0);
    }
    setCheck('enable-whitelist', c.enable_whitelist != null ? c.enable_whitelist : false);
  }

  function bindConfigToForm() {
    const s = getSelectedServer();
    const c = getSelectedConfig();
    if (!c) return;
    const get = (id) => {
      const el = document.getElementById(id);
      return el ? el.value : '';
    };
    const getCheck = (id) => {
      const el = document.getElementById(id);
      return el ? el.checked : false;
    };
    if (s) s.mpmaplist_path = (get('mpmaplist-path') || '').trim();
    const name = get('profile-name').trim();
    if (name) c.name = name;
    c.session_name = get('session-name');
    c.style = get('style-select');
    const domainRadio = document.querySelector('input[name="domain-type"]:checked');
    c.domain = domainRadio ? domainRadio.value : 'local';
    c.max_clients = Math.min(Math.max(parseInt(get('max-clients'), 10) || 32, 1), 32);
    c.point_limit = parseInt(get('point-limit'), 10) || 0;
    c.round_limit = parseInt(get('round-limit'), 10) || 5;
    c.round_count = parseInt(get('round-count'), 10);
    if (isNaN(c.round_count) || c.round_count < 1) c.round_count = 1;
    if (c.round_count > 20) c.round_count = 20;
    c.respawn_time = parseInt(get('respawn-time'), 10) || 3;
    c.spawn_protection = parseInt(get('spawn-protection'), 10) || 5;
    if (c.spawn_protection > 30) c.spawn_protection = 30;
    c.warmup = parseInt(get('warmup'), 10) || 0;
    if (c.warmup > 60) c.warmup = 60;
    c.inverse_damage = parseInt(get('inverse-damage'), 10);
    if (isNaN(c.inverse_damage)) c.inverse_damage = 100;
    if (c.inverse_damage > 200) c.inverse_damage = 200;
    c.friendly_fire = getCheck('friendly-fire');
    c.auto_team_balance = getCheck('auto-team-balance');
    c.third_person_view = getCheck('third-person-view');
    c.allow_crosshair = getCheck('allow-crosshair');
    c.falling_dmg = getCheck('falling-dmg');
    c.allow_respawn = getCheck('allow-respawn');
    c.allow_vehicles = getCheck('allow-vehicles');
    c.difficulty = get('difficulty');
    c.respawn_number = parseInt(get('respawn-number'), 10);
    if (isNaN(c.respawn_number) || c.respawn_number < 0) c.respawn_number = 0;
    if (c.respawn_number > 99) c.respawn_number = 99;
    c.team_respawn = getCheck('team-respawn');
    c.password = get('password');
    c.admin_pass = get('admin-pass');
    c.max_ping = parseInt(get('max-ping'), 10) || 0;
    c.max_freq = parseInt(get('max-freq'), 10);
    if (isNaN(c.max_freq)) c.max_freq = 50;
    c.max_inactivity = parseInt(get('max-inactivity'), 10) || 0;
    c.voice_chat = parseInt(get('voice-chat'), 10) || 0;
    if (c.voice_chat > 6) c.voice_chat = 6;
    if (s) s.current_config = c.name;
    if (!c.ban_list) c.ban_list = [];
    if (!c.whitelist) c.whitelist = [];
    c.enable_whitelist = document.getElementById('enable-whitelist') ? document.getElementById('enable-whitelist').checked : (c.enable_whitelist != null ? c.enable_whitelist : false);
    var sm = state.server_manager;
    if (sm) {
      sm.enable_watchdog = document.getElementById('watchdog-restart-on-crash') ? document.getElementById('watchdog-restart-on-crash').checked : sm.enable_watchdog;
      var daysEl = document.getElementById('watchdog-restart-days');
      if (daysEl) {
        var d = parseInt(daysEl.value, 10);
        sm.restart_interval_days = (isNaN(d) || d < 0) ? 0 : Math.min(365, d);
      }
      var logDaysEl = document.getElementById('log-rotation-days');
      if (logDaysEl) {
        var ld = parseInt(logDaysEl.value, 10);
        sm.log_rotation_days = (isNaN(ld) || ld < 0) ? 0 : Math.min(365, ld);
      }
    }
  }

  let selectedMapIndex = -1;
  let selectedAvailableMapIndex = -1;
  let selectedBanIndex = -1;
  let selectedWhitelistIndex = -1;
  let unsavedChanges = false;
  let unsavedPollInterval = null;
  const UNSAVED_POLL_MS = 400;
  let autoSaveTimeout = null;
  const AUTO_SAVE_DELAY_MS = 800;

  function setUnsaved(value) {
    unsavedChanges = !!value;
    if (autoSaveTimeout !== null) {
      clearTimeout(autoSaveTimeout);
      autoSaveTimeout = null;
    }
    if (unsavedChanges) {
      autoSaveTimeout = setTimeout(function () {
        autoSaveTimeout = null;
        performSave();
      }, AUTO_SAVE_DELAY_MS);
    }
    if (unsavedPollInterval !== null) {
      clearInterval(unsavedPollInterval);
      unsavedPollInterval = null;
    }
    if (unsavedChanges) {
      unsavedPollInterval = setInterval(function () {
        var el = document.getElementById('unsaved-indicator');
        if (el) el.classList.toggle('visible', true);
      }, UNSAVED_POLL_MS);
    }
    var el = document.getElementById('unsaved-indicator');
    if (el) el.classList.toggle('visible', unsavedChanges);
    if (!unsavedChanges && el) {
      el.classList.remove('visible');
      var parent = el.parentElement;
      requestAnimationFrame(function () {
        if (parent) void parent.offsetHeight;
        void el.offsetHeight;
        requestAnimationFrame(function () {
          if (parent) void parent.offsetHeight;
          void el.offsetHeight;
        });
      });
    }
  }

  function performSave() {
    bindConfigToForm();
    ensureCurrentConfigs();
    const payload = JSON.stringify({ action: 'save', servers: state.servers, server_manager: state.server_manager });
    ipcLog('Auto-save', 'ipc.postMessage body=' + payload.length + ' bytes');
    if (typeof window.ipc !== 'undefined' && window.ipc.postMessage) {
      try {
        window.ipc.postMessage(payload);
        showMessage('Saving..');
      } catch (err) {
        ipcLog('Save postMessage error', err);
        showMessage('Save failed.', true);
        if (window.__spectreIpcStatus) window.__spectreIpcStatus('Save error: ' + (err && err.message ? err.message : 'postMessage'));
      }
    } else {
      ipcLog('Save skipped', 'window.ipc.postMessage not available');
      showMessage('Save not available.', true);
      if (window.__spectreIpcStatus) window.__spectreIpcStatus('IPC not available');
    }
  }

  function requestRunningState() {
    if (typeof window.ipc !== 'undefined' && window.ipc.postMessage) {
      try {
        window.ipc.postMessage(JSON.stringify({ action: 'get_running', servers: state.servers }));
      } catch (e) { /* ignore */ }
    }
  }

  function requestLogContent() {
    if (typeof window.ipc !== 'undefined' && window.ipc.postMessage) {
      try {
        window.ipc.postMessage(JSON.stringify({ action: 'get_log_content', servers: state.servers }));
      } catch (e) { /* ignore */ }
    }
  }

  function openLogFile() {
    if (typeof window.ipc !== 'undefined' && window.ipc.postMessage) {
      try {
        window.ipc.postMessage(JSON.stringify({ action: 'open_log_file', servers: state.servers }));
      } catch (e) { /* ignore */ }
    }
  }

  function ensureCurrentConfigs() {
    state.servers.forEach(function (s) {
      if (!s.configs || !s.configs.length) return;
      var found = s.configs.some(function (c) { return c && c.name === s.current_config; });
      if (!found) s.current_config = s.configs[0].name;
    });
  }

  function getAvailableMapsForServer() {
    const s = getSelectedServer();
    const maps = (s && s.available_maps_by_style) ? s.available_maps_by_style : {};
    return typeof maps === 'object' ? maps : {};
  }

  function getPoolForCurrentStyle() {
    const c = getSelectedConfig();
    const style = c ? (c.style || 'Occupation') : 'Occupation';
    const byStyle = getAvailableMapsForServer();
    let arr = byStyle[style];
    if (!Array.isArray(arr) || arr.length === 0) {
      if (style === 'Deathmatch') arr = byStyle['Occupation'];
      else if (style === 'Objectives') arr = byStyle['Objectives'];
    }
    return Array.isArray(arr) ? arr : [];
  }

  function getAvailableMapsForRotation() {
    const c = getSelectedConfig();
    const rotation = c && c.maps ? c.maps : [];
    const pool = getPoolForCurrentStyle();
    return pool.filter(function (name) { return rotation.indexOf(name) === -1; });
  }

  function renderAvailableMapList() {
    const ul = document.getElementById('available-map-list');
    if (!ul) return;
    const available = getAvailableMapsForRotation();
    if (selectedAvailableMapIndex >= available.length) selectedAvailableMapIndex = -1;
    if (available.length === 0) {
      ul.innerHTML = '<li class="empty-hint">No maps to add. Set this server\'s maplist path below and save, then reopen to load maps.</li>';
      selectedAvailableMapIndex = -1;
      return;
    }
    ul.innerHTML = available.map(function (name, i) {
      return '<li class="' + (i === selectedAvailableMapIndex ? 'selected' : '') + '" data-index="' + i + '">' + escapeHtml(name) + '</li>';
    }).join('');
  }

  function renderMapList() {
    const ul = document.getElementById('map-list');
    if (!ul) return;
    const c = getSelectedConfig();
    const maps = c ? (c.maps || []) : [];
    if (selectedMapIndex >= maps.length) selectedMapIndex = -1;
    if (maps.length === 0) {
      ul.innerHTML = '<li class="empty-hint">No maps in rotation. Add from available list.</li>';
      selectedMapIndex = -1;
      return;
    }
    ul.innerHTML = maps.map((m, i) =>
      `<li class="${i === selectedMapIndex ? 'selected' : ''}" data-index="${i}">${escapeHtml(m)}</li>`
    ).join('');
  }

  function renderBanList() {
    const ul = document.getElementById('ban-list');
    if (!ul) return;
    const c = getSelectedConfig();
    const list = c ? (c.ban_list || []) : [];
    if (selectedBanIndex >= list.length) selectedBanIndex = -1;
    if (list.length === 0) {
      ul.innerHTML = '<li class="empty-hint">No entries. Add an IP and optional comment above.</li>';
      selectedBanIndex = -1;
      return;
    }
    ul.innerHTML = list.map(function (entry, i) {
      return '<li class="' + (i === selectedBanIndex ? 'selected' : '') + '" data-index="' + i + '">' + escapeHtml(entry) + '</li>';
    }).join('');
  }

  function renderWhitelist() {
    const ul = document.getElementById('whitelist-list');
    if (!ul) return;
    const c = getSelectedConfig();
    const list = c ? (c.whitelist || []) : [];
    if (selectedWhitelistIndex >= list.length) selectedWhitelistIndex = -1;
    if (list.length === 0) {
      ul.innerHTML = '<li class="empty-hint">No entries. Add an IP and optional comment above.</li>';
      selectedWhitelistIndex = -1;
      return;
    }
    ul.innerHTML = list.map(function (entry, i) {
      return '<li class="' + (i === selectedWhitelistIndex ? 'selected' : '') + '" data-index="' + i + '">' + escapeHtml(entry) + '</li>';
    }).join('');
  }

  function renderCurrentPlayersTable() {
    const tbody = document.getElementById('current-players-tbody');
    if (!tbody) return;
    const list = state.currentPlayerList || [];
    const revealed = state.playerListRevealed || {};
    if (list.length === 0) {
      tbody.innerHTML = '<tr><td colspan="3" class="empty-hint">No players. Server may be stopped or no one connected.</td></tr>';
      return;
    }
    tbody.innerHTML = list.map(function (p, i) {
      var name = (p && p.name != null) ? String(p.name) : '';
      var ip = (p && p.ip != null) ? String(p.ip) : '';
      var isRevealed = revealed[i];
      var ipDisplay = isRevealed ? escapeHtml(ip) : 'Click to reveal';
      var ipClass = 'player-ip-cell' + (isRevealed ? ' revealed' : '');
      var dataAttrs = isRevealed ? '' : ' data-index="' + i + '" data-ip="' + escapeHtml(ip) + '"';
      var ipInner = '<span class="spoiler-tag">' + ipDisplay + '</span>';
      var banBtn = '<button type="button" class="btn btn-sm btn-ban-row" data-ip="' + escapeHtml(ip) + '" title="Add IP to ban list">Ban</button>';
      return '<tr><td>' + escapeHtml(name) + '</td><td class="' + ipClass + '"' + dataAttrs + '>' + ipInner + '</td><td class="player-ban-cell">' + banBtn + '</td></tr>';
    }).join('');
  }

  function countRunning() {
    return state.servers.filter(function (s) { return s.running; }).length;
  }

  // Coalesce frequent render calls (especially from IPC/status updates) into animation frames
  let renderRequested = false;
  let lastStartServerRunning = null;
  let lastRunCount = null;
  let lastStatus = null;
  let lastStatusText = null;
  let lastPlayersText = null;
  let lastStopAllDisplay = null;
  function requestRender() {
    if (renderRequested) return;
    renderRequested = true;
    requestAnimationFrame(function () {
      renderRequested = false;
      render();
    });
  }

  function render() {
    const s = getSelectedServer();
    const c = getSelectedConfig();
    var dupNotice = document.getElementById('duplicate-hd2-notice');
    if (dupNotice && dupNotice.closest('.sidebar')) dupNotice.remove();
    renderServerSelect();
    renderProfileList();
    renderTabs();
    bindFormToConfig();
    renderAvailableMapList();
    renderMapList();
    renderBanList();
    renderWhitelist();
    renderCurrentPlayersTable();
    const gameSelect = document.getElementById('game-select');
    if (gameSelect && s) gameSelect.value = s.use_sabre_squadron ? 'sabre' : 'hd2';
    var runCount = countRunning();
    var startServerBtn = document.getElementById('start-server');
    if (startServerBtn && s) {
      var isRunning = !!s.running;
      if (lastStartServerRunning !== isRunning) {
        lastStartServerRunning = isRunning;
        startServerBtn.textContent = isRunning ? 'Stop Server' : 'Start Server';
        startServerBtn.className = isRunning ? 'btn btn-start btn-stop' : 'btn btn-start';
      }
    }
    var startAllBtn = document.getElementById('start-all-servers');
    if (startAllBtn) {
      if (lastRunCount !== runCount) {
        lastRunCount = runCount;
        startAllBtn.textContent = runCount > 0 ? 'Stop All Servers' : 'Start All Servers';
        startAllBtn.className = runCount > 0 ? 'btn btn-sm btn-stop' : 'btn btn-sm';
      }
    }
    var stopAllBtn = document.getElementById('stop-all-servers');
    if (stopAllBtn) {
      var stopAllDisplay = runCount > 1 ? '' : 'none';
      if (lastStopAllDisplay !== stopAllDisplay) {
        lastStopAllDisplay = stopAllDisplay;
        stopAllBtn.style.display = stopAllDisplay;
      }
    }
    var statusDot = document.getElementById('server-status-dot');
    var statusText = document.getElementById('server-status-text');
    var playersEl = document.getElementById('server-status-players');
    if (statusDot && statusText) {
      var status = 'stopped';
      if (state.serverError) status = 'error';
      else if (state.serverStarting) status = 'starting';
      else if (s && s.running) status = 'online';
      if (lastStatus !== status) {
        lastStatus = status;
        statusDot.className = 'server-status-dot status-' + status;
        statusText.textContent = status === 'online' ? 'Online' : status === 'starting' ? 'Starting' : status === 'error' ? 'Error' : 'Stopped';
      }
      if (playersEl) {
        var playersText = status === 'online'
          ? (state.playerCount.active + ' / ' + state.playerCount.total)
          : '-- / --';
        if (lastPlayersText !== playersText) {
          lastPlayersText = playersText;
          playersEl.textContent = playersText;
        }
      }
    }
    if (playersEl && !statusDot && lastPlayersText !== '-- / --') {
      lastPlayersText = '-- / --';
      playersEl.textContent = '-- / --';
    }

    // Manage player polling interval only when the running state changes,
    // instead of on every render, to avoid flooding IPC and starving UI events.
    var isRunningFlag = !!(s && s.running);
    if (typeof window.ipc !== 'undefined' && window.ipc.postMessage) {
      if (isRunningFlag && !lastPlayerPollRunning) {
        function requestPlayers() {
          try {
            window.ipc.postMessage(JSON.stringify({
              action: 'get_players',
              server_index: state.selectedServerIndex,
              servers: state.servers
            }));
          } catch (e) {}
        }
        requestPlayers();
        window._playerPollTimer = setInterval(requestPlayers, 2000);
        lastPlayerPollRunning = true;
      } else if (!isRunningFlag && lastPlayerPollRunning) {
        if (typeof window._playerPollTimer !== 'undefined' && window._playerPollTimer !== null) {
          clearInterval(window._playerPollTimer);
          window._playerPollTimer = null;
        }
        lastPlayerPollRunning = false;
      }
    } else {
      // No IPC – ensure no stray interval is left running.
      if (typeof window._playerPollTimer !== 'undefined' && window._playerPollTimer !== null) {
        clearInterval(window._playerPollTimer);
        window._playerPollTimer = null;
      }
      lastPlayerPollRunning = false;
    }
  }

  function escapeHtml(s) {
    const div = document.createElement('div');
    div.textContent = s;
    return div.innerHTML;
  }

  // Game select
  const gameSelect = document.getElementById('game-select');
  if (gameSelect) {
    gameSelect.onchange = function () {
      const s = getSelectedServer();
      if (s) s.use_sabre_squadron = this.value === 'sabre';
    };
  }

  const profileNameEl = document.getElementById('profile-name');
  if (profileNameEl) {
    profileNameEl.addEventListener('input', function () {
      bindConfigToForm();
      renderProfileList();
      const s = getSelectedServer();
      const c = getSelectedConfig();
      if (s && c) s.name = c.name;
      renderServerSelect();
    });
  }

  document.getElementById('profile-new')?.addEventListener('click', function () {
    const s = getSelectedServer();
    ipcLog('Profile New clicked', s ? 'server=' + s.name : 'no server');
    if (s) {
      const last = s.configs[s.configs.length - 1];
      const base = last ? { ...last } : { name: 'New profile', domain: 'local', session_name: 'A Spectre Session', style: 'Objectives', max_clients: 32, point_limit: 0, round_limit: 5, round_count: 3, respawn_time: 3, spawn_protection: 5, warmup: 10, inverse_damage: 100, friendly_fire: true, auto_team_balance: true, third_person_view: false, allow_crosshair: true, falling_dmg: true, allow_respawn: false, allow_vehicles: true, difficulty: 'Hard', respawn_number: 0, team_respawn: true, password: '', admin_pass: '', max_ping: 0, max_freq: 50, max_inactivity: 0, voice_chat: 0, maps: ['Alps3'] };
      s.configs.push({
        ...base,
        name: 'New profile',
        maps: Array.isArray(base.maps) ? base.maps.slice() : [],
        ban_list: Array.isArray(base.ban_list) ? base.ban_list.slice() : [],
        whitelist: Array.isArray(base.whitelist) ? base.whitelist.slice() : [],
        enable_whitelist: base.enable_whitelist != null ? base.enable_whitelist : false
      });
      state.selectedConfigIndex = s.configs.length - 1;
      setUnsaved(true);
      render();
      ipcLog('Profile added', 'total configs=' + s.configs.length);
    }
  });
  document.getElementById('profile-delete')?.addEventListener('click', function () {
    const s = getSelectedServer();
    if (s && s.configs.length > 1) {
      s.configs.splice(state.selectedConfigIndex, 1);
      state.selectedConfigIndex = Math.min(state.selectedConfigIndex, s.configs.length - 1);
      setUnsaved(true);
      render();
    }
  });
  document.getElementById('profile-duplicate')?.addEventListener('click', function () {
    const s = getSelectedServer();
    const c = getSelectedConfig();
    if (s && c) {
      const copy = {
        ...c,
        name: c.name + ' (copy)',
        maps: Array.isArray(c.maps) ? c.maps.slice() : [],
        ban_list: Array.isArray(c.ban_list) ? c.ban_list.slice() : [],
        whitelist: Array.isArray(c.whitelist) ? c.whitelist.slice() : []
      };
      s.configs.splice(state.selectedConfigIndex + 1, 0, copy);
      state.selectedConfigIndex++;
      setUnsaved(true);
      render();
    }
  });

  document.getElementById('server-add')?.addEventListener('click', function () {
    bindConfigToForm();
    const ports = state.servers.map(function (s) { return s.port || 22000; });
    const nextPort = ports.length ? Math.max.apply(null, ports) + 1 : 22000;
    const newServer = {
      name: 'New profile',
      running: false,
      watchdog: false,
      messages: false,
      users: [],
      port: nextPort,
      use_sabre_squadron: true,
      hd2ds_path: '',
      hd2ds_sabresquadron_path: '',
      mpmaplist_path: '',
      current_config: 'Default',
        configs: [
        { name: 'Default', domain: 'local', session_name: 'A Spectre Session', style: 'Objectives', max_clients: 32, point_limit: 0, round_limit: 5, round_count: 3, respawn_time: 3, spawn_protection: 5, warmup: 10, inverse_damage: 100, friendly_fire: true, auto_team_balance: true, third_person_view: false, allow_crosshair: true, falling_dmg: true, allow_respawn: false, allow_vehicles: true, difficulty: 'Hard', respawn_number: 0, team_respawn: true, password: '', admin_pass: '', max_ping: 0, max_freq: 50, max_inactivity: 0, voice_chat: 0, maps: ['Alps3'], ban_list: [], enable_whitelist: false, whitelist: [] }
      ]
    };
    state.servers.push(newServer);
    state.selectedServerIndex = state.servers.length - 1;
    state.selectedConfigIndex = 0;
    setUnsaved(true);
    render();
    ipcLog('Server added', 'total servers=' + state.servers.length);
  });

  document.getElementById('server-delete')?.addEventListener('click', function () {
    if (state.servers.length <= 1) return;
    bindConfigToForm();
    var idx = state.selectedServerIndex;
    state.servers.splice(idx, 1);
    state.selectedServerIndex = Math.min(idx, state.servers.length - 1);
    if (state.selectedServerIndex < 0) state.selectedServerIndex = 0;
    state.selectedConfigIndex = 0;
    setUnsaved(true);
    render();
    ipcLog('Server deleted', 'remaining=' + state.servers.length);
  });

  function getEffectiveHd2Dir(server) {
    var exePath = server.use_sabre_squadron ? (server.hd2ds_sabresquadron_path || '') : (server.hd2ds_path || '');
    exePath = (exePath || '').trim();
    if (!exePath) return '';
    var last = Math.max(exePath.lastIndexOf('\\'), exePath.lastIndexOf('/'));
    if (last <= 0) return '';
    var dir = exePath.slice(0, last).replace(/\\/g, '/').toLowerCase();
    return dir;
  }

  function getPathFileName(path) {
    var p = (path || '').trim().replace(/\\/g, '/');
    var last = p.lastIndexOf('/');
    return (last < 0 ? p : p.slice(last + 1)).toLowerCase();
  }

  function validateHd2dsPath(path) {
    var p = (path || '').trim();
    if (!p) return { valid: true };
    var name = getPathFileName(p);
    if (name !== 'hd2ds.exe') return { valid: false, msg: 'HD2DS.exe location must point to a file named HD2DS.exe' };
    return { valid: true };
  }

  function validateSabrePath(path) {
    var p = (path || '').trim();
    if (!p) return { valid: true };
    var name = getPathFileName(p);
    if (name !== 'hd2ds_sabresquadron.exe') return { valid: false, msg: 'HD2DS_SabreSquadron.exe location must point to a file named HD2DS_SabreSquadron.exe' };
    return { valid: true };
  }

  function getEditServerValidationErrors() {
    var hd2dsEl = document.getElementById('edit-server-hd2ds-path');
    var sabreEl = document.getElementById('edit-server-sabre-path');
    var h = hd2dsEl ? (hd2dsEl.value || '').trim() : '';
    var s = sabreEl ? (sabreEl.value || '').trim() : '';
    var errs = [];
    var r1 = validateHd2dsPath(h);
    if (!r1.valid) errs.push(r1.msg);
    var r2 = validateSabrePath(s);
    if (!r2.valid) errs.push(r2.msg);
    return errs;
  }

  function getDuplicateHd2Pairs() {
    var dirToServers = {};
    state.servers.forEach(function (sv, i) {
      var d = getEffectiveHd2Dir(sv);
      if (!d) return;
      if (!dirToServers[d]) dirToServers[d] = [];
      dirToServers[d].push({ index: i, name: sv.name || ('Server ' + (i + 1)) });
    });
    var pairs = [];
    Object.keys(dirToServers).forEach(function (dir) {
      var list = dirToServers[dir];
      if (list.length > 1) pairs.push({ dir: dir, servers: list });
    });
    return pairs;
  }

  document.getElementById('edit-server')?.addEventListener('click', function () {
    const s = getSelectedServer();
    if (!s) return;
    const portEl = document.getElementById('edit-server-port');
    const hd2dsEl = document.getElementById('edit-server-hd2ds-path');
    const sabreEl = document.getElementById('edit-server-sabre-path');
    const warnRow = document.getElementById('edit-server-duplicate-warning');
    const dialog = document.getElementById('edit-server-dialog');
    if (portEl && dialog) {
      portEl.value = s.port || 22000;
      if (hd2dsEl) hd2dsEl.value = s.hd2ds_path != null ? s.hd2ds_path : '';
      if (sabreEl) sabreEl.value = s.hd2ds_sabresquadron_path != null ? s.hd2ds_sabresquadron_path : '';
      var pairs = getDuplicateHd2Pairs();
      var msg = '';
      var myDir = getEffectiveHd2Dir(s);
      if (myDir) {
        var pair = pairs.filter(function (p) { return p.dir === myDir; })[0];
        if (pair && pair.servers.length > 1) {
          var names = pair.servers.map(function (x) { return x.name; });
          msg = 'Shared game detected, the below servers are using the same directory: ' + names.join(', ') + '. This instance may have mods or other changes; ensure this is intended.';
        }
      }
      if (warnRow) {
        var p = warnRow.querySelector('.form-notice-warning');
        if (p) {
          p.textContent = msg || '';
          warnRow.style.display = msg ? 'block' : 'none';
        }
      }
      var validationRow = document.getElementById('edit-server-validation');
      var validationErrs = getEditServerValidationErrors();
      if (validationRow) {
        var vp = validationRow.querySelector('.form-notice-warning');
        if (vp) {
          vp.textContent = validationErrs.length ? validationErrs.join(' ') : '';
          validationRow.style.display = validationErrs.length ? 'block' : 'none';
        }
      }
      dialog.showModal();
    }
  });

  document.getElementById('edit-server-dialog')?.addEventListener('submit', function (e) {
    e.preventDefault();
    const s = getSelectedServer();
    const portEl = document.getElementById('edit-server-port');
    const hd2dsEl = document.getElementById('edit-server-hd2ds-path');
    const sabreEl = document.getElementById('edit-server-sabre-path');
    const dialog = document.getElementById('edit-server-dialog');
    const validationRow = document.getElementById('edit-server-validation');
    if (!s || !portEl || !dialog) return;
    const port = parseInt(portEl.value, 10);
    if (port < 1 || port > 65535) return;
    const otherHasPort = state.servers.some(function (sv, i) {
      return i !== state.selectedServerIndex && (sv.port || 22000) === port;
    });
    if (otherHasPort) {
      showMessage('Another server already uses that port.', true);
      return;
    }
    var validationErrs = getEditServerValidationErrors();
    if (validationErrs.length > 0) {
      if (validationRow) {
        var vp = validationRow.querySelector('.form-notice-warning');
        if (vp) {
          vp.textContent = validationErrs.join(' ');
          validationRow.style.display = 'block';
        }
      }
      showMessage(validationErrs.join(' '), true);
      return;
    }
    if (validationRow) validationRow.style.display = 'none';
    s.port = port;
    s.hd2ds_path = hd2dsEl ? (hd2dsEl.value || '').trim() : (s.hd2ds_path || '');
    s.hd2ds_sabresquadron_path = sabreEl ? (sabreEl.value || '').trim() : (s.hd2ds_sabresquadron_path || '');
    setUnsaved(true);
    dialog.close();
    renderServerSelect();
  });

  document.getElementById('edit-server-cancel')?.addEventListener('click', function () {
    document.getElementById('edit-server-dialog')?.close();
  });

  document.getElementById('edit-server-browse-hd2ds')?.addEventListener('click', function () {
    if (typeof window.ipc !== 'undefined' && window.ipc.postMessage) {
      try {
        window.ipc.postMessage(JSON.stringify({ action: 'browse_hd2_dir', browse_which: 'hd2ds', servers: state.servers }));
      } catch (err) { ipcLog('Browse HD2DS folder error', err); }
    }
  });
  document.getElementById('edit-server-browse-sabre')?.addEventListener('click', function () {
    if (typeof window.ipc !== 'undefined' && window.ipc.postMessage) {
      try {
        window.ipc.postMessage(JSON.stringify({ action: 'browse_hd2_dir', browse_which: 'sabre', servers: state.servers }));
      } catch (err) { ipcLog('Browse Sabre folder error', err); }
    }
  });

  document.getElementById('save-config')?.addEventListener('click', function () {
    if (autoSaveTimeout !== null) {
      clearTimeout(autoSaveTimeout);
      autoSaveTimeout = null;
    }
    performSave();
  });

  function showMessage(msg, isErrorOrType) {
    var el = document.getElementById('message-banner');
    if (!el) return;
    el.textContent = msg || '';
    var suffix = isErrorOrType === true ? ' error' : (isErrorOrType === 'warning' ? ' warning' : (msg ? ' success' : ''));
    el.className = 'message-banner' + suffix;
    if (msg) setTimeout(function () { el.textContent = ''; el.className = 'message-banner'; }, 3000);
  }

  document.getElementById('start-server')?.addEventListener('click', function () {
    bindConfigToForm();
    ensureCurrentConfigs();
    var s = getSelectedServer();
    if (!s || typeof window.ipc === 'undefined' || !window.ipc.postMessage) {
      if (s) ipcLog('Start/Stop Server (bridge not wired)');
      return;
    }
    try {
      if (s.running) {
        window.ipc.postMessage(JSON.stringify({
          action: 'stop',
          server_index: state.selectedServerIndex,
          servers: state.servers
        }));
        ipcLog('Stop Server', state.selectedServerIndex);
      } else {
        var path = s.use_sabre_squadron ? (s.hd2ds_sabresquadron_path || '') : (s.hd2ds_path || '');
        if (!path.trim()) {
          state.serverStarting = false;
          state.serverError = false;
          showMessage('Failed to start - missing executable path!', true);
          render();
          return;
        }
        var mpmaplist = (s.mpmaplist_path || '').trim();
        if (!mpmaplist) {
          state.serverStarting = false;
          state.serverError = false;
          showMessage('Failed to start - set the maplist path in server settings first.', true);
          render();
          return;
        }
        var c = getSelectedConfig();
        if (c && (!c.maps || c.maps.length === 0)) {
          showMessage('No maps in rotation – add maps in General before starting.', 'warning');
        }
        state.serverError = false;
        state.serverStarting = true;
        render();
        window.ipc.postMessage(JSON.stringify({
          action: 'start',
          server_index: state.selectedServerIndex,
          servers: state.servers
        }));
        ipcLog('Start Server', state.selectedServerIndex);
      }
    } catch (err) { ipcLog('Start/Stop Server postMessage error', err); }
  });

  document.getElementById('start-all-servers')?.addEventListener('click', function () {
    bindConfigToForm();
    ensureCurrentConfigs();
    if (typeof window.ipc === 'undefined' || !window.ipc.postMessage) {
      ipcLog('Start/Stop All (bridge not wired)');
      return;
    }
    try {
      if (countRunning() > 0) {
        window.ipc.postMessage(JSON.stringify({ action: 'stop_all', servers: state.servers }));
        ipcLog('Stop All Servers');
      } else {
        window.ipc.postMessage(JSON.stringify({ action: 'start_all', servers: state.servers }));
        ipcLog('Start All Servers', state.servers.length);
      }
    } catch (err) { ipcLog('Start/Stop All postMessage error', err); }
  });

  document.getElementById('stop-all-servers')?.addEventListener('click', function () {
    bindConfigToForm();
    if (typeof window.ipc !== 'undefined' && window.ipc.postMessage) {
      try {
        window.ipc.postMessage(JSON.stringify({ action: 'stop_all', servers: state.servers }));
        ipcLog('Stop All Servers');
      } catch (err) { ipcLog('Stop All postMessage error', err); }
    }
  });

  document.getElementById('map-add')?.addEventListener('click', function () {
    const c = getSelectedConfig();
    if (!c) return;
    const available = getAvailableMapsForRotation();
    if (selectedAvailableMapIndex < 0 || selectedAvailableMapIndex >= available.length) return;
    const name = available[selectedAvailableMapIndex];
    c.maps = c.maps || [];
    c.maps.push(name);
    selectedMapIndex = c.maps.length - 1;
    selectedAvailableMapIndex = -1;
    setUnsaved(true);
    renderAvailableMapList();
    renderMapList();
  });
  document.getElementById('map-remove')?.addEventListener('click', function () {
    const c = getSelectedConfig();
    if (!c || !c.maps || selectedMapIndex < 0 || selectedMapIndex >= c.maps.length) return;
    c.maps.splice(selectedMapIndex, 1);
    selectedMapIndex = Math.min(selectedMapIndex, c.maps.length - 1);
    if (c.maps.length === 0) selectedMapIndex = -1;
    setUnsaved(true);
    renderAvailableMapList();
    renderMapList();
  });
  document.getElementById('map-up')?.addEventListener('click', function () {
    const c = getSelectedConfig();
    if (!c || !c.maps || selectedMapIndex <= 0) return;
    const arr = c.maps;
    [arr[selectedMapIndex - 1], arr[selectedMapIndex]] = [arr[selectedMapIndex], arr[selectedMapIndex - 1]];
    selectedMapIndex--;
    setUnsaved(true);
    renderMapList();
  });
  document.getElementById('map-down')?.addEventListener('click', function () {
    const c = getSelectedConfig();
    if (!c || !c.maps || selectedMapIndex < 0 || selectedMapIndex >= c.maps.length - 1) return;
    const arr = c.maps;
    [arr[selectedMapIndex], arr[selectedMapIndex + 1]] = [arr[selectedMapIndex + 1], arr[selectedMapIndex]];
    selectedMapIndex++;
    setUnsaved(true);
    renderMapList();
  });

  document.getElementById('map-add-random')?.addEventListener('click', function () {
    const c = getSelectedConfig();
    if (!c) return;
    const available = getAvailableMapsForRotation();
    if (available.length === 0) return;
    const inputEl = document.getElementById('map-add-random-count');
    var n = inputEl ? parseInt(inputEl.value, 10) : 1;
    if (isNaN(n) || n < 1) n = 1;
    n = Math.min(n, available.length);
    c.maps = c.maps || [];
    var pool = available.slice();
    for (var i = 0; i < n; i++) {
      var idx = Math.floor(Math.random() * pool.length);
      c.maps.push(pool[idx]);
      pool.splice(idx, 1);
    }
    selectedMapIndex = c.maps.length - 1;
    setUnsaved(true);
    renderAvailableMapList();
    renderMapList();
  });

  document.getElementById('map-shuffle')?.addEventListener('click', function () {
    const c = getSelectedConfig();
    if (!c || !c.maps || c.maps.length === 0) return;
    var arr = c.maps;
    for (var i = arr.length - 1; i > 0; i--) {
      var j = Math.floor(Math.random() * (i + 1));
      var t = arr[i];
      arr[i] = arr[j];
      arr[j] = t;
    }
    selectedMapIndex = -1;
    setUnsaved(true);
    renderMapList();
  });

  document.getElementById('map-sort')?.addEventListener('click', function () {
    const c = getSelectedConfig();
    if (!c || !c.maps || c.maps.length === 0) return;
    c.maps.sort(function (a, b) { return String(a).localeCompare(b); });
    selectedMapIndex = -1;
    setUnsaved(true);
    renderMapList();
  });

  document.getElementById('map-remove-all')?.addEventListener('click', function () {
    const c = getSelectedConfig();
    if (!c) return;
    if (!c.maps || c.maps.length === 0) return;
    c.maps.length = 0;
    selectedMapIndex = -1;
    setUnsaved(true);
    renderAvailableMapList();
    renderMapList();
  });

  // Global click delegation for map/ban/whitelist/player rows so interactions
  // keep working even when lists are re-rendered frequently while a server runs.
  document.addEventListener('click', function (e) {
    const mapAvailLi = e.target.closest('#available-map-list li[data-index]');
    if (mapAvailLi) {
      const idx = parseInt(mapAvailLi.dataset.index, 10);
      ipcLog('Available map click', 'index=' + idx + ' text=' + mapAvailLi.textContent);
      selectedAvailableMapIndex = idx;
      renderAvailableMapList();
      return;
    }
    const mapLi = e.target.closest('#map-list li[data-index]');
    if (mapLi) {
      const idx = parseInt(mapLi.dataset.index, 10);
      ipcLog('Rotation map click', 'index=' + idx + ' text=' + mapLi.textContent);
      selectedMapIndex = idx;
      renderMapList();
      return;
    }
    const banLi = e.target.closest('#ban-list li[data-index]');
    if (banLi) {
      const idx = parseInt(banLi.getAttribute('data-index'), 10);
      ipcLog('Banlist row click', 'index=' + idx + ' text=' + banLi.textContent);
      selectedBanIndex = idx;
      renderBanList();
      return;
    }
    const wlLi = e.target.closest('#whitelist-list li[data-index]');
    if (wlLi) {
      const idx = parseInt(wlLi.dataset.index, 10);
      ipcLog('Whitelist row click', 'index=' + idx + ' text=' + wlLi.textContent);
      selectedWhitelistIndex = idx;
      renderWhitelist();
      return;
    }
    const banBtn = e.target.closest('.btn-ban-row');
    if (banBtn && document.getElementById('current-players-tbody')?.contains(banBtn)) {
      const ip = (banBtn.getAttribute('data-ip') || '').trim();
      if (ip) {
        ipcLog('Player Ban click', ip);
        const c = getSelectedConfig();
        if (c) {
          c.ban_list = c.ban_list || [];
          if (c.ban_list.indexOf(ip) === -1) {
            c.ban_list.push(ip + ':>' + 'Unspecified ban'.slice(0, BAN_REASON_MAX));
            selectedBanIndex = c.ban_list.length - 1;
            setUnsaved(true);
            renderBanList();
            renderCurrentPlayersTable();
          }
        }
        e.preventDefault();
        e.stopPropagation();
        return;
      }
    }
    const cell = e.target.closest('.player-ip-cell:not(.revealed)');
    if (cell && document.getElementById('current-players-tbody')?.contains(cell)) {
      const idx = cell.getAttribute('data-index');
      if (idx !== null && idx !== '') {
        var i = parseInt(idx, 10);
        if (!isNaN(i)) {
          ipcLog('Player Reveal click', 'row=' + i);
          state.playerListRevealed = state.playerListRevealed || {};
          state.playerListRevealed[i] = true;
          renderCurrentPlayersTable();
          e.preventDefault();
          e.stopPropagation();
        }
      }
    }
  });

  const BAN_REASON_MAX = 21;
  document.getElementById('ban-list-add-comment')?.addEventListener('input', function () {
    const el = document.getElementById('ban-list-add-comment-counter');
    if (el) el.textContent = (this.value || '').length + '/' + BAN_REASON_MAX;
  });
  document.getElementById('ban-list-add')?.addEventListener('click', function () {
    const c = getSelectedConfig();
    const ipEl = document.getElementById('ban-list-add-ip');
    const commentEl = document.getElementById('ban-list-add-comment');
    if (!c || !ipEl) return;
    const ip = (ipEl.value || '').trim();
    if (!ip) return;
    const comment = commentEl ? (commentEl.value || '').trim().slice(0, BAN_REASON_MAX) : '';
    const entry = comment ? ip + ':>' + comment : ip;
    c.ban_list = c.ban_list || [];
    c.ban_list.push(entry);
    ipEl.value = '';
    if (commentEl) { commentEl.value = ''; const counterEl = document.getElementById('ban-list-add-comment-counter'); if (counterEl) counterEl.textContent = '0/' + BAN_REASON_MAX; }
    selectedBanIndex = c.ban_list.length - 1;
    setUnsaved(true);
    renderBanList();
  });
  document.getElementById('ban-list-remove')?.addEventListener('click', function () {
    const c = getSelectedConfig();
    if (!c || !c.ban_list || selectedBanIndex < 0 || selectedBanIndex >= c.ban_list.length) return;
    c.ban_list.splice(selectedBanIndex, 1);
    selectedBanIndex = Math.min(selectedBanIndex, c.ban_list.length - 1);
    if (c.ban_list.length === 0) selectedBanIndex = -1;
    setUnsaved(true);
    renderBanList();
  });

  // whitelist events keep their own click handlers for add/remove;
  // row selection is handled by the global click delegate above.
  document.getElementById('whitelist-add')?.addEventListener('click', function () {
    const c = getSelectedConfig();
    const ipEl = document.getElementById('whitelist-add-ip');
    const commentEl = document.getElementById('whitelist-add-comment');
    if (!c || !ipEl) return;
    const ip = (ipEl.value || '').trim();
    if (!ip) return;
    const comment = commentEl ? (commentEl.value || '').trim() : '';
    const entry = comment ? ip + ':>' + comment : ip;
    c.whitelist = c.whitelist || [];
    c.whitelist.push(entry);
    ipEl.value = '';
    if (commentEl) commentEl.value = '';
    selectedWhitelistIndex = c.whitelist.length - 1;
    setUnsaved(true);
    renderWhitelist();
  });
  document.getElementById('whitelist-remove')?.addEventListener('click', function () {
    const c = getSelectedConfig();
    if (!c || !c.whitelist || selectedWhitelistIndex < 0 || selectedWhitelistIndex >= c.whitelist.length) return;
    c.whitelist.splice(selectedWhitelistIndex, 1);
    selectedWhitelistIndex = Math.min(selectedWhitelistIndex, c.whitelist.length - 1);
    if (c.whitelist.length === 0) selectedWhitelistIndex = -1;
    setUnsaved(true);
    renderWhitelist();
  });

  document.getElementById('style-select')?.addEventListener('change', function () {
    bindConfigToForm();
    const c = getSelectedConfig();
    if (c) c.maps = [];
    setUnsaved(true);
    render();
  });

  document.querySelector('.content')?.addEventListener('input', function () { bindConfigToForm(); setUnsaved(true); });
  document.querySelector('.content')?.addEventListener('change', function () { bindConfigToForm(); setUnsaved(true); });

  document.querySelectorAll('.tab').forEach(tab => {
    const orig = tab.onclick;
    tab.onclick = function () {
      bindConfigToForm();
      if (orig) orig.call(this);
    };
  });

  function requestHostRepaint() {
    if (typeof window.ipc !== 'undefined' && window.ipc.postMessage) {
      requestAnimationFrame(function () {
        requestAnimationFrame(function () {
          try {
            window.ipc.postMessage(JSON.stringify({ action: 'repaint', servers: state.servers }));
          } catch (e) {}
        });
      });
    }
  }

  window.__spectreIpcStatus = function (msg) {
    var el = document.getElementById('ipc-debug');
    if (el) el.textContent = 'IPC: ' + (msg || '');
    if (msg === 'Saved OK') {
        if (autoSaveTimeout !== null) {
          clearTimeout(autoSaveTimeout);
          autoSaveTimeout = null;
          setUnsaved(true);
        } else {
          setUnsaved(false);
        }
        showMessage('Saved');
        requestHostRepaint();
      }
    else if (msg && msg.startsWith('STATE:')) {
      try {
        var json = msg.slice(6);
        var next = JSON.parse(json);
        if (Array.isArray(next)) {
          state.servers = next;
          setUnsaved(false);
          showMessage('Saved');
          requestRender();
          requestHostRepaint();
        }
      } catch (e) { showMessage('Save failed.', true); }
    } else if (msg && msg.startsWith('REFRESH:')) {
      try {
        var json = msg.slice(8);
        var next = JSON.parse(json);
        if (Array.isArray(next)) {
          state.servers = next;
          setUnsaved(false);
          showMessage('Maps loaded');
          requestRender();
          requestHostRepaint();
        }
      } catch (e) { showMessage('Refresh failed.', true); }
    } else if (msg && msg.indexOf('Save') !== -1) showMessage('Save failed', true);
    else if (msg === 'Started OK') {
      showMessage('Started');
      state.serverStarting = false;
      state.serverError = false;
      var idx = state.selectedServerIndex;
      if (state.servers[idx]) state.servers[idx].running = true;
      requestRunningState();
      requestRender();
    } else if (msg === 'All servers started') {
      showMessage('Started');
      state.serverStarting = false;
      state.serverError = false;
      state.servers.forEach(function (s) { s.running = true; });
      requestRunningState();
      requestRender();
    } else if (msg === 'Stopped OK') {
      showMessage('Stopped');
      state.serverStarting = false;
      state.playerCount = { active: '--', total: '--' };
      var idx = state.selectedServerIndex;
      if (state.servers[idx]) state.servers[idx].running = false;
      requestRunningState();
      requestRender();
    } else if (msg === 'All servers stopped') {
      showMessage('Stopped');
      state.servers.forEach(function (s) { s.running = false; });
      state.playerCount = { active: '--', total: '--' };
      requestRunningState();
      requestRender();
    } else if (msg && msg.startsWith('RUNNING:')) {
      try {
        state.serverStarting = false;
        var json = msg.slice(7);
        var ports = JSON.parse(json);
        if (Array.isArray(ports)) {
          state.servers.forEach(function (s) {
            s.running = ports.indexOf(s.port) !== -1;
          });
          requestRender();
        }
      } catch (e) { /* ignore */ }
    } else if (msg && msg.startsWith('PLAYERS:')) {
      var part = msg.slice(8);
      var parts = part.split(',');
      if (parts.length >= 2) {
        state.playerCount = { active: parts[0].trim(), total: parts[1].trim() };
        requestRender();
      }
    } else if (msg && msg.startsWith('PLAYER_LIST:')) {
      try {
        var json = msg.slice(12);
        // If the player list hasn't changed since last time, skip updating state/rendering.
        if (json === lastPlayerListJson) return;
        lastPlayerListJson = json;
        var list = JSON.parse(json);
        state.currentPlayerList = Array.isArray(list) ? list : [];
        requestRender();
      } catch (e) {
        state.currentPlayerList = [];
        requestRender();
      }
    } else if (msg && msg.indexOf('LOG_CONTENT:') === 0) {
      var logEl = document.getElementById('log-content');
      if (logEl) logEl.textContent = msg.slice('LOG_CONTENT:'.length);
    } else if (msg && msg.indexOf('MPMAPLIST_PATH:') === 0) {
      var path = msg.slice('MPMAPLIST_PATH:'.length);
      var inputEl = document.getElementById('mpmaplist-path');
      if (inputEl) inputEl.value = path;
      var s = getSelectedServer();
      if (s) s.mpmaplist_path = path;
      showMessage('Loading…');
      if (typeof window.ipc !== 'undefined' && window.ipc.postMessage) {
        try {
          window.ipc.postMessage(JSON.stringify({ action: 'refresh_mpmaplist', servers: state.servers }));
        } catch (err) { showMessage('Maps loaded'); requestRender(); }
      } else {
        showMessage('The mpmaplist has been set successfully.');
        requestRender();
      }
    } else if (msg && msg.indexOf('MPMAPLIST_PATH_INVALID:') === 0) {
      showMessage(msg.slice('MPMAPLIST_PATH_INVALID:'.length), true);
    } else if (msg && msg.indexOf('HD2DS_PATH:') === 0) {
      var path = msg.slice('HD2DS_PATH:'.length);
      var inputEl = document.getElementById('edit-server-hd2ds-path');
      if (inputEl) inputEl.value = path;
    } else if (msg && msg.indexOf('HD2DS_SABRE_PATH:') === 0) {
      var path = msg.slice('HD2DS_SABRE_PATH:'.length);
      var inputEl = document.getElementById('edit-server-sabre-path');
      if (inputEl) inputEl.value = path;
    } else if (msg && (msg.indexOf('Error') !== -1 || msg.indexOf('failed') !== -1)) {
      if (state.serverStarting) {
        state.serverStarting = false;
        state.serverError = true;
        requestRender();
      }
      showMessage(msg, true);
    }
  };

  document.getElementById('logs-refresh')?.addEventListener('click', function () {
    requestLogContent();
  });
  document.getElementById('logs-open-folder')?.addEventListener('click', function () {
    openLogFile();
  });

  document.getElementById('mpmaplist-browse')?.addEventListener('click', function () {
    bindConfigToForm();
    if (typeof window.ipc !== 'undefined' && window.ipc.postMessage) {
      try {
        var payload = JSON.stringify({ action: 'browse_mpmaplist', servers: state.servers });
        window.ipc.postMessage(payload);
      } catch (err) {
        ipcLog('Browse mpmaplist postMessage error', err);
      }
    }
  });
  render();
  if (state.activeTab === 'logs') requestLogContent();
  ipcLog('Ready');
})();
