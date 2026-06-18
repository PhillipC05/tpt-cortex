/**
 * Typed WebSocket client for the Cortex daemon.
 *
 * Protocol (matches cortex-daemon/ipc/protocol.go):
 *   1. On connect, server sends: { type: "connected", token: string }
 *   2. Client sends JSON-RPC: { jsonrpc, method: "ExecuteCortex", params, id, token }
 *   3. Server responds:       { jsonrpc, id, result: { logs } | error: { code, message } }
 */

export type ConnectionState = 'disconnected' | 'connecting' | 'connected';

export interface ExecuteResult {
	logs: string[];
}

interface ConnectedMsg {
	type: 'connected';
	token: string;
}

interface RPCResponse {
	jsonrpc: '2.0';
	id: number;
	result?: ExecuteResult;
	error?: { code: number; message: string };
}

type PendingCall = {
	resolve: (r: ExecuteResult) => void;
	reject: (e: Error) => void;
};

export class CortexClient {
	private ws: WebSocket | null = null;
	private token = '';
	private nextId = 1;
	private pending = new Map<number, PendingCall>();
	private onState: (s: ConnectionState) => void;
	private url: string;

	constructor(onStateChange: (s: ConnectionState) => void, url = 'ws://127.0.0.1:9911/ws') {
		this.onState = onStateChange;
		this.url = url;
	}

	connect(): Promise<void> {
		if (this.ws) return Promise.resolve();

		return new Promise((resolve, reject) => {
			this.onState('connecting');
			const ws = new WebSocket(this.url);
			this.ws = ws;

			const timeout = setTimeout(() => {
				ws.close();
				reject(new Error('Connection timeout'));
			}, 6000);

			ws.onmessage = (ev) => {
				let msg: ConnectedMsg | RPCResponse;
				try {
					msg = JSON.parse(ev.data as string);
				} catch {
					return;
				}

				// Handshake
				if ((msg as ConnectedMsg).type === 'connected') {
					clearTimeout(timeout);
					this.token = (msg as ConnectedMsg).token;
					this.onState('connected');
					resolve();
					return;
				}

				// RPC response
				const rpc = msg as RPCResponse;
				const call = this.pending.get(rpc.id);
				if (!call) return;
				this.pending.delete(rpc.id);

				if (rpc.error) {
					call.reject(new Error(rpc.error.message));
				} else {
					call.resolve(rpc.result ?? { logs: [] });
				}
			};

			ws.onerror = () => {
				clearTimeout(timeout);
				this.cleanup();
				reject(new Error('WebSocket connection failed'));
			};

			ws.onclose = () => {
				clearTimeout(timeout);
				this.cleanup();
			};
		});
	}

	execute(script: string, opts: { allow?: string[] } = {}): Promise<ExecuteResult> {
		if (!this.ws || this.ws.readyState !== WebSocket.OPEN) {
			return Promise.reject(new Error('Not connected to Cortex daemon'));
		}

		return new Promise((resolve, reject) => {
			const id = this.nextId++;
			this.pending.set(id, { resolve, reject });

			const msg = {
				jsonrpc: '2.0',
				method: 'ExecuteCortex',
				params: { script, allow: opts.allow ?? [] },
				id,
				token: this.token,
			};
			this.ws!.send(JSON.stringify(msg));

			// Per-call timeout (30 s)
			setTimeout(() => {
				if (this.pending.has(id)) {
					this.pending.delete(id);
					reject(new Error('Execution timed out'));
				}
			}, 30_000);
		});
	}

	disconnect() {
		this.ws?.close();
		this.cleanup();
	}

	private cleanup() {
		this.ws = null;
		this.token = '';
		// Reject all in-flight calls
		for (const [, call] of this.pending) {
			call.reject(new Error('Disconnected'));
		}
		this.pending.clear();
		this.onState('disconnected');
	}
}
