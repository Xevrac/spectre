/**
 * Server Utility — card UI logic.
 * State and actions; Rust bridge (load/save/start) can be wired later.
 */

(function () {
  'use strict';

  const state = {
    servers: [
      {
        name: 'Server 1',
        port: 22000,
        use_sabre_squadron: true,
        current_config: 'Default',
        configs: [
          {
            name: 'Default',
            domain: 'Internet',
            session_name: 'A Spectre Session',
            style: 'Occupation',
            max_clients: 64,
            point_limit: 0,
            round_limit: 25,
            round_count: 1,
            respawn_time: 20,
            spawn_protection: 0,
            warmup: 10,
            inverse_damage: 0,
            friendly_fire: true,
            auto_team_balance: false,
            third_person_view: false,
            allow_crosshair: true,
            falling_dmg: true,
            allow_respawn: true,
            allow_vehicles: true,
            difficulty: 'Hard',
            respawn_number: 1,
            team_respawn: true,
            password: '',
            admin_pass: '',
            max_ping: 0,
            max_freq: 0,
            max_inactivity: 0,
            voice_chat: 0,
            maps: ['map_01', 'map_02']
          }
        ]
      }
    ],
    selectedServerIndex: 0,
    selectedConfigIndex: 0,
    activeTab: 'maps',
    /** Maps from mpmaplist.txt by style: { Occupation: [...], Cooperative: [...], ... } */
    availableMapsByStyle: {}
  };

  function ipcLog(msg, detail) {
    console.log('[IPC JS] ' + msg, detail !== undefined ? detail : '');
  }
  if (typeof window !== 'undefined' && window.__spectreInitialState) {
    try {
      const initial = window.__spectreInitialState;
      if (initial.servers && Array.isArray(initial.servers) && initial.servers.length > 0) {
        state.servers = initial.servers;
        ipcLog('Initial state applied', state.servers.length + ' servers');
      } else {
        ipcLog('Initial state had no servers, keeping default');
      }
      if (typeof initial.selectedServerIndex === 'number') state.selectedServerIndex = Math.min(initial.selectedServerIndex, state.servers.length - 1);
      if (typeof initial.selectedConfigIndex === 'number') state.selectedConfigIndex = Math.min(initial.selectedConfigIndex, (state.servers[state.selectedServerIndex]?.configs?.length || 1) - 1);
      if (initial.availableMapsByStyle && typeof initial.availableMapsByStyle === 'object') {
        state.availableMapsByStyle = initial.availableMapsByStyle;
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
      };
    });
    document.querySelectorAll('.tab-panel').forEach(p => {
      p.classList.toggle('hidden', p.id !== 'panel-' + state.activeTab);
    });
  }

  function bindFormToConfig() {
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
    set('profile-name', c.name);
    set('session-name', c.session_name);
    set('style-select', c.style);
    const domain = (c.domain === 'LAN') ? 'LAN' : 'Internet';
    const domainEl = document.getElementById('domain-' + domain.toLowerCase());
    if (domainEl) domainEl.checked = true;
    set('max-clients', c.max_clients);
    set('point-limit', c.point_limit);
    set('round-limit', c.round_limit);
    set('round-count', c.round_count != null ? c.round_count : 1);
    set('respawn-time', c.respawn_time);
    set('spawn-protection', c.spawn_protection != null ? c.spawn_protection : 0);
    set('warmup', c.warmup != null ? c.warmup : 10);
    set('inverse-damage', c.inverse_damage != null ? c.inverse_damage : 0);
    setCheck('friendly-fire', c.friendly_fire);
    setCheck('auto-team-balance', c.auto_team_balance);
    setCheck('third-person-view', c.third_person_view);
    setCheck('allow-crosshair', c.allow_crosshair != null ? c.allow_crosshair : true);
    setCheck('falling-dmg', c.falling_dmg != null ? c.falling_dmg : true);
    setCheck('allow-respawn', c.allow_respawn != null ? c.allow_respawn : true);
    setCheck('allow-vehicles', c.allow_vehicles != null ? c.allow_vehicles : true);
    set('difficulty', c.difficulty);
    set('respawn-number', c.respawn_number != null ? c.respawn_number : 1);
    setCheck('team-respawn', c.team_respawn != null ? c.team_respawn : true);
    set('password', c.password);
    set('admin-pass', c.admin_pass);
    set('max-ping', c.max_ping);
    set('max-freq', c.max_freq != null ? c.max_freq : 0);
    set('max-inactivity', c.max_inactivity != null ? c.max_inactivity : 0);
    set('voice-chat', c.voice_chat != null ? String(c.voice_chat) : '0');
  }

  function bindConfigToForm() {
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
    const name = get('profile-name').trim();
    if (name) c.name = name;
    c.session_name = get('session-name');
    c.style = get('style-select');
    const domainRadio = document.querySelector('input[name="domain-type"]:checked');
    c.domain = domainRadio ? domainRadio.value : 'Internet';
    c.max_clients = parseInt(get('max-clients'), 10) || 64;
    c.point_limit = parseInt(get('point-limit'), 10) || 0;
    c.round_limit = parseInt(get('round-limit'), 10) || 25;
    c.round_count = parseInt(get('round-count'), 10);
    if (isNaN(c.round_count) || c.round_count < 1) c.round_count = 1;
    if (c.round_count > 20) c.round_count = 20;
    c.respawn_time = parseInt(get('respawn-time'), 10) || 20;
    c.spawn_protection = parseInt(get('spawn-protection'), 10) || 0;
    if (c.spawn_protection > 30) c.spawn_protection = 30;
    c.warmup = parseInt(get('warmup'), 10) || 0;
    if (c.warmup > 60) c.warmup = 60;
    c.inverse_damage = parseInt(get('inverse-damage'), 10) || 0;
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
    c.max_freq = parseInt(get('max-freq'), 10) || 0;
    c.max_inactivity = parseInt(get('max-inactivity'), 10) || 0;
    c.voice_chat = parseInt(get('voice-chat'), 10) || 0;
    if (c.voice_chat > 6) c.voice_chat = 6;
  }

  let selectedMapIndex = -1;
  let selectedAvailableMapIndex = -1;

  /** Pool of map names for the current config's style (from mpmaplist). */
  function getPoolForCurrentStyle() {
    const c = getSelectedConfig();
    const style = c ? (c.style || 'Occupation') : 'Occupation';
    let arr = state.availableMapsByStyle[style];
    if (!Array.isArray(arr) || arr.length === 0) {
      if (style === 'Invasion') arr = state.availableMapsByStyle['Occupation'];
      else if (style === 'Objectives') arr = state.availableMapsByStyle['Objectives'];
    }
    return Array.isArray(arr) ? arr : [];
  }

  /** Maps that can be added: in pool for current style but not already in rotation. */
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
      ul.innerHTML = '<li class="empty-hint">No maps to add. Set mpmaplist path or add from rotation.</li>';
      selectedAvailableMapIndex = -1;
      return;
    }
    ul.innerHTML = available.map(function (name, i) {
      return '<li class="' + (i === selectedAvailableMapIndex ? 'selected' : '') + '" data-index="' + i + '">' + escapeHtml(name) + '</li>';
    }).join('');
    ul.querySelectorAll('li[data-index]').forEach(function (li) {
      li.addEventListener('click', function () {
        selectedAvailableMapIndex = parseInt(this.dataset.index, 10);
        renderAvailableMapList();
      });
    });
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
    ul.querySelectorAll('li[data-index]').forEach(li => {
      li.addEventListener('click', function () {
        selectedMapIndex = parseInt(this.dataset.index, 10);
        renderMapList();
      });
    });
  }

  function render() {
    // Each server's display name = its current profile name (dropdown shows profile names)
    state.servers.forEach(function (srv) {
      const cur = srv.configs && srv.configs.find(function (cfg) { return cfg.name === srv.current_config; });
      if (cur) srv.name = cur.name;
    });
    const s = getSelectedServer();
    const c = getSelectedConfig();
    if (s && c) s.name = c.name;
    renderServerSelect();
    renderProfileList();
    renderTabs();
    bindFormToConfig();
    renderAvailableMapList();
    renderMapList();
    const gameSelect = document.getElementById('game-select');
    if (gameSelect && s) gameSelect.value = s.use_sabre_squadron ? 'sabre' : 'hd2';
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

  // Profile name: sync input -> config, refresh list and server dropdown (server name = profile name)
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

  // Profile actions
  document.getElementById('profile-new')?.addEventListener('click', function () {
    const s = getSelectedServer();
    ipcLog('Profile New clicked', s ? 'server=' + s.name : 'no server');
    if (s) {
      const last = s.configs[s.configs.length - 1];
      const base = last ? { ...last } : { name: 'New profile', domain: 'Internet', session_name: 'A Spectre Session', style: 'Occupation', max_clients: 64, point_limit: 0, round_limit: 25, round_count: 1, respawn_time: 20, spawn_protection: 0, warmup: 10, inverse_damage: 0, friendly_fire: true, auto_team_balance: false, third_person_view: false, allow_crosshair: true, falling_dmg: true, allow_respawn: true, allow_vehicles: true, difficulty: 'Hard', respawn_number: 1, team_respawn: true, password: '', admin_pass: '', max_ping: 0, max_freq: 0, max_inactivity: 0, voice_chat: 0, maps: [] };
      s.configs.push({ ...base, name: 'New profile', maps: Array.isArray(base.maps) ? base.maps.slice() : [] });
      state.selectedConfigIndex = s.configs.length - 1;
      render();
      ipcLog('Profile added', 'total configs=' + s.configs.length);
    }
  });
  document.getElementById('profile-delete')?.addEventListener('click', function () {
    const s = getSelectedServer();
    if (s && s.configs.length > 1) {
      s.configs.splice(state.selectedConfigIndex, 1);
      state.selectedConfigIndex = Math.min(state.selectedConfigIndex, s.configs.length - 1);
      render();
    }
  });
  document.getElementById('profile-duplicate')?.addEventListener('click', function () {
    const s = getSelectedServer();
    const c = getSelectedConfig();
    if (s && c) {
      const copy = { ...c, name: c.name + ' (copy)' };
      s.configs.splice(state.selectedConfigIndex + 1, 0, copy);
      state.selectedConfigIndex++;
      render();
    }
  });

  document.getElementById('server-add')?.addEventListener('click', function () {
    bindConfigToForm();
    const nextPort = 22000 + state.servers.length;
    const newServer = {
      name: 'New profile',
      running: false,
      watchdog: false,
      messages: false,
      users: [],
      port: nextPort,
      use_sabre_squadron: true,
      current_config: 'Default',
      configs: [
        { name: 'Default', domain: 'Internet', session_name: 'A Spectre Session', style: 'Occupation', max_clients: 64, point_limit: 0, round_limit: 25, round_count: 1, respawn_time: 20, spawn_protection: 0, warmup: 10, inverse_damage: 0, friendly_fire: true, auto_team_balance: false, third_person_view: false, allow_crosshair: true, falling_dmg: true, allow_respawn: true, allow_vehicles: true, difficulty: 'Hard', respawn_number: 1, team_respawn: true, password: '', admin_pass: '', max_ping: 0, max_freq: 0, max_inactivity: 0, voice_chat: 0, maps: [] }
      ]
    };
    state.servers.push(newServer);
    state.selectedServerIndex = state.servers.length - 1;
    state.selectedConfigIndex = 0;
    render();
    ipcLog('Server added', 'total servers=' + state.servers.length);
  });

  document.getElementById('edit-server')?.addEventListener('click', function () {
    const s = getSelectedServer();
    if (!s) return;
    const portEl = document.getElementById('edit-server-port');
    const dialog = document.getElementById('edit-server-dialog');
    if (portEl && dialog) {
      portEl.value = s.port || 22000;
      dialog.showModal();
    }
  });

  document.getElementById('edit-server-dialog')?.addEventListener('submit', function (e) {
    e.preventDefault();
    const s = getSelectedServer();
    const portEl = document.getElementById('edit-server-port');
    const dialog = document.getElementById('edit-server-dialog');
    if (s && portEl && dialog) {
      const port = parseInt(portEl.value, 10);
      if (port >= 1 && port <= 65535) s.port = port;
      dialog.close();
      renderServerSelect();
    }
  });

  document.getElementById('edit-server-cancel')?.addEventListener('click', function () {
    document.getElementById('edit-server-dialog')?.close();
  });

  document.getElementById('save-config')?.addEventListener('click', function () {
    bindConfigToForm();
    const payload = JSON.stringify({ action: 'save', servers: state.servers });
    ipcLog('Save clicked', 'ipc.postMessage body=' + payload.length + ' bytes');
    if (typeof window.ipc !== 'undefined' && window.ipc.postMessage) {
      try {
        window.ipc.postMessage(payload);
        showSaveStatus('Saving…');
        // Result is reported via window.__spectreIpcStatus() from Rust after save completes.
      } catch (err) {
        ipcLog('Save postMessage error', err);
        showSaveStatus('Save failed.');
        if (window.__spectreIpcStatus) window.__spectreIpcStatus('Save error: ' + (err && err.message ? err.message : 'postMessage'));
      }
    } else {
      ipcLog('Save skipped', 'window.ipc.postMessage not available');
      showSaveStatus('Save not available.');
      if (window.__spectreIpcStatus) window.__spectreIpcStatus('IPC not available');
    }
  });

  function showSaveStatus(msg) {
    var el = document.getElementById('save-config');
    if (!el) return;
    var orig = el.textContent;
    el.textContent = msg;
    setTimeout(function () { el.textContent = orig; }, 2000);
  }

  document.getElementById('start-server')?.addEventListener('click', function () {
    bindConfigToForm();
    // TODO: bridge to Rust — start server process for selected server
    console.log('Start Server (bridge not wired)');
  });

  document.getElementById('start-all-servers')?.addEventListener('click', function () {
    bindConfigToForm();
    if (typeof window.ipc !== 'undefined' && window.ipc.postMessage) {
      try {
        const payload = JSON.stringify({ action: 'start_all', servers: state.servers });
        window.ipc.postMessage(payload);
        ipcLog('Start All Servers', state.servers.length + ' servers');
      } catch (err) {
        ipcLog('Start All postMessage error', err);
      }
    } else {
      ipcLog('Start All Servers (bridge not wired)', state.servers.length);
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
    renderAvailableMapList();
    renderMapList();
  });
  document.getElementById('map-remove')?.addEventListener('click', function () {
    const c = getSelectedConfig();
    if (!c || !c.maps || selectedMapIndex < 0 || selectedMapIndex >= c.maps.length) return;
    c.maps.splice(selectedMapIndex, 1);
    selectedMapIndex = Math.min(selectedMapIndex, c.maps.length - 1);
    if (c.maps.length === 0) selectedMapIndex = -1;
    renderAvailableMapList();
    renderMapList();
  });
  document.getElementById('map-up')?.addEventListener('click', function () {
    const c = getSelectedConfig();
    if (!c || !c.maps || selectedMapIndex <= 0) return;
    const arr = c.maps;
    [arr[selectedMapIndex - 1], arr[selectedMapIndex]] = [arr[selectedMapIndex], arr[selectedMapIndex - 1]];
    selectedMapIndex--;
    renderMapList();
  });
  document.getElementById('map-down')?.addEventListener('click', function () {
    const c = getSelectedConfig();
    if (!c || !c.maps || selectedMapIndex < 0 || selectedMapIndex >= c.maps.length - 1) return;
    const arr = c.maps;
    [arr[selectedMapIndex], arr[selectedMapIndex + 1]] = [arr[selectedMapIndex + 1], arr[selectedMapIndex]];
    selectedMapIndex++;
    renderMapList();
  });

  document.getElementById('style-select')?.addEventListener('change', function () {
    bindConfigToForm();
    // Reset map rotation when style changes; previous maps may be from a different game mode
    const c = getSelectedConfig();
    if (c) c.maps = [];
    render();
  });

  // Sync form -> state when switching tabs/config
  document.querySelectorAll('.tab').forEach(tab => {
    const orig = tab.onclick;
    tab.onclick = function () {
      bindConfigToForm();
      if (orig) orig.call(this);
    };
  });

  window.__spectreIpcStatus = function (msg) {
    var el = document.getElementById('ipc-debug');
    if (el) el.textContent = 'IPC: ' + (msg || '');
    if (msg === 'Saved OK') showSaveStatus('Saved');
    else if (msg && msg.indexOf('Save') !== -1) showSaveStatus('Save failed');
  };
  render();
  ipcLog('Ready');
})();
