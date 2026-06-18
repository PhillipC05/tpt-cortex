<script lang="ts">
	import { onMount, onDestroy } from 'svelte';
	import { CortexClient, type ConnectionState, type ExecuteResult } from '$lib/cortex-client';

	// ── Types ─────────────────────────────────────────────────────────────────
	type NodeId = string;
	type PortId = 'out' | 'left' | 'right' | 'value' | 'in';

	interface NodeDef {
		id: NodeId;
		type: string;
		x: number;
		y: number;
		constI32?: number;
		constF64?: number;
		constBool?: boolean;
		constStr?: string;
		op?: string;
	}

	interface Edge {
		from: NodeId;
		fromPort: 'out';
		to: NodeId;
		toPort: PortId;
	}

	// ── Constants ─────────────────────────────────────────────────────────────
	const NODE_WIDTH = 180;

	const NODE_HEIGHTS: Record<string, number> = {
		const_i32: 80,
		const_f64: 80,
		const_bool: 80,
		const_str: 80,
		binop: 110,
		negate: 70,
		not: 70,
		native_log: 70,
		result: 60,
	};

	const INPUT_PORTS: Record<string, PortId[]> = {
		const_i32: [],
		const_f64: [],
		const_bool: [],
		const_str: [],
		binop: ['left', 'right'],
		negate: ['value'],
		not: ['value'],
		native_log: ['value'],
		result: ['value'],
	};

	const TYPE_COLORS: Record<string, string> = {
		const_i32: '#22d3ee',
		const_f64: '#22d3ee',
		const_bool: '#22d3ee',
		const_str: '#22d3ee',
		binop: '#c084fc',
		negate: '#c084fc',
		not: '#c084fc',
		native_log: '#4ade80',
		result: '#fb923c',
	};

	const NODE_LABELS: Record<string, string> = {
		const_i32: 'const i32',
		const_f64: 'const f64',
		const_bool: 'const bool',
		const_str: 'const string',
		binop: 'binary op',
		negate: 'negate',
		not: 'not',
		native_log: 'native.log',
		result: 'result',
	};

	const PALETTE_TYPES = ['const_i32', 'const_f64', 'const_bool', 'const_str', 'binop', 'negate', 'not', 'native_log'];

	// ── State ─────────────────────────────────────────────────────────────────
	let nextId = $state(10);
	function genId(): NodeId { return `n${nextId++}`; }

	let nodes = $state<NodeDef[]>([
		{ id: 'n1', type: 'const_i32', x: 100, y: 100, constI32: 6 },
		{ id: 'n2', type: 'const_i32', x: 100, y: 240, constI32: 7 },
		{ id: 'n3', type: 'binop', x: 350, y: 160, op: '*' },
		{ id: 'n4', type: 'result', x: 570, y: 170 },
	]);

	let edges = $state<Edge[]>([
		{ from: 'n1', fromPort: 'out', to: 'n3', toPort: 'left' },
		{ from: 'n2', fromPort: 'out', to: 'n3', toPort: 'right' },
		{ from: 'n3', fromPort: 'out', to: 'n4', toPort: 'value' },
	]);

	// Port selection state
	let selectedOutput = $state<{ nodeId: NodeId; port: 'out' } | null>(null);

	// Drag state
	let dragging = $state<{ nodeId: NodeId; startX: number; startY: number; origX: number; origY: number } | null>(null);

	// Task config
	let taskName = $state('myTask');
	let returnType = $state('i32');

	// Generated code
	let generatedCode = $state('');
	let codeError = $state('');

	// Client
	let client: CortexClient;
	let connState: ConnectionState = $state('disconnected');
	let runOutput = $state('');
	let runError = $state('');
	let running = $state(false);

	// SVG canvas ref
	let svgEl = $state<SVGSVGElement | null>(null);

	onMount(() => {
		client = new CortexClient((s) => { connState = s; });
		client.connect().catch(() => {});
	});

	onDestroy(() => {
		client?.disconnect();
	});

	// ── Port position helpers ─────────────────────────────────────────────────
	function nodeHeight(n: NodeDef): number {
		return NODE_HEIGHTS[n.type] ?? 80;
	}

	function outputPortPos(n: NodeDef): { x: number; y: number } {
		return { x: n.x + NODE_WIDTH, y: n.y + nodeHeight(n) / 2 };
	}

	function inputPortPos(n: NodeDef, port: PortId): { x: number; y: number } {
		const ports = INPUT_PORTS[n.type] ?? [];
		const idx = ports.indexOf(port);
		const count = ports.length;
		const h = nodeHeight(n);
		if (count === 0) return { x: n.x, y: n.y + h / 2 };
		const step = h / (count + 1);
		return { x: n.x, y: n.y + step * (idx + 1) };
	}

	// ── Edge path ─────────────────────────────────────────────────────────────
	function edgePath(e: Edge): string {
		const fromNode = nodes.find(n => n.id === e.from);
		const toNode = nodes.find(n => n.id === e.to);
		if (!fromNode || !toNode) return '';
		const from = outputPortPos(fromNode);
		const to = inputPortPos(toNode, e.toPort);
		const dx = Math.abs(to.x - from.x) * 0.5;
		return `M ${from.x} ${from.y} C ${from.x + dx} ${from.y}, ${to.x - dx} ${to.y}, ${to.x} ${to.y}`;
	}

	// ── Interaction ───────────────────────────────────────────────────────────
	function clickOutputPort(nodeId: NodeId, e: MouseEvent) {
		e.stopPropagation();
		selectedOutput = { nodeId, port: 'out' };
	}

	function clickInputPort(nodeId: NodeId, port: PortId, e: MouseEvent) {
		e.stopPropagation();
		if (!selectedOutput) return;
		// Remove existing edge to this input
		edges = edges.filter(ed => !(ed.to === nodeId && ed.toPort === port));
		// Add new edge
		edges = [...edges, { from: selectedOutput.nodeId, fromPort: 'out', to: nodeId, toPort: port }];
		selectedOutput = null;
	}

	function clickCanvas() {
		selectedOutput = null;
	}

	function deleteNode(id: NodeId, e: MouseEvent) {
		e.stopPropagation();
		nodes = nodes.filter(n => n.id !== id);
		edges = edges.filter(ed => ed.from !== id && ed.to !== id);
	}

	function addNode(type: string) {
		const id = genId();
		const defaults: Partial<NodeDef> = {};
		if (type === 'const_i32') defaults.constI32 = 0;
		if (type === 'const_f64') defaults.constF64 = 0.0;
		if (type === 'const_bool') defaults.constBool = false;
		if (type === 'const_str') defaults.constStr = '';
		if (type === 'binop') defaults.op = '+';
		nodes = [...nodes, { id, type, x: 200, y: 200, ...defaults }];
	}

	// ── Dragging ──────────────────────────────────────────────────────────────
	function startDrag(nodeId: NodeId, e: MouseEvent) {
		// Don't drag if clicking on port, input, button
		const target = e.target as HTMLElement;
		if (target.closest('.port') || target.closest('input') || target.closest('select') || target.closest('button') || target.closest('label')) return;
		e.preventDefault();
		const node = nodes.find(n => n.id === nodeId)!;
		dragging = { nodeId, startX: e.clientX, startY: e.clientY, origX: node.x, origY: node.y };
	}

	function onMouseMove(e: MouseEvent) {
		if (!dragging) return;
		const dx = e.clientX - dragging.startX;
		const dy = e.clientY - dragging.startY;
		nodes = nodes.map(n =>
			n.id === dragging!.nodeId
				? { ...n, x: dragging!.origX + dx, y: dragging!.origY + dy }
				: n
		);
	}

	function onMouseUp() {
		dragging = null;
	}

	// ── Code generation ───────────────────────────────────────────────────────
	function resolveExpr(nodeId: NodeId, visited = new Set<NodeId>()): string {
		if (visited.has(nodeId)) return '/* cycle */';
		visited.add(nodeId);
		const node = nodes.find(n => n.id === nodeId);
		if (!node) return '/* missing */';

		const getInput = (port: PortId): string => {
			const edge = edges.find(e => e.to === nodeId && e.toPort === port);
			if (!edge) return '/* unconnected */';
			return resolveExpr(edge.from, new Set(visited));
		};

		switch (node.type) {
			case 'const_i32':
				return String(node.constI32 ?? 0);
			case 'const_f64': {
				const v = node.constF64 ?? 0.0;
				return Number.isInteger(v) ? v.toFixed(1) : String(v);
			}
			case 'const_bool':
				return node.constBool ? 'true' : 'false';
			case 'const_str':
				return `"${(node.constStr ?? '').replace(/\\/g, '\\\\').replace(/"/g, '\\"')}"`;
			case 'binop': {
				const left = getInput('left');
				const right = getInput('right');
				return `(${left} ${node.op ?? '+'} ${right})`;
			}
			case 'negate': {
				const val = getInput('value');
				return `(0 - ${val})`;
			}
			case 'not': {
				const val = getInput('value');
				return `!${val}`;
			}
			case 'native_log': {
				const val = getInput('value');
				return `native.log(${val})`;
			}
			default:
				return '/* unknown */';
		}
	}

	function generateCode() {
		codeError = '';
		const resultNode = nodes.find(n => n.type === 'result');
		if (!resultNode) { codeError = 'No result node found.'; return; }

		const valueEdge = edges.find(e => e.to === resultNode.id && e.toPort === 'value');
		if (!valueEdge) { codeError = 'Result node has no input connected.'; return; }

		const expr = resolveExpr(valueEdge.from);
		generatedCode = `task ${taskName}() -> ${returnType} {\n    return ${expr};\n}`;
	}

	async function runCode() {
		if (!generatedCode) generateCode();
		if (!generatedCode) return;
		running = true;
		runError = '';
		runOutput = '';
		try {
			const result: ExecuteResult = await client.execute(generatedCode, { allow: ['native.log'] });
			runOutput = result.logs.join('\n') || '(no output)';
		} catch (e) {
			runError = e instanceof Error ? e.message : String(e);
		} finally {
			running = false;
		}
	}

	async function copyCode() {
		if (generatedCode) await navigator.clipboard.writeText(generatedCode);
	}

	// ── In-progress edge (mouse tracking) ─────────────────────────────────────
	let mousePos = $state({ x: 0, y: 0 });

	function onSvgMouseMove(e: MouseEvent) {
		if (!svgEl) return;
		const rect = svgEl.getBoundingClientRect();
		mousePos = { x: e.clientX - rect.left, y: e.clientY - rect.top };
		onMouseMove(e);
	}

	function pendingEdgePath(): string {
		if (!selectedOutput || !svgEl) return '';
		const fromNode = nodes.find(n => n.id === selectedOutput!.nodeId);
		if (!fromNode) return '';
		const from = outputPortPos(fromNode);
		const to = mousePos;
		const dx = Math.abs(to.x - from.x) * 0.5;
		return `M ${from.x} ${from.y} C ${from.x + dx} ${from.y}, ${to.x - dx} ${to.y}, ${to.x} ${to.y}`;
	}
</script>

<svelte:window onmousemove={onMouseMove} onmouseup={onMouseUp} />

<svelte:head>
	<title>Node Editor — TPT Cortex</title>
</svelte:head>

<div class="page">
	<!-- ── Sidebar ── -->
	<aside class="sidebar">
		<div class="sidebar-header">
			<a href="/" class="back-link">← Back</a>
			<h1>Node Editor</h1>
		</div>

		<section class="sidebar-section">
			<h2>Task config</h2>
			<label class="field">
				<span>Name</span>
				<input bind:value={taskName} spellcheck="false" />
			</label>
			<label class="field">
				<span>Return type</span>
				<select bind:value={returnType}>
					<option value="i32">i32</option>
					<option value="f64">f64</option>
					<option value="bool">bool</option>
					<option value="string">string</option>
					<option value="void">void</option>
				</select>
			</label>
		</section>

		<section class="sidebar-section">
			<h2>Add node</h2>
			<div class="palette">
				{#each PALETTE_TYPES as type}
					<button class="palette-btn" style="border-left-color: {TYPE_COLORS[type]}" onclick={() => addNode(type)}>
						{NODE_LABELS[type]}
					</button>
				{/each}
			</div>
		</section>

		<section class="sidebar-section">
			<h2>Actions</h2>
			<div class="action-btns">
				<button class="btn-primary" onclick={generateCode}>Generate Code</button>
				{#if generatedCode}
					<button class="btn-secondary" onclick={copyCode}>Copy</button>
					<button class="btn-secondary" onclick={runCode} disabled={connState !== 'connected' || running}>
						{running ? 'Running…' : 'Run'}
					</button>
				{/if}
			</div>
			<div class="conn-status" class:connected={connState === 'connected'}>
				<span class="dot"></span>
				{connState === 'connected' ? 'Daemon connected' : connState === 'connecting' ? 'Connecting…' : 'Daemon offline'}
			</div>
		</section>

		{#if codeError}
			<div class="err-banner">{codeError}</div>
		{/if}
	</aside>

	<!-- ── Main canvas area ── -->
	<div class="canvas-area">
		<!-- SVG Canvas -->
		<!-- svelte-ignore a11y_click_events_have_key_events -->
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<svg
			bind:this={svgEl}
			class="canvas"
			onmousemove={onSvgMouseMove}
			onmouseup={onMouseUp}
			onclick={clickCanvas}
		>
			<!-- Dot grid pattern -->
			<defs>
				<pattern id="dotgrid" x="0" y="0" width="24" height="24" patternUnits="userSpaceOnUse">
					<circle cx="1" cy="1" r="0.8" fill="#1e1e2e" />
				</pattern>
			</defs>
			<rect width="100%" height="100%" fill="url(#dotgrid)" />

			<!-- Edges -->
			{#each edges as edge (edge.from + '-' + edge.fromPort + '-' + edge.to + '-' + edge.toPort)}
				<path
					d={edgePath(edge)}
					stroke="#6366f1"
					stroke-width="2"
					fill="none"
					stroke-linecap="round"
				/>
			{/each}

			<!-- Pending edge while connecting -->
			{#if selectedOutput}
				<path
					d={pendingEdgePath()}
					stroke="#facc15"
					stroke-width="2"
					fill="none"
					stroke-dasharray="5 4"
					stroke-linecap="round"
				/>
			{/if}

			<!-- Nodes -->
			{#each nodes as node (node.id)}
				{@const h = nodeHeight(node)}
				{@const color = TYPE_COLORS[node.type] ?? '#888'}
				{@const ports = INPUT_PORTS[node.type] ?? []}

				<foreignObject
					x={node.x}
					y={node.y}
					width={NODE_WIDTH}
					height={h}
					class="node-fo"
				>
					<!-- svelte-ignore a11y_no_static_element_interactions -->
					<div
						class="node-card"
						onmousedown={(e) => startDrag(node.id, e)}
						style="--node-color: {color}; height: {h}px;"
					>
						<div class="node-header">
							<span class="node-type-label" style="color: {color}">{NODE_LABELS[node.type] ?? node.type}</span>
							{#if node.type !== 'result'}
								<button class="delete-btn" onclick={(e) => deleteNode(node.id, e)}>×</button>
							{/if}
						</div>

						<!-- Config fields -->
						{#if node.type === 'const_i32'}
							<input
								class="node-input"
								type="number"
								bind:value={node.constI32}
								onclick={(e) => e.stopPropagation()}
							/>
						{:else if node.type === 'const_f64'}
							<input
								class="node-input"
								type="number"
								step="0.01"
								bind:value={node.constF64}
								onclick={(e) => e.stopPropagation()}
							/>
						{:else if node.type === 'const_bool'}
							<label class="node-checkbox" onclick={(e) => e.stopPropagation()}>
								<input type="checkbox" bind:checked={node.constBool} />
								<span>{node.constBool ? 'true' : 'false'}</span>
							</label>
						{:else if node.type === 'const_str'}
							<input
								class="node-input"
								type="text"
								bind:value={node.constStr}
								placeholder="value"
								onclick={(e) => e.stopPropagation()}
							/>
						{:else if node.type === 'binop'}
							<select class="node-select" bind:value={node.op} onclick={(e) => e.stopPropagation()}>
								{#each ['+', '-', '*', '/', '==', '!=', '<', '<=', '>', '>='] as op}
									<option value={op}>{op}</option>
								{/each}
							</select>
						{/if}
					</div>
				</foreignObject>

				<!-- Output port (right edge, vertically centered) -->
				{@const opPos = outputPortPos(node)}
				<!-- svelte-ignore a11y_click_events_have_key_events -->
				<circle
					class="port port-out"
					class:selected={selectedOutput?.nodeId === node.id}
					cx={opPos.x}
					cy={opPos.y}
					r="6"
					fill={selectedOutput?.nodeId === node.id ? '#facc15' : '#6366f1'}
					stroke="#0a0a0e"
					stroke-width="2"
					onclick={(e) => clickOutputPort(node.id, e)}
				/>

				<!-- Input ports (left edge) -->
				{#each ports as port}
					{@const ipPos = inputPortPos(node, port)}
					{@const hasEdge = edges.some(e => e.to === node.id && e.toPort === port)}
					<!-- svelte-ignore a11y_click_events_have_key_events -->
					<circle
						class="port port-in"
						cx={ipPos.x}
						cy={ipPos.y}
						r="6"
						fill={hasEdge ? '#6366f1' : '#2a2a3e'}
						stroke={selectedOutput ? '#facc15' : '#6366f1'}
						stroke-width="2"
						onclick={(e) => clickInputPort(node.id, port, e)}
					/>
					<!-- Port label -->
					<text
						x={ipPos.x + 10}
						y={ipPos.y + 4}
						font-size="9"
						fill="#6b6b7a"
						pointer-events="none"
					>{port}</text>
				{/each}
			{/each}
		</svg>

		<!-- Generated code panel -->
		{#if generatedCode || runOutput || runError}
			<div class="code-panel">
				{#if generatedCode}
					<div class="code-section">
						<div class="code-label">Generated Cortex code</div>
						<pre class="code-pre">{generatedCode}</pre>
					</div>
				{/if}
				{#if runOutput}
					<div class="code-section">
						<div class="code-label" style="color: #4ade80">Output</div>
						<pre class="code-pre output-pre">{runOutput}</pre>
					</div>
				{/if}
				{#if runError}
					<div class="code-section">
						<div class="code-label" style="color: #f87171">Error</div>
						<pre class="code-pre error-pre">{runError}</pre>
					</div>
				{/if}
			</div>
		{/if}
	</div>
</div>

<style>
	:global(*, *::before, *::after) { box-sizing: border-box; margin: 0; padding: 0; }
	:global(body) {
		background: #0d0d0f;
		color: #e2e2e6;
		font-family: 'Inter', system-ui, sans-serif;
		min-height: 100vh;
		overflow: hidden;
	}

	.page {
		display: flex;
		height: 100vh;
		overflow: hidden;
		background: #0d0d0f;
	}

	/* ── Sidebar ── */
	.sidebar {
		width: 250px;
		min-width: 250px;
		background: #111117;
		border-right: 1px solid #1e1e26;
		display: flex;
		flex-direction: column;
		gap: 0;
		overflow-y: auto;
		z-index: 10;
	}

	.sidebar-header {
		padding: 1rem;
		border-bottom: 1px solid #1e1e26;
		display: flex;
		flex-direction: column;
		gap: 0.4rem;
	}

	.back-link {
		font-size: 0.75rem;
		color: #6366f1;
		text-decoration: none;
	}
	.back-link:hover { text-decoration: underline; }

	.sidebar-header h1 {
		font-size: 1rem;
		font-weight: 700;
		color: #fff;
	}

	.sidebar-section {
		padding: 0.875rem 1rem;
		border-bottom: 1px solid #1a1a22;
		display: flex;
		flex-direction: column;
		gap: 0.5rem;
	}

	.sidebar-section h2 {
		font-size: 0.7rem;
		font-weight: 600;
		color: #555566;
		text-transform: uppercase;
		letter-spacing: 0.08em;
		margin-bottom: 0.1rem;
	}

	.field {
		display: flex;
		flex-direction: column;
		gap: 0.2rem;
		font-size: 0.78rem;
		color: #9090a0;
	}

	.field input, .field select {
		background: #0d0d10;
		border: 1px solid #2a2a34;
		border-radius: 6px;
		color: #e2e2e6;
		padding: 0.35rem 0.6rem;
		font-size: 0.82rem;
		font-family: 'Inter', system-ui, sans-serif;
		outline: none;
		transition: border-color 0.15s;
	}
	.field input:focus, .field select:focus { border-color: #6366f1; }

	.palette {
		display: flex;
		flex-direction: column;
		gap: 0.3rem;
	}

	.palette-btn {
		background: #1a1a24;
		border: 1px solid #2a2a38;
		border-left-width: 3px;
		border-radius: 6px;
		color: #c0c0d0;
		padding: 0.35rem 0.65rem;
		font-size: 0.8rem;
		text-align: left;
		cursor: pointer;
		transition: background 0.15s;
	}
	.palette-btn:hover { background: #22222e; }

	.action-btns {
		display: flex;
		flex-wrap: wrap;
		gap: 0.4rem;
	}

	.btn-primary {
		background: #6366f1;
		color: #fff;
		border: none;
		border-radius: 6px;
		padding: 0.45rem 0.9rem;
		font-size: 0.82rem;
		font-weight: 500;
		cursor: pointer;
		transition: opacity 0.15s;
	}
	.btn-primary:hover { opacity: 0.85; }

	.btn-secondary {
		background: #1e1e28;
		color: #c0c0d0;
		border: none;
		border-radius: 6px;
		padding: 0.45rem 0.9rem;
		font-size: 0.82rem;
		font-weight: 500;
		cursor: pointer;
		transition: opacity 0.15s;
	}
	.btn-secondary:hover:not(:disabled) { opacity: 0.8; }
	.btn-secondary:disabled { opacity: 0.35; cursor: default; }

	.conn-status {
		display: flex;
		align-items: center;
		gap: 0.35rem;
		font-size: 0.72rem;
		color: #555566;
		margin-top: 0.2rem;
	}
	.conn-status .dot {
		width: 6px; height: 6px; border-radius: 50%; background: #444;
	}
	.conn-status.connected { color: #4ade80; }
	.conn-status.connected .dot { background: #4ade80; box-shadow: 0 0 5px #4ade80; }

	.err-banner {
		margin: 0.75rem 1rem;
		background: #1f0a0a;
		border: 1px solid #3d1515;
		border-radius: 6px;
		padding: 0.5rem 0.75rem;
		font-size: 0.78rem;
		color: #f87171;
	}

	/* ── Canvas area ── */
	.canvas-area {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		position: relative;
	}

	.canvas {
		flex: 1;
		background: #0a0a0e;
		cursor: default;
		user-select: none;
	}

	/* ── Node cards (inside foreignObject) ── */
	.node-fo {
		overflow: visible;
	}

	.node-card {
		background: #1a1a24;
		border: 1.5px solid #2a2a38;
		border-radius: 10px;
		width: 100%;
		display: flex;
		flex-direction: column;
		gap: 0;
		cursor: move;
		overflow: hidden;
		transition: border-color 0.1s;
		border-top: 2px solid var(--node-color, #6366f1);
	}
	.node-card:hover {
		border-color: #3a3a50;
	}

	.node-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0.3rem 0.5rem 0.25rem;
		gap: 0.3rem;
	}

	.node-type-label {
		font-size: 0.7rem;
		font-weight: 600;
		letter-spacing: 0.02em;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.delete-btn {
		background: none;
		border: none;
		color: #555566;
		cursor: pointer;
		font-size: 0.9rem;
		padding: 0 0.1rem;
		line-height: 1;
		transition: color 0.1s;
		flex-shrink: 0;
	}
	.delete-btn:hover { color: #f87171; }

	.node-input {
		background: #12121a;
		border: none;
		border-top: 1px solid #2a2a38;
		color: #e2e2e6;
		padding: 0.3rem 0.5rem;
		font-size: 0.8rem;
		font-family: monospace;
		width: 100%;
		outline: none;
	}
	.node-input:focus { background: #16161f; }

	.node-select {
		background: #12121a;
		border: none;
		border-top: 1px solid #2a2a38;
		color: #e2e2e6;
		padding: 0.3rem 0.5rem;
		font-size: 0.82rem;
		width: 100%;
		outline: none;
		cursor: pointer;
	}

	.node-checkbox {
		display: flex;
		align-items: center;
		gap: 0.4rem;
		padding: 0.3rem 0.5rem;
		border-top: 1px solid #2a2a38;
		font-size: 0.8rem;
		color: #e2e2e6;
		cursor: pointer;
	}
	.node-checkbox input { cursor: pointer; }

	/* ── Ports ── */
	.port {
		cursor: crosshair;
		transition: r 0.1s;
	}
	.port:hover { r: 8; }
	.port.selected { filter: drop-shadow(0 0 4px #facc15); }

	/* ── Code panel ── */
	.code-panel {
		background: #0d0d12;
		border-top: 1px solid #1e1e26;
		max-height: 220px;
		overflow-y: auto;
		display: flex;
		flex-direction: column;
		gap: 0;
	}

	.code-section {
		padding: 0.6rem 1rem;
		border-bottom: 1px solid #1a1a22;
	}
	.code-section:last-child { border-bottom: none; }

	.code-label {
		font-size: 0.68rem;
		color: #6b6b7a;
		text-transform: uppercase;
		letter-spacing: 0.07em;
		margin-bottom: 0.35rem;
	}

	.code-pre {
		font-family: 'Fira Code', 'Cascadia Code', monospace;
		font-size: 0.8rem;
		color: #a5f3fc;
		white-space: pre;
		overflow-x: auto;
		line-height: 1.5;
	}

	.output-pre { color: #4ade80; }
	.error-pre { color: #f87171; }
</style>
