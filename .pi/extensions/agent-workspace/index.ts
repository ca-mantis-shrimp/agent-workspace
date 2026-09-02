import type { ToolResultMessage } from "@earendil-works/pi-ai";
import { Buffer } from "node:buffer";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { Type } from "typebox";
import { access, realpath } from "node:fs/promises";
import { spawn } from "node:child_process";
import { isAbsolute, join, relative, resolve, sep } from "node:path";

interface ReadParameters {
	path: string;
	offset?: number;
	limit?: number;
}

function repositoryRelativePath(
	repositoryRoot: string,
	cwd: string,
	requestedPath: string,
): string | undefined {
	const absolute = resolve(cwd, requestedPath.replace(/^@/, ""));
	const candidate = relative(resolve(repositoryRoot), absolute);
	if (candidate === "" || candidate === ".") return undefined;
	if (
		candidate === ".." ||
		candidate.startsWith(`..${sep}`) ||
		isAbsolute(candidate)
	) {
		return undefined;
	}
	return candidate.split(sep).join("/");
}

interface ReadToolCallEvent {
	toolName: string;
	toolCallId: string;
	input: unknown;
}

interface RepositoryRuntime {
	root: string;
	binary: string;
	workspace: string;
}

function readParameters(input: unknown): ReadParameters | undefined {
	if (!input || typeof input !== "object") return undefined;
	const value = input as { path?: unknown; offset?: unknown; limit?: unknown };
	if (typeof value.path !== "string") return undefined;
	if (value.offset !== undefined && typeof value.offset !== "number")
		return undefined;
	if (value.limit !== undefined && typeof value.limit !== "number")
		return undefined;
	return { path: value.path, offset: value.offset, limit: value.limit };
}

function textResult(message: ToolResultMessage): string | undefined {
	if (message.isError || message.content.length !== 1) return undefined;
	const [content] = message.content;
	return content?.type === "text" ? content.text : undefined;
}

const PI_PAGINATION_NOTICE =
	/\n\n\[\d+ more lines in file\. Use offset=\d+ to continue\.\]$/;

function stripPiReadChrome(resultText: string): string {
	return resultText.replace(PI_PAGINATION_NOTICE, "");
}

function nativeReadWasTruncated(details: unknown): boolean {
	if (!details || typeof details !== "object") return false;
	const truncation = (details as { truncation?: unknown }).truncation;
	return Boolean(
		truncation &&
			typeof truncation === "object" &&
			(truncation as { truncated?: unknown }).truncated === true,
	);
}

export default function (pi: ExtensionAPI) {
	const runtimes = new Map<string, RepositoryRuntime | null>();
	const pendingReads = new Map<string, ReadParameters>();

	async function runtimeFor(
		cwd: string,
		signal?: AbortSignal,
	): Promise<RepositoryRuntime | undefined> {
		if (runtimes.has(cwd)) return runtimes.get(cwd) ?? undefined;
		const git = await pi.exec(
			"git",
			["-C", cwd, "rev-parse", "--show-toplevel"],
			{
				signal,
				timeout: 5_000,
			},
		);
		if (git.code !== 0) {
			runtimes.set(cwd, null);
			return undefined;
		}
		const root = git.stdout.trim();
		const runtime = {
			root,
			binary: join(root, "target", "debug", "agent-workspace"),
			workspace: join(root, ".agent-workspace"),
		};
		try {
			await access(runtime.binary);
		} catch {
			// Do not cache this miss: a build later in the same Pi session should
			// activate the adapter without requiring a restart.
			return undefined;
		}
		runtimes.set(cwd, runtime);
		return runtime;
	}

	function execWithInput(
		command: string,
		args: string[],
		input: string,
		signal?: AbortSignal,
	): Promise<{ code: number; stdout: string; stderr: string }> {
		return new Promise((resolvePromise, rejectPromise) => {
			const child = spawn(command, args, {
				stdio: ["pipe", "pipe", "pipe"],
				signal,
			});
			let stdout = "";
			let stderr = "";
			child.stdout.setEncoding("utf8");
			child.stderr.setEncoding("utf8");
			child.stdout.on("data", (chunk: string) => (stdout += chunk));
			child.stderr.on("data", (chunk: string) => (stderr += chunk));
			const timeout = setTimeout(() => child.kill("SIGKILL"), 10_000);
			child.on("error", (error) => {
				clearTimeout(timeout);
				rejectPromise(error);
			});
			child.on("close", (code) => {
				clearTimeout(timeout);
				resolvePromise({ code: code ?? 1, stdout, stderr });
			});
			child.stdin.end(input);
		});
	}

	async function captureRead(
		parameters: ReadParameters,
		message: ToolResultMessage,
		cwd: string,
		signal?: AbortSignal,
	): Promise<void> {
		const resultText = textResult(message);
		if (resultText === undefined) return;
		const runtime = await runtimeFor(cwd, signal);
		if (!runtime) return;

		const requested = resolve(cwd, parameters.path.replace(/^@/, ""));
		const canonical = await realpath(requested);
		const repositoryPath = repositoryRelativePath(
			runtime.root,
			runtime.root,
			canonical,
		);
		if (!repositoryPath || repositoryPath.startsWith(".agent-workspace/")) return;

		const semanticReadText = stripPiReadChrome(resultText);
		const args = [
			"observe-read",
			"--repository",
			runtime.root,
			"--workspace",
			runtime.workspace,
			"--path",
			repositoryPath,
			"--provider",
			"pi.read",
			"--model-visible-bytes",
			Buffer.byteLength(resultText, "utf8").toString(),
		];
		if (parameters.offset !== undefined)
			args.push("--offset", parameters.offset.toString());
		if (parameters.limit !== undefined)
			args.push("--limit", parameters.limit.toString());
		if (nativeReadWasTruncated(message.details)) args.push("--truncated");

		// Pi's extension exec API has no stdin channel. Spawn the kernel directly
		// so arbitrary model-visible read text stays off argv and ephemeral files;
		// observe-read consumes it on stdin and owns every capture decision.
		const captured = await execWithInput(
			runtime.binary,
			args,
			semanticReadText,
			signal,
		);
		if (captured.code !== 0) return;
	}

	pi.on("tool_call", (rawEvent) => {
		const event = rawEvent as ReadToolCallEvent;
		if (event.toolName !== "read") return;
		const parameters = readParameters(event.input);
		if (parameters) pendingReads.set(event.toolCallId, parameters);
	});

	pi.on("session_start", () => pendingReads.clear());
	pi.on("agent_settled", () => pendingReads.clear());

	// `context` is Pi's model-boundary message projection. This sees finalized
	// tool results after tool_result and message_end middleware, rather than an
	// intermediate native result. A later-loaded context handler can still alter
	// the projection; that extension-order limitation is documented explicitly.
	pi.on("context", async (event, ctx) => {
		for (const [toolCallId, parameters] of [...pendingReads]) {
			const message = event.messages.find(
				(candidate): candidate is ToolResultMessage =>
					candidate.role === "toolResult" &&
					candidate.toolCallId === toolCallId &&
					candidate.toolName === "read",
			);
			if (!message) continue;
			pendingReads.delete(toolCallId);
			try {
				await captureRead(parameters, message, ctx.cwd, ctx.signal);
			} catch {
				// Auto-capture is observational and must never turn a successful native
				// read into a failed tool result. A mismatch/race records nothing.
			}
		}
	});

	// Orientation tools: thin projections over the kernel's brief status and
	// checkpoint-delta surfaces. They add no semantics — the binary owns every
	// decision — so a fresh agent can orient without shelling to the CLI.
	// Expected environment conditions (no Git checkout, no kernel binary) come
	// back as plain text so the agent can adapt; only a genuinely failed kernel
	// invocation throws and surfaces as a tool error.
	async function runKernel(
		cwd: string,
		command: string[],
		signal?: AbortSignal,
	): Promise<{
		content: [{ type: "text"; text: string }];
		details: { runtime: string };
	}> {
		const runtime = await runtimeFor(cwd, signal);
		if (!runtime) {
			return {
				content: [
					{
						type: "text",
						text:
							"No agent-workspace runtime here: this is not a Git checkout with a built target/debug/agent-workspace binary.",
					},
				],
				details: { runtime: "absent" },
			};
		}
		let result;
		try {
			result = await pi.exec(
				runtime.binary,
				[
					...command,
					"--repository",
					runtime.root,
					"--workspace",
					runtime.workspace,
				],
				{ signal, timeout: 10_000 },
			);
		} catch (error) {
			throw new Error(
				`agent-workspace kernel binary not runnable at ${runtime.binary}: ${error instanceof Error ? error.message : String(error)}`,
			);
		}
		if (result.code !== 0) {
			throw new Error(
				`agent-workspace ${command[0]} failed (${result.code}): ${result.stderr || result.stdout || "no output"}`,
			);
		}
		return {
			content: [{ type: "text", text: result.stdout }],
			details: { runtime: "active" },
		};
	}

	pi.registerTool({
		name: "workspace_status",
		label: "Workspace Status",
		description:
			"Orient in the persistent agent workspace: objective, a kernel-bounded stale-first claim window with explicit omission count, aggregate freshness, open transactions, and latest checkpoint. `full` returns the complete audit record.",
		promptSnippet:
			"Workspace orientation: objective, claim freshness, checkpoints.",
		promptGuidelines: [
			"Call workspace_status when resuming work or before acting on a workspace claim: a claim it reports as stale outranks your remembered belief about that claim.",
		],
		parameters: Type.Object({
			full: Type.Optional(
				Type.Boolean({
					description:
						"Return the full status projection instead of the brief one (default false).",
				}),
			),
		}),
		async execute(_toolCallId, params, signal, _onUpdate, ctx) {
			const command = params.full ? ["status", "--full"] : ["status", "--compact"];
			return runKernel(ctx.cwd, command, signal);
		},
	});

	pi.registerTool({
		name: "workspace_delta",
		label: "Workspace Delta",
		description:
			"Kernel-bounded changes since a checkpoint: objective shift plus total/recent ids for claims, observations, and transactions. The concise resume surface after workspace_status; `full` returns complete entities.",
		promptSnippet: "Workspace resume surface: changes since the last checkpoint.",
		promptGuidelines: [
			"Call workspace_delta right after workspace_status when resuming: it shows only what changed since the checkpoint, instead of the full projection.",
		],
		parameters: Type.Object({
			full: Type.Optional(
				Type.Boolean({
					description:
						"Return complete changed entities instead of the bounded id summary.",
				}),
			),
			since: Type.Optional(
				Type.String({
					description:
						"Diff against this checkpoint label instead of the latest checkpoint.",
				}),
			),
		}),
		async execute(_toolCallId, params, signal, _onUpdate, ctx) {
			const command = ["delta"];
			if (params.full) command.push("--full");
			else command.push("--compact");
			if (params.since) command.push("--since", params.since);
			return runKernel(ctx.cwd, command, signal);
		},
	});

	pi.registerTool({
		name: "workspace_working_set",
		label: "Workspace Working Set",
		description:
			"The bounded attention model: ranked semantic locations you have focused (path, selector, revision, freshness, why), current observations not yet cited by any claim, and the ordered navigation trail. Every section is kernel-bounded with an explicit omission count.",
		promptSnippet:
			"Where your attention stands: focused locations, uncited candidates, navigation trail.",
		promptGuidelines: [
			"Call workspace_working_set to see what you are attending to and what is worth attending to next: a location it reports as stale had an edit land under it, so re-read before you rely on it.",
		],
		parameters: Type.Object({}),
		async execute(_toolCallId, _params, signal, _onUpdate, ctx) {
			return runKernel(ctx.cwd, ["working-set", "--compact"], signal);
		},
	});
}
