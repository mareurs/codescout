(function() {
    'use strict';

    const POLL_INTERVAL = 5000;
    let callsChart = null;
    let lastErrors = [];

    function activePage() {
        const btn = document.querySelector('.nav-btn.active');
        return btn ? btn.dataset.page : 'overview';
    }

    // --- Page navigation ---
    document.querySelectorAll('.nav-btn').forEach(btn => {
        btn.addEventListener('click', () => {
            document.querySelectorAll('.nav-btn').forEach(b => b.classList.remove('active'));
            document.querySelectorAll('.page').forEach(p => p.classList.remove('active'));
            btn.classList.add('active');
            document.getElementById('page-' + btn.dataset.page).classList.add('active');
        });
    });

    // --- Theme ---
    const theme = localStorage.getItem('ce-theme') || 'light';
    if (theme === 'dark') document.documentElement.setAttribute('data-theme', 'dark');
    document.getElementById('theme-toggle').addEventListener('click', () => {
        const isDark = document.documentElement.getAttribute('data-theme') === 'dark';
        document.documentElement.setAttribute('data-theme', isDark ? '' : 'dark');
        localStorage.setItem('ce-theme', isDark ? 'light' : 'dark');
        document.getElementById('theme-toggle').textContent = isDark ? '\u2600' : '\u263e';
    });
    document.getElementById('theme-toggle').textContent = theme === 'dark' ? '\u263e' : '\u2600';

    // --- Data fetching ---
    async function fetchJson(url) {
        try {
            const resp = await fetch(url);
            return await resp.json();
        } catch (e) {
            return null;
        }
    }

    // --- Render helpers ---
    function kv(label, value) { return '<div><strong>' + label + ':</strong> ' + value + '</div>'; }
    function dot(cls) { return '<span class="status-dot ' + cls + '"></span>'; }
    function esc(str) {
        return String(str)
            .replace(/&/g, '&amp;')
            .replace(/</g, '&lt;')
            .replace(/>/g, '&gt;')
            .replace(/"/g, '&quot;')
            .replace(/'/g, '&#39;');
    }

    // --- Overview page ---
    async function refreshOverview() {
        const [proj, config, index, drift, libs] = await Promise.all([
            fetchJson('/api/project'),
            fetchJson('/api/config'),
            fetchJson('/api/index'),
            fetchJson('/api/drift?threshold=0.1'),
            fetchJson('/api/libraries'),
        ]);

        if (proj) {
            const langs = (proj.languages || []).join(', ') || 'none detected';
            const git = proj.git_branch
                ? proj.git_branch + (proj.git_dirty ? ' (dirty)' : '')
                : 'not a git repo';
            document.getElementById('project-info').innerHTML =
                kv('Name', esc(proj.name)) +
                kv('Root', '<code>' + esc(proj.root) + '</code>') +
                kv('Languages', esc(langs)) +
                kv('Git', esc(git));
        }

        if (config) {
            const embed = config.embeddings || {};
            const sec = config.security || {};
            document.getElementById('config-info').innerHTML =
                kv('Embedding model', esc(embed.model || 'default')) +
                kv('Chunk size', esc(embed.chunk_size || '?')) +
                kv('Shell mode', esc(sec.shell_mode || 'disabled'));
        }

        if (index) {
            if (!index.available) {
                document.getElementById('index-info').innerHTML =
                    '<p class="muted">' + esc(index.reason) + '</p>';
            } else {
                const statusCls = index.stale ? 'status-stale' : 'status-fresh';
                const statusText = index.stale
                    ? 'Stale (' + index.behind_commits + ' commits behind)'
                    : 'Up to date';
                document.getElementById('index-info').innerHTML =
                    dot(statusCls) + esc(statusText) +
                    kv('Files', index.file_count) +
                    kv('Chunks', index.chunk_count) +
                    kv('Model', esc(index.model || 'unknown'));
            }
        }

        if (drift && drift.available && drift.files && drift.files.length > 0) {
            const rows = drift.files.slice(0, 10).map(f =>
                '<tr><td>' + esc(f.path) + '</td>' +
                '<td class="num">' + f.avg_drift.toFixed(2) + '</td>' +
                '<td class="num">' + f.max_drift.toFixed(2) + '</td></tr>'
            ).join('');
            document.getElementById('drift-info').innerHTML =
                '<table><thead><tr><th>File</th><th>Avg</th><th>Max</th></tr></thead><tbody>' +
                rows + '</tbody></table>';
        } else {
            document.getElementById('drift-info').innerHTML =
                '<p class="muted">No significant drift detected.</p>';
        }

        if (libs) {
            const entries = libs.libraries || [];
            if (entries.length === 0) {
                document.getElementById('libraries-info').innerHTML =
                    '<p class="muted">No libraries registered.</p>';
            } else {
                const rows = entries.map(l =>
                    '<tr><td>' + esc(l.name) + '</td><td>' + esc(l.language) +
                    '</td><td>' + (l.indexed ? '&#10003;' : '&mdash;') + '</td></tr>'
                ).join('');
                document.getElementById('libraries-info').innerHTML =
                    '<table><thead><tr><th>Name</th><th>Language</th><th>Indexed</th></tr></thead>' +
                    '<tbody>' + rows + '</tbody></table>';
            }
        }
    }

    // --- Errors rendering (search + collapse) ---
    function renderErrors() {
        const el = document.getElementById('errors-list');
        if (!lastErrors.length) { el.innerHTML = '<p class="muted">No recent errors.</p>'; return; }

        const q = (document.getElementById('errors-search').value || '').toLowerCase();
        const collapse = document.getElementById('errors-collapse').checked;

        let rows = lastErrors.filter(e =>
            !q ||
            (e.tool || '').toLowerCase().includes(q) ||
            (e.outcome || '').toLowerCase().includes(q) ||
            (e.message || '').toLowerCase().includes(q)
        );

        if (!rows.length) { el.innerHTML = '<p class="muted">No matching errors.</p>'; return; }

        let thead, tbody;
        if (collapse) {
            const groups = new Map();
            rows.forEach(e => {
                const key = (e.tool || '') + '\x00' + (e.outcome || '') + '\x00' + (e.message || '');
                if (!groups.has(key)) groups.set(key, { ...e, count: 1 });
                else groups.get(key).count++;
            });
            thead = '<thead><tr><th>Time</th><th>Tool</th><th>Type</th><th>Message</th><th>#</th></tr></thead>';
            tbody = [...groups.values()].map(e =>
                '<tr><td>' + esc(e.timestamp) + '</td><td>' + esc(e.tool) +
                '</td><td>' + esc(e.outcome) + '</td><td>' + esc(e.message || '&mdash;') +
                '</td><td class="num">' + (e.count > 1 ? '<span class="badge">' + e.count + '</span>' : '') + '</td></tr>'
            ).join('');
        } else {
            thead = '<thead><tr><th>Time</th><th>Tool</th><th>Type</th><th>Message</th></tr></thead>';
            tbody = rows.map(e =>
                '<tr><td>' + esc(e.timestamp) + '</td><td>' + esc(e.tool) +
                '</td><td>' + esc(e.outcome) + '</td><td>' + esc(e.message || '&mdash;') + '</td></tr>'
            ).join('');
        }
        el.innerHTML = '<table>' + thead + '<tbody>' + tbody + '</tbody></table>';
    }

    // --- Tool Stats page ---
    async function refreshStats() {
        const win = document.getElementById('stats-window').value;
        const [usage, errors, lsp] = await Promise.all([
            fetchJson('/api/usage?window=' + win),
            fetchJson('/api/errors?limit=20'),
            fetchJson('/api/lsp?window=' + win),
        ]);

        if (usage && usage.available) {
            const tools = usage.by_tool || [];
            const totalCalls = usage.total_calls || 0;
            const totalErrors = tools.reduce((sum, t) => sum + t.errors, 0);
            const totalOverflows = tools.reduce((sum, t) => sum + t.overflows, 0);
            const errorPct = totalCalls > 0 ? (totalErrors / totalCalls * 100).toFixed(1) : '0';
            const overflowPct = totalCalls > 0 ? (totalOverflows / totalCalls * 100).toFixed(1) : '0';

            document.getElementById('usage-summary').innerHTML =
                '<strong>' + totalCalls + '</strong> total calls &nbsp;|&nbsp; ' +
                '<strong>' + errorPct + '%</strong> error rate &nbsp;|&nbsp; ' +
                '<strong>' + overflowPct + '%</strong> overflow rate';

            // Chart — built once on first load, never updated by polling
            if (!callsChart) {
                const labels = tools.map(t => t.tool);
                const data = tools.map(t => t.calls);
                const ctx = document.getElementById('calls-chart').getContext('2d');
                callsChart = new Chart(ctx, {
                    type: 'bar',
                    data: {
                        labels,
                        datasets: [{
                            label: 'Calls',
                            data,
                            backgroundColor: 'rgba(13, 110, 253, 0.7)',
                        }],
                    },
                    options: {
                        responsive: true,
                        plugins: { legend: { display: false } },
                        scales: { y: { beginAtZero: true } },
                    },
                });
            }

            // Table
            if (tools.length > 0) {
                const thead = '<thead><tr><th>Tool</th><th class="num">Calls</th><th class="num">Errors</th><th class="num">Err%</th>' +
                    '<th class="num">Overflows</th><th class="num">Ovf%</th><th class="num">p50</th><th class="num">p99</th></tr></thead>';
                const rows = tools.map(t =>
                    '<tr><td>' + esc(t.tool) + '</td><td class="num">' + t.calls +
                    '</td><td class="num">' + t.errors +
                    '</td><td class="num">' + t.error_rate_pct.toFixed(1) + '%' +
                    '</td><td class="num">' + t.overflows +
                    '</td><td class="num">' + t.overflow_rate_pct.toFixed(1) + '%' +
                    '</td><td class="num">' + t.p50_ms + 'ms' +
                    '</td><td class="num">' + t.p99_ms + 'ms</td></tr>'
                ).join('');
                document.getElementById('usage-table').innerHTML =
                    '<table>' + thead + '<tbody>' + rows + '</tbody></table>';
            } else {
                document.getElementById('usage-table').innerHTML =
                    '<p class="muted">No tool calls in this window.</p>';
            }
        } else {
            const reason = (usage && usage.reason) || 'No usage data available.';
            document.getElementById('usage-summary').innerHTML =
                '<p class="muted">' + esc(reason) + '</p>';
            document.getElementById('usage-table').innerHTML = '';
        }

        lastErrors = (errors && errors.available) ? (errors.errors || []) : [];
        renderErrors();

        // LSP startup section
        const fmtMs = ms => ms == null ? '—' :
            ms >= 1000 ? (ms / 1000).toFixed(1) + 's' : ms + 'ms';

        if (lsp && lsp.available) {
            const langs = lsp.by_language || [];
            if (langs.length > 0) {
                const thead = '<thead><tr><th>Language</th><th class="num">Starts</th><th class="num">Failures</th><th>Reasons</th>' +
                    '<th class="num">Avg handshake</th><th class="num">p95 handshake</th>' +
                    '<th class="num">Avg first resp</th><th class="num">p95 first resp</th></tr></thead>';
                const rows = langs.map(l => {
                    const r = l.reasons || {};
                    const badges = [
                        r.new_session    ? r.new_session    + ' new'     : '',
                        r.idle_evicted   ? r.idle_evicted   + ' evicted' : '',
                        r.lru_evicted    ? r.lru_evicted    + ' lru'     : '',
                        r.crashed        ? r.crashed        + ' crash'   : '',
                    ].filter(Boolean).join(' · ');
                    return '<tr><td>' + esc(l.language) + '</td>' +
                        '<td class="num">' + esc(String(l.starts)) + '</td>' +
                        '<td class="num">' + (l.failures ? '<span style="color:var(--err,#c0392b)">' + esc(String(l.failures)) + '</span>' : '0') + '</td>' +
                        '<td>' + esc(badges) + '</td>' +
                        '<td class="num">' + fmtMs(l.avg_handshake_ms) + '</td>' +
                        '<td class="num">' + fmtMs(l.p95_handshake_ms) + '</td>' +
                        '<td class="num">' + fmtMs(l.avg_first_response_ms) + '</td>' +
                        '<td class="num">' + fmtMs(l.p95_first_response_ms) + '</td></tr>';
                }).join('');
                document.getElementById('lsp-table').innerHTML =
                    '<table>' + thead + '<tbody>' + rows + '</tbody></table>';
            } else {
                document.getElementById('lsp-table').innerHTML =
                    '<p class="muted">No LSP startup events in this window.</p>';
            }

            // Recent events (not window-filtered — always shows the most recent cold starts)
            const recent = lsp.recent || [];
            if (recent.length > 0) {
                const items = recent.map(e => {
                    const firstResp = e.first_response_ms != null
                        ? ' · first resp ' + fmtMs(e.first_response_ms) : '';
                    return '<li>[' + esc(e.language) + '] ' + esc(e.reason) +
                        ' · handshake ' + esc(String(e.handshake_ms)) + 'ms' +
                        firstResp +
                        ' · <span class="muted">' + esc(e.started_at) + '</span></li>';
                }).join('');
                document.getElementById('lsp-recent').innerHTML = '<ul>' + items + '</ul>';
            } else {
                document.getElementById('lsp-recent').innerHTML =
                    '<p class="muted">No recent LSP events.</p>';
            }

            // Failed starts — servers that died during initialize (e.g. an expired build)
            const failures = lsp.recent_failures || [];
            if (failures.length > 0) {
                const fitems = failures.map(f => {
                    const err = f.error ? ' · ' + esc(f.error) : '';
                    return '<li>[' + esc(f.language) + '] ' + esc(f.reason) + err +
                        ' · <span class="muted">' + esc(f.started_at) + '</span></li>';
                }).join('');
                document.getElementById('lsp-failures').innerHTML = '<ul>' + fitems + '</ul>';
            } else {
                document.getElementById('lsp-failures').innerHTML =
                    '<p class="muted">No failed LSP starts.</p>';
            }
        } else {
            document.getElementById('lsp-table').innerHTML =
                '<p class="muted">' + esc((lsp && lsp.reason) || 'No LSP data available.') + '</p>';
            document.getElementById('lsp-recent').innerHTML = '';
            document.getElementById('lsp-failures').innerHTML = '';
        }
    }

    document.getElementById('stats-window').addEventListener('change', refreshStats);
    document.getElementById('errors-search').addEventListener('input', renderErrors);
    document.getElementById('errors-collapse').addEventListener('change', renderErrors);

    // --- Memories page ---
    let currentTopic = null;

    async function refreshMemories() {
        const data = await fetchJson('/api/memories');
        const topics = (data && data.topics) || [];
        const list = document.getElementById('memory-topics');
        list.innerHTML = topics.map(t =>
            '<li class="' + (t === currentTopic ? 'active' : '') +
            '" data-topic="' + esc(t) + '">' + esc(t) + '</li>'
        ).join('');

        list.querySelectorAll('li').forEach(li => {
            li.addEventListener('click', () => loadMemory(li.dataset.topic));
        });
    }

    async function loadMemory(topic) {
        currentTopic = topic;
        const data = await fetchJson('/api/memories/' + encodeURIComponent(topic));
        if (data && data.content !== undefined) {
            document.getElementById('memory-viewer').innerHTML =
                '<h3>' + esc(topic) + '</h3>' +
                '<pre>' + esc(data.content) + '</pre>' +
                '<div style="margin-top:1rem">' +
                '<button class="btn btn-danger" id="delete-btn">Delete</button>' +
                '</div>';
            document.getElementById('delete-btn').addEventListener('click', () => deleteMemory(topic));
        }
        refreshMemories();
    }

    async function deleteMemory(topic) {
        if (!confirm('Delete memory "' + topic + '"?')) return;
        await fetch('/api/memories/' + encodeURIComponent(topic), { method: 'DELETE' });
        currentTopic = null;
        document.getElementById('memory-viewer').innerHTML = '<p class="muted">Deleted.</p>';
        refreshMemories();
    }

    // --- Polling ---
    async function refreshAll() {
        const page = activePage();
        if (page === 'overview') await refreshOverview();
        else if (page === 'stats') await refreshStats();
        else if (page === 'memories') await refreshMemories();
        document.getElementById('last-refresh').textContent =
            'Last refreshed: ' + new Date().toLocaleTimeString();
    }

    // Initial load: populate all pages so switching tabs feels instant
    Promise.all([refreshOverview(), refreshStats(), refreshMemories()]).then(() => {
        document.getElementById('last-refresh').textContent =
            'Last refreshed: ' + new Date().toLocaleTimeString();
    });
    setInterval(refreshAll, POLL_INTERVAL);
})();
