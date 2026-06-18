const DAEMON_URL = 'ws://127.0.0.1:9911';
const TOKEN_KEY = 'cortex_daemon_token';

export type ExecuteResult = { logs: string[] };
export type ConnectionState = 'disconnected' | 'connecting' | 'connected';

type PendingRequest = {
	resolve: (result: ExecuteResult) => void;
	reject: (err: Error) => void;
};

export class CortexClient {
	private ws: WebSocket | null = null;
	private token: string | null = null;
	private state: ConnectionState = 'disconnected';
	private pending = new Map<number, PendingRequest>();
	private nextId = 1;
	private onStateChange?: (s: ConnectionState) => void;

	constructor(onStateChange?: (s: ConnectionState) => void) {
		this.onStateChange = onStateChange;
	}

	get connectionState(): ConnectionState {
		return this.state;
	}

	/** Attempt to connect to the local daemon. Resolves when connected, rejects if unreachable. */
	connect(): Promise<void> {
		return new Promise((resolve, reject) => {
			if (this.state === 'connected') { resolve(); return; }

			this.setState('connecting');
			const ws = new WebSocket(DAEMON_URL);
			let settled = false;

			ws.onopen = () => {
				// Wait for the "connected" message with the token before resolving
			};

			ws.onmessage = (event) => {
				let msg: Record<string, unknown>;
				try { msg = JSON.parse(event.data); } catch { return; }

				// First message: token handshake
				if (msg.type === 'connected' && typeof msg.token === 'string') {
					this.token = msg.token;
					localStorage.setItem(TOKEN_KEY, msg.token);
					this.ws = ws;
					this.setState('connected');
					if (!settled) { settled = true; resolve(); }
					return;
				}

				// JSON-RPC response
				const id = msg.id as number;
				const req = this.pending.get(id);
				if (!req) return;
				this.pending.delete(id);

				if (msg.error) {
					req.reject(new Error((msg.error as { message: string }).message));
				} else {
					req.resolve(msg.result as ExecuteResult);
				}
			};

			ws.onerror = () => {
				this.setState('disconnected');
				if (!settled) { settled = true; reject(new Error('Cannot reach TPT Core daemon')); }
			};

			ws.onclose = () => {
				this.ws = null;
				this.setState('disconnected');
				// Reject all pending requests
				for (const [, req] of this.pending) {
					req.reject(new Error('Connection closed'));
				}
				this.pending.clear();
			};
		});
	}

	disconnect() {
		this.ws?.close();
		this.ws = null;
	}

	/** Execute a Cortex script on the daemon. */
	execute(script: string, options: { allow?: string[] } = {}): Promise<ExecuteResult> {
		if (!this.ws || this.state !== 'connected' || !this.token) {
			return Promise.reject(new Error('Not connected to TPT Core'));
		}

		return new Promise((resolve, reject) => {
			const id = this.nextId++;
			this.pending.set(id, { resolve, reject });
			this.ws!.send(JSON.stringify({
				jsonrpc: '2.0',
				method: 'ExecuteCortex',
				params: { script, allow: options.allow ?? [] },
				id,
				token: this.token,
			}));
		});
	}

	private setState(s: ConnectionState) {
		this.state = s;
		this.onStateChange?.(s);
	}
}
