(function () {
  'use strict';

  const state = {
    servers: [
      {
        name: 'Server 1',
        port: 22000,
        use_sabre_squadron: true,
        mpmaplist_path: '',
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
    activeTab: 'maps'
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
    if (s) s.current_config = c.name;
  }

  let selectedMapIndex = -1;
  let selectedAvailableMapIndex = -1;
  let unsavedChanges = false;
  let unsavedPollInterval = null;
  const UNSAVED_POLL_MS = 400;

  function setUnsaved(value) {
    unsavedChanges = !!value;
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
      requestAnimationFrame(function () {
        el.classList.remove('visible');
        void el.offsetHeight;
        requestAnimationFrame(function () { void el.offsetHeight; });
      });
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
      if (style === 'Invasion') arr = byStyle['Occupation'];
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
      const base = last ? { ...last } : { name: 'New profile', domain: 'Internet', session_name: 'A Spectre Session', style: 'Occupation', max_clients: 64, point_limit: 0, round_limit: 25, round_count: 1, respawn_time: 20, spawn_protection: 0, warmup: 10, inverse_damage: 0, friendly_fire: true, auto_team_balance: false, third_person_view: false, allow_crosshair: true, falling_dmg: true, allow_respawn: true, allow_vehicles: true, difficulty: 'Hard', respawn_number: 1, team_respawn: true, password: '', admin_pass: '', max_ping: 0, max_freq: 0, max_inactivity: 0, voice_chat: 0, maps: [] };
      s.configs.push({ ...base, name: 'New profile', maps: Array.isArray(base.maps) ? base.maps.slice() : [] });
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
      const copy = { ...c, name: c.name + ' (copy)' };
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
      mpmaplist_path: '',
      current_config: 'Default',
      configs: [
        { name: 'Default', domain: 'Internet', session_name: 'A Spectre Session', style: 'Occupation', max_clients: 64, point_limit: 0, round_limit: 25, round_count: 1, respawn_time: 20, spawn_protection: 0, warmup: 10, inverse_damage: 0, friendly_fire: true, auto_team_balance: false, third_person_view: false, allow_crosshair: true, falling_dmg: true, allow_respawn: true, allow_vehicles: true, difficulty: 'Hard', respawn_number: 1, team_respawn: true, password: '', admin_pass: '', max_ping: 0, max_freq: 0, max_inactivity: 0, voice_chat: 0, maps: [] }
      ]
    };
    state.servers.push(newServer);
    state.selectedServerIndex = state.servers.length - 1;
    state.selectedConfigIndex = 0;
    setUnsaved(true);
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
    s.port = port;
    setUnsaved(true);
    dialog.close();
    renderServerSelect();
  });

  document.getElementById('edit-server-cancel')?.addEventListener('click', function () {
    document.getElementById('edit-server-dialog')?.close();
  });

  document.getElementById('save-config')?.addEventListener('click', function () {
    bindConfigToForm();
    ensureCurrentConfigs();
    const payload = JSON.stringify({ action: 'save', servers: state.servers });
    ipcLog('Save clicked', 'ipc.postMessage body=' + payload.length + ' bytes');
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
  });

  function showMessage(msg, isError) {
    var el = document.getElementById('message-banner');
    if (!el) return;
    el.textContent = msg || '';
    el.className = 'message-banner' + (isError ? ' error' : (msg ? ' success' : ''));
    if (msg) setTimeout(function () { el.textContent = ''; el.className = 'message-banner'; }, 3000);
  }

  document.getElementById('start-server')?.addEventListener('click', function () {
    bindConfigToForm();
    ensureCurrentConfigs();
    if (typeof window.ipc !== 'undefined' && window.ipc.postMessage) {
      try {
        const payload = JSON.stringify({
          action: 'start',
          server_index: state.selectedServerIndex,
          servers: state.servers
        });
        window.ipc.postMessage(payload);
        ipcLog('Start Server', state.selectedServerIndex);
      } catch (err) {
        ipcLog('Start Server postMessage error', err);
      }
    } else {
      ipcLog('Start Server (bridge not wired)');
    }
  });

  document.getElementById('start-all-servers')?.addEventListener('click', function () {
    bindConfigToForm();
    ensureCurrentConfigs();
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

  document.getElementById('style-select')?.addEventListener('change', function () {
    bindConfigToForm();
    const c = getSelectedConfig();
    if (c) c.maps = [];
    setUnsaved(true);
    render();
  });

  document.querySelector('.content')?.addEventListener('input', function () { setUnsaved(true); });
  document.querySelector('.content')?.addEventListener('change', function () { setUnsaved(true); });

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
    if (msg === 'Saved OK') { setUnsaved(false); showMessage('Saved'); }
    else if (msg && msg.startsWith('STATE:')) {
      try {
        var json = msg.slice(6);
        var next = JSON.parse(json);
        if (Array.isArray(next)) {
          state.servers = next;
          setUnsaved(false);
          showMessage('Saved');
          render();
        }
      } catch (e) { showMessage('Save failed.', true); }
    } else if (msg && msg.startsWith('REFRESH:')) {
      try {
        var json = msg.slice(8);
        var next = JSON.parse(json);
        if (Array.isArray(next)) {
          state.servers = next;
          next.forEach(function (s) { (s.configs || []).forEach(function (c) { if (c) c.maps = []; }); });
          setUnsaved(false);
          showMessage('Maps loaded');
          render();
        }
      } catch (e) { showMessage('Refresh failed.', true); }
    } else if (msg && msg.indexOf('Save') !== -1) showMessage('Save failed', true);
    else if (msg === 'Started OK' || msg === 'All servers started') showMessage('Started');
    else if (msg && msg.indexOf('MPMAPLIST_PATH:') === 0) {
      var path = msg.slice('MPMAPLIST_PATH:'.length);
      var inputEl = document.getElementById('mpmaplist-path');
      if (inputEl) inputEl.value = path;
      var s = getSelectedServer();
      if (s) s.mpmaplist_path = path;
      showMessage('Loading…');
      if (typeof window.ipc !== 'undefined' && window.ipc.postMessage) {
        try {
          window.ipc.postMessage(JSON.stringify({ action: 'refresh_mpmaplist', servers: state.servers }));
        } catch (err) { showMessage('Maps loaded'); render(); }
      } else {
        showMessage('The mpmaplist has been set successfully.');
        render();
      }
    } else if (msg && msg.indexOf('MPMAPLIST_PATH_INVALID:') === 0) {
      showMessage(msg.slice('MPMAPLIST_PATH_INVALID:'.length), true);
    } else if (msg && (msg.indexOf('Error') !== -1 || msg.indexOf('failed') !== -1)) showMessage(msg, true);
  };

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
  ipcLog('Ready');
})();
