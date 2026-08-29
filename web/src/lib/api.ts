// The fetch half: every call answers an `Envelope<T>`, and a refusal is a
// value in it rather than a thrown error. That is the same posture the
// CLI and the API take — tower says no by returning a shaped answer — so
// a caller reads `error` the way it reads `data`.

import type { Envelope } from './tower';

/// A failure the server never got to answer — the request itself did not
/// complete, or the body is not an envelope. Its id sits in a `web/`
/// namespace on purpose: it is the page's own failure, not one of
/// tower's refusals, and `ff tower explain` has nothing to say about it,
/// so it carries no exits.
function unreachable(cmd: string, detail: string): Envelope<never> {
	return {
		tower: 1,
		cmd,
		error: { id: 'web/unreachable', message: detail, exits: [] }
	};
}

async function envelope<T>(cmd: string, response: Response): Promise<Envelope<T>> {
	// The status varies by refusal id, the way the CLI's exit code does;
	// the envelope is the contract, so it is parsed either way.
	try {
		return (await response.json()) as Envelope<T>;
	} catch {
		return unreachable(cmd, `${cmd} answered ${response.status} with no envelope`);
	}
}

export async function get<T>(path: string): Promise<Envelope<T>> {
	try {
		return await envelope<T>(path, await fetch(path));
	} catch (err) {
		return unreachable(path, `${path} is unreachable: ${err}`);
	}
}

export async function post<T>(path: string, body: unknown): Promise<Envelope<T>> {
	try {
		const response = await fetch(path, {
			method: 'POST',
			headers: { 'content-type': 'application/json' },
			body: JSON.stringify(body)
		});
		return await envelope<T>(path, response);
	} catch (err) {
		return unreachable(path, `${path} is unreachable: ${err}`);
	}
}
