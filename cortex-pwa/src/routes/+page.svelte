<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { CortexClient, type ConnectionState, type ExecuteResult } from '$lib/cortex-client';

	// ── Shared connState ──────────────────────────────────────────────────────────
	let connState: ConnectionState = $connState('disconnected');
	let client: CortexClient;
	let running = $connState(false);
	let error = $connState('');
	let output = $connState('');

	// ── File demo ─────────────────────────────────────────────────────────────
	let filePath = $connState('C:/Users/Phillip/test.txt');

	// ── GPS Timesheet ─────────────────────────────────────────────────────────
	type LocationEntry = { lat: number; lng: number; acc: number; ts: string };
	type SyncStatus = 'idle' | 'syncing' | 'synced';
	let gpsTracking = $connState(false);
	let gpsEntries = $connState([] as LocationEntry[]);
	let gpsOutput = $connState('');
	let gpsError = $connState('');
	let watchId = $connState(null as number | null);
	let bgScheduled = $connState(false);
	let onlineStatus = $connState(true);
	let syncStatus = $connState('idle' as SyncStatus);

	onMount(() => {
		client = new CortexClient((s) => { connState = s; });
		tryConnect();
		onlineStatus = navigator.onLine;
		window.addEventListener('online', handleOnline);
		window.addEventListener('offline', handleOffline);
	});

	onDestroy(() => {
		client?.disconnect();
		if (watchId !== null) navigator.geolocation.clearWatch(watchId);
		window.removeEventListener('online', handleOnline);
		window.removeEventListener('offline', handleOffline);
	});

	async function tryConnect() {
		error = '';
		try { await client.connect(); } catch { /* show banner */ }
	}

	// ── File demo ─────────────────────────────────────────────────────────────

	async function readFile() {
		running = true; error = ''; output = '';
		const safePath = filePath.replace(/\\/g, '/');
		const script = `task readFile() -> void {
    let content: string = native.fs.read("${safePath}");
    native.notify("File Contents", content);
    native.log(content);
}`;
		try {
			const result: ExecuteResult = await client.execute(script, {
				allow: ['native.fs.read', 'native.notify', 'native.log']
			});
			output = result.logs.join('\n');
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally { running = false; }
	}

	async function runMathDemo() {
		running = true; error = ''; output = '';
		const script = `task demo() -> void {
    let a: i32 = 6;
    let b: i32 = 7;
    native.log("6 * 7 =");
    native.log(a * b);
}`;
		try {
			const result = await client.execute(script, { allow: ['native.log'] });
			output = result.logs.join('\n');
		} catch (e) {
			error = e instanceof Error ? e.message : String(e);
		} finally { running = false; }
	}

	// ── GPS Timesheet ─────────────────────────────────────────────────────────

	function escapeForCortexString(s: string): string {
		return s.replace(/\\/g, '\\\\').replace(/"/g, '\\"');
	}

	async function appendLocationToDB(lat: number, lng: number, acc: number) {
		const entry: LocationEntry = {
			lat: parseFloat(lat.toFixed(6)),
			lng: parseFloat(lng.toFixed(6)),
			acc: parseFloat(acc.toFixed(1)),
			ts: new Date().toISOString()
		};
		const safe = escapeForCortexString(JSON.stringify(entry));
		const script = `task log() -> void {
    native.db.append("locations", "${safe}");
    native.log("location logged");
}`;
		try {
			await client.execute(script, { allow: ['native.db.append', 'native.log'] });
			gpsEntries = [entry, ...gpsEntries].slice(0, 20);
		} catch (e) {
			gpsError = e instanceof Error ? e.message : String(e);
		}
	}

	function startTracking() {
		if (!navigator.geolocation) {
			gpsError = 'Geolocation not available in this browser';
			return;
		}
		gpsError = '';
		gpsTracking = true;
		const id = navigator.geolocation.watchPosition(
			(pos) => {
				if (connState === 'connected') {
					appendLocationToDB(pos.coords.latitude, pos.coords.longitude, pos.coords.accuracy);
				}
			},
			(err) => {
				gpsError = `Geolocation error: ${err.message}`;
				gpsTracking = false;
				watchId = null;
			},
			{ enableHighAccuracy: true, timeout: 15000, maximumAge: 0 }
		);
		watchId = id;
	}

	function stopTracking() {
		if (watchId !== null) {
			navigator.geolocation.clearWatch(watchId);
			watchId = null;
		}
		gpsTracking = false;
	}

	async function viewLog() {
		gpsOutput = ''; gpsError = '';
		const script = `task query() -> void {
    let rows: string = native.db.query("locations");
    native.log(rows);
}`;
		try {
			const result = await client.execute(script, { allow: ['native.db.query', 'native.log'] });
			const raw = result.logs.join('');
			try {
				const parsed: string[] = JSON.parse(raw);
				gpsEntries = parsed
					.map((r) => { try { return JSON.parse(r) as LocationEntry; } catch { return null; } })
					.filter((e): e is LocationEntry => e !== null)
					.reverse()
					.slice(0, 20);
				gpsOutput = `${parsed.length} entries in database`;
			} catch {
				gpsOutput = raw || 'No entries yet';
			}
		} catch (e) {
			gpsError = e instanceof Error ? e.message : String(e);
		}
	}

	async function scheduleBackground() {
		gpsError = '';
		const innerScript = `task tick() -> void { let loc: string = native.location.current(); native.db.append("locations", loc); native.log("ticked"); }`;
		const escaped = escapeForCortexString(innerScript);
		const script = `task start() -> void {
    native.schedule.add("*/30 * * * * *", "${escaped}");
    native.log("background GPS tracking started");
}`;
		try {
			const result = await client.execute(script, { allow: ['native.schedule.add', 'native.log'] });
			bgScheduled = true;
			gpsOutput = result.logs.join('\n') || 'Background tracking scheduled (every 30s)';
		} catch (e) {
			gpsError = e instanceof Error ? e.message : String(e);
		}
	}

	function handleOnline() {
		onlineStatus = true;
		if (connState === 'connected') {
			syncStatus = 'syncing';
			viewLog().then(() => {
				syncStatus = 'synced';
				setTimeout(() => { syncStatus = 'idle'; }, 3000);
			});
		}
	}

	function handleOffline() {
		onlineStatus = false;
		syncStatus = 'idle';
	}

	function formatTs(ts: string): string {
		try { return new Date(ts).toLocaleTimeString(); } catch { return ts; }
	}
</script>

<svelte:head>
	<title>TPT Cortex Demo</title>
</svelte:head>

<main>
	<header>
		<div class="logo">
			<span class="logo-icon">⬡</span>
			<span class="logo-text">TPT Cortex</span>
		</div>
		<div class="status" class:connected={connState === 'connected'} class:connecting={connState === 'connecting'}>
			<span class="dot"></span>
			{connState === 'connected' ? 'Core connected' : connState === 'connecting' ? 'Connecting…' : 'Core not detected'}
		</div>
	</header>

	{#if connState === 'disconnected'}
		<div class="banner">
			<p class="banner-title">TPT Core not detected</p>
			<p class="banner-body">
				Start the daemon to unlock native file access, notifications, and background APIs.
			</p>
			<code class="banner-code">go run ./cortex-daemon</code>
			<button class="btn-secondary" onclick={tryConnect}>Retry connection</button>
		</div>
	{/if}

	<!-- ── Native File Read ── -->
	<section class="card">
		<h2>Native File Read</h2>
		<p class="card-desc">Sends a Cortex script to the daemon that reads a local file and fires a desktop notification.</p>
		<label>
			<span>File path</span>
			<input bind:value={filePath} disabled={connState !== 'connected'} placeholder="C:\Users\you\test.txt" />
		</label>
		<button class="btn-primary" onclick={readFile} disabled={connState !== 'connected' || running}>
			{running ? 'Running…' : 'Read File + Notify'}
		</button>
	</section>

	<!-- ── Math Demo ── -->
	<section class="card">
		<h2>Math Demo</h2>
		<p class="card-desc">Pure-logic Cortex script — no native permissions required.</p>
		<button class="btn-primary" onclick={runMathDemo} disabled={connState !== 'connected' || running}>
			{running ? 'Running…' : 'Run 6 × 7'}
		</button>
	</section>

	{#if output}
		<section class="card output-card">
			<h2>Output</h2>
			<pre>{output}</pre>
		</section>
	{/if}

	{#if error}
		<section class="card error-card">
			<h2>Error</h2>
			<pre>{error}</pre>
		</section>
	{/if}

	<!-- ── GPS Timesheet ── -->
	<section class="card gps-card">
		<div class="gps-header">
			<h2>GPS Timesheet</h2>
			<div class="net-badge" class:online={onlineStatus} class:syncing={syncStatus === 'syncing'} class:synced={syncStatus === 'synced'}>
				<span class="dot"></span>
				{syncStatus === 'syncing' ? 'Syncing…' : syncStatus === 'synced' ? 'Synced' : onlineStatus ? 'Online' : 'Offline'}
			</div>
		</div>
		<p class="card-desc">
			Captures GPS fixes via the browser, persists each entry to SQLite via a Cortex script,
			and auto-syncs on reconnect. Schedules a background daemon task that runs every 30 s
			using <code>native.location.current()</code> — survives lock screen on Android.
		</p>

		<div class="gps-actions">
			{#if !gpsTracking}
				<button class="btn-primary" onclick={startTracking} disabled={connState !== 'connected'}>
					Start Tracking
				</button>
			{:else}
				<button class="btn-danger" onclick={stopTracking}>
					Stop Tracking
				</button>
			{/if}
			<button class="btn-secondary" onclick={viewLog} disabled={connState !== 'connected'}>
				View DB Log
			</button>
			<button
				class="btn-secondary"
				class:scheduled={bgScheduled}
				onclick={scheduleBackground}
				disabled={connState !== 'connected' || bgScheduled}
			>
				{bgScheduled ? 'Background Active' : 'Schedule Background'}
			</button>
		</div>

		{#if gpsTracking}
			<p class="gps-live">
				<span class="pulse"></span>
				Tracking active — {gpsEntries.length} fix{gpsEntries.length === 1 ? '' : 'es'} this session
			</p>
		{/if}

		{#if gpsOutput}
			<p class="gps-status">{gpsOutput}</p>
		{/if}

		{#if gpsError}
			<p class="gps-err">{gpsError}</p>
		{/if}

		{#if gpsEntries.length > 0}
			<div class="entry-table">
				<div class="entry-row entry-head">
					<span>Time</span>
					<span>Lat</span>
					<span>Lng</span>
					<span>Acc</span>
				</div>
				{#each gpsEntries.slice(0, 8) as e}
					<div class="entry-row">
						<span>{formatTs(e.ts)}</span>
						<span>{e.lat.toFixed(5)}</span>
						<span>{e.lng.toFixed(5)}</span>
						<span>{e.acc}m</span>
					</div>
				{/each}
				{#if gpsEntries.length > 8}
					<p class="entry-more">+{gpsEntries.length - 8} more entries in DB</p>
				{/if}
			</div>
		{/if}
	</section>
</main>

<style>
	:global(*, *::before, *::after) { box-sizing: border-box; margin: 0; padding: 0; }
	:global(body) {
		background: #0d0d0f;
		color: #e2e2e6;
		font-family: 'Inter', system-ui, sans-serif;
		min-height: 100vh;
	}

	main {
		max-width: 640px;
		margin: 0 auto;
		padding: 2rem 1.5rem;
		display: flex;
		flex-direction: column;
		gap: 1.25rem;
	}

	header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding-bottom: 1rem;
		border-bottom: 1px solid #1e1e24;
	}

	.logo { display: flex; align-items: center; gap: 0.5rem; }
	.logo-icon { font-size: 1.5rem; }
	.logo-text { font-size: 1.2rem; font-weight: 700; letter-spacing: -0.02em; color: #fff; }

	.status {
		display: flex; align-items: center; gap: 0.4rem;
		font-size: 0.8rem; color: #6b6b7a;
	}
	.status .dot { width: 7px; height: 7px; border-radius: 50%; background: #444; }
	.status.connected { color: #4ade80; }
	.status.connected .dot { background: #4ade80; box-shadow: 0 0 6px #4ade80; }
	.status.connecting { color: #facc15; }
	.status.connecting .dot { background: #facc15; }

	.banner {
		border: 1px solid #2a2030; border-radius: 10px; background: #13101a;
		padding: 1.25rem 1.5rem; display: flex; flex-direction: column; gap: 0.6rem;
	}
	.banner-title { font-weight: 600; color: #c084fc; }
	.banner-body { font-size: 0.85rem; color: #9090a0; }
	.banner-code {
		font-family: monospace; font-size: 0.85rem;
		background: #1a1a22; color: #a5f3fc;
		padding: 0.5rem 0.75rem; border-radius: 6px;
	}

	.card {
		background: #111117; border: 1px solid #1e1e26;
		border-radius: 12px; padding: 1.25rem 1.5rem;
		display: flex; flex-direction: column; gap: 0.75rem;
	}

	h2 { font-size: 0.95rem; font-weight: 600; color: #fff; }
	.card-desc { font-size: 0.82rem; color: #6b6b7a; line-height: 1.5; }
	.card-desc code { font-family: monospace; color: #a5f3fc; font-size: 0.8rem; }

	label { display: flex; flex-direction: column; gap: 0.3rem; font-size: 0.8rem; color: #9090a0; }
	input {
		background: #0d0d10; border: 1px solid #2a2a34; border-radius: 7px;
		color: #e2e2e6; padding: 0.5rem 0.75rem;
		font-size: 0.875rem; font-family: monospace; outline: none;
		transition: border-color 0.15s;
	}
	input:focus { border-color: #6366f1; }
	input:disabled { opacity: 0.4; }

	.btn-primary, .btn-secondary, .btn-danger {
		border: none; border-radius: 7px; padding: 0.55rem 1.1rem;
		font-size: 0.875rem; font-weight: 500;
		cursor: pointer; transition: opacity 0.15s;
	}
	.btn-primary { background: #6366f1; color: #fff; }
	.btn-primary:hover:not(:disabled) { opacity: 0.85; }
	.btn-primary:disabled { opacity: 0.35; cursor: default; }
	.btn-secondary { background: #1e1e28; color: #c0c0d0; }
	.btn-secondary:hover:not(:disabled) { opacity: 0.8; }
	.btn-secondary:disabled { opacity: 0.35; cursor: default; }
	.btn-secondary.scheduled { background: #1a2a1e; color: #4ade80; }
	.btn-danger { background: #3d1515; color: #f87171; }
	.btn-danger:hover { opacity: 0.85; }

	.output-card pre {
		font-family: monospace; font-size: 0.82rem; color: #a5f3fc;
		background: #0a0a0f; padding: 0.75rem; border-radius: 6px;
		white-space: pre-wrap; word-break: break-all;
	}
	.error-card { border-color: #3d1515; }
	.error-card h2 { color: #f87171; }
	.error-card pre {
		font-family: monospace; font-size: 0.82rem; color: #f87171;
		background: #140808; padding: 0.75rem; border-radius: 6px;
		white-space: pre-wrap;
	}

	/* ── GPS card ── */
	.gps-card { border-color: #1e2630; }

	.gps-header { display: flex; align-items: center; justify-content: space-between; }

	.net-badge {
		display: flex; align-items: center; gap: 0.35rem;
		font-size: 0.75rem; color: #555566; padding: 0.25rem 0.6rem;
		border-radius: 20px; background: #0f0f14; border: 1px solid #1e1e28;
	}
	.net-badge .dot { width: 6px; height: 6px; border-radius: 50%; background: #444; }
	.net-badge.online { color: #4ade80; border-color: #1a2e1e; }
	.net-badge.online .dot { background: #4ade80; }
	.net-badge.syncing { color: #facc15; border-color: #2a2710; }
	.net-badge.syncing .dot { background: #facc15; animation: blink 0.8s step-start infinite; }
	.net-badge.synced { color: #60a5fa; border-color: #101830; }
	.net-badge.synced .dot { background: #60a5fa; }

	.gps-actions { display: flex; flex-wrap: wrap; gap: 0.5rem; }

	.gps-live {
		display: flex; align-items: center; gap: 0.5rem;
		font-size: 0.82rem; color: #4ade80;
	}
	.pulse {
		width: 8px; height: 8px; border-radius: 50%; background: #4ade80;
		animation: pulse 1.4s ease-in-out infinite;
	}

	.gps-status { font-size: 0.82rem; color: #9090a0; }
	.gps-err { font-size: 0.82rem; color: #f87171; }

	.entry-table {
		display: flex; flex-direction: column; gap: 0;
		border: 1px solid #1e1e2a; border-radius: 8px; overflow: hidden;
		font-size: 0.78rem; font-family: monospace;
	}
	.entry-row {
		display: grid; grid-template-columns: 1fr 1.2fr 1.2fr 0.6fr;
		gap: 0.5rem; padding: 0.4rem 0.75rem;
		border-bottom: 1px solid #16161e;
	}
	.entry-row:last-child { border-bottom: none; }
	.entry-head { color: #555566; font-size: 0.72rem; background: #0d0d12; }
	.entry-row:not(.entry-head) { color: #a5f3fc; }
	.entry-more {
		font-size: 0.75rem; color: #555566; text-align: center;
		padding: 0.4rem; background: #0d0d12;
	}

	@keyframes pulse {
		0%, 100% { opacity: 1; transform: scale(1); }
		50% { opacity: 0.4; transform: scale(1.4); }
	}
	@keyframes blink {
		50% { opacity: 0; }
	}
</style>
