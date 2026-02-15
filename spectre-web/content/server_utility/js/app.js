/**
 * Server Utility — card UI logic.
 * State and actions; Rust bridge (load/save/start) can be wired later.
 */

(function () {
  'use strict';

  // Mock state (mirrors ServerLauncherData / Server / ServerConfig from spectre-core)
  const state = {
    servers: [
      {
        name: 'Server 1',
        port: 22000,
        use_sabre_squadron: false,
        current_config: 'Default',
        configs: [
          {
            name: 'Default',
            session_name: 'H&D 2 SERVER',
            style: 'Occupation',
            max_clients: 64,
            point_limit: 0,
            round_limit: 25,
            respawn_time: 20,
            friendly_fire: true,
            auto_team_balance: false,
            difficulty: 'Hard',
            password: '',
            admin_pass: '',
            max_ping: 0,
            maps: ['map_01', 'map_02']
          }
        ]
      }
    ],
    selectedServerIndex: 0,
    selectedConfigIndex: 0,
    activeTab: 'maps'
  };

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
      if (el) el.value = value;
    };
    const setCheck = (id, value) => {
      const el = document.getElementById(id);
      if (el) el.checked = !!value;
    };
    set('session-name', c.session_name);
    set('style-select', c.style);
    set('max-clients', c.max_clients);
    set('point-limit', c.point_limit);
    set('round-limit', c.round_limit);
    set('respawn-time', c.respawn_time);
    setCheck('friendly-fire', c.friendly_fire);
    setCheck('auto-team-balance', c.auto_team_balance);
    set('difficulty', c.difficulty);
    set('password', c.password);
    set('admin-pass', c.admin_pass);
    set('max-ping', c.max_ping);
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
    c.session_name = get('session-name');
    c.style = get('style-select');
    c.max_clients = parseInt(get('max-clients'), 10) || 64;
    c.point_limit = parseInt(get('point-limit'), 10) || 0;
    c.round_limit = parseInt(get('round-limit'), 10) || 25;
    c.respawn_time = parseInt(get('respawn-time'), 10) || 20;
    c.friendly_fire = getCheck('friendly-fire');
    c.auto_team_balance = getCheck('auto-team-balance');
    c.difficulty = get('difficulty');
    c.password = get('password');
    c.admin_pass = get('admin-pass');
    c.max_ping = parseInt(get('max-ping'), 10) || 0;
  }

  function renderMapList() {
    const ul = document.getElementById('map-list');
    if (!ul) return;
    const c = getSelectedConfig();
    const maps = c ? c.maps : [];
    ul.innerHTML = maps.length === 0
      ? '<li class="empty-hint">No maps. Add maps above.</li>'
      : maps.map((m, i) => `<li class="selected" data-index="${i}">${escapeHtml(m)}</li>`).join('');
  }

  function render() {
    renderServerSelect();
    renderProfileList();
    renderTabs();
    bindFormToConfig();
    renderMapList();
    const gameSelect = document.getElementById('game-select');
    const s = getSelectedServer();
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

  // Profile actions (placeholders)
  document.getElementById('profile-new')?.addEventListener('click', function () {
    const s = getSelectedServer();
    if (s) {
      s.configs.push({ ...s.configs[s.configs.length - 1] || {}, name: 'New profile', maps: [] });
      state.selectedConfigIndex = s.configs.length - 1;
      render();
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

  document.getElementById('edit-server')?.addEventListener('click', function () {
    // TODO: open edit server dialog / bridge to Rust
  });

  document.getElementById('save-config')?.addEventListener('click', function () {
    bindConfigToForm();
    // TODO: bridge to Rust — save config file
    console.log('Save Configuration (bridge not wired)');
  });

  document.getElementById('start-server')?.addEventListener('click', function () {
    bindConfigToForm();
    // TODO: bridge to Rust — start server process
    console.log('Start Server (bridge not wired)');
  });

  // Map toolbar placeholders
  ['map-add', 'map-remove', 'map-up', 'map-down'].forEach(id => {
    document.getElementById(id)?.addEventListener('click', function () {
      console.log(id + ' (bridge not wired)');
    });
  });

  // Sync form -> state when switching tabs/config
  document.querySelectorAll('.tab').forEach(tab => {
    const orig = tab.onclick;
    tab.onclick = function () {
      bindConfigToForm();
      if (orig) orig.call(this);
    };
  });

  render();
})();
