import type { ToolResultMessage } from "@earendil-works/pi-ai";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import { Buffer } from "node:buffer";
import { access, realpath } from "node:fs/promises";
import { delimiter, isAbsolute, join, relative, resolve, sep } from "node:path";

interface ReadParameters {
	path: string;
	offset?: number;
	limit?: number;
}

interface ReadToolCallEvent {
	toolName: string;
	toolCallId: string;
	input: unknown;
}

interface McpTool {
	name: string;
	description?: string;
	inputSchema: {
		type: "object";
		properties?: Record<string, object>;
		required?: string[];
		[key: string]: unknown;
	};
}

interface RepositoryRuntime {
	root: string;
	binary: string;
	client: Client;
	tools: McpTool[];
}

interface McpTextContent {
	type: "text";
	text: string;
}

interface PiMetadata {
	label: string;
	promptSnippet: string;
	promptGuidelines: string[];
}

const TOOL_METADATA = {
	workspace_status: {
		label: "Workspace Status",
		promptSnippet:
			"Workspace orientation: objective, claim freshness, checkpoints.",
		promptGuidelines: [
			"Call workspace_status when resuming work or before acting on a workspace claim: a stale claim outranks your remembered belief.",
		],
	},
	workspace_delta: {
		label: "Workspace Delta",
		promptSnippet:
			"Workspace resume surface: changes since the last checkpoint.",
		promptGuidelines: [
			"Call workspace_delta right after workspace_status when resuming.",
		],
	},
	workspace_working_set: {
		label: "Workspace Working Set",
		promptSnippet:
			"Focused locations, uncited candidates, and navigation trail.",
		promptGuidelines: [
			"Re-read a working-set location reported stale before relying on it.",
		],
	},
	workspace_findings: {
		label: "Workspace Findings",
		promptSnippet: "Outstanding provider findings with freshness.",
		promptGuidelines: ["Re-verify stale findings before acting on them."],
	},
	workspace_transaction_preview: {
		label: "Workspace Transaction Preview",
		promptSnippet: "Review transaction scope, evidence, risks, and readiness.",
		promptGuidelines: [
			"Preview a transaction before accepting it; not-ready means acceptance will reject it.",
		],
	},
	workspace_record_belief: {
		label: "Workspace Record Belief",
		promptSnippet: "Record a durable belief and cite the files it rests on.",
		promptGuidelines: [
			"Record durable beliefs as they form; a later stale verdict outranks memory.",
		],
	},
	workspace_bind_objective: {
		label: "Workspace Bind Objective",
		promptSnippet: "Bind the workspace to the current objective.",
		promptGuidelines: ["Bind the external work authority when one exists."],
	},
	workspace_supersede_claim: {
		label: "Workspace Supersede Claim",
		promptSnippet: "Retire an outdated claim onto its recorded replacement.",
		promptGuidelines: [
			"Record the replacement belief before superseding the old claim.",
		],
	},
	workspace_checkpoint: {
		label: "Workspace Checkpoint",
		promptSnippet: "Draw a named restart boundary in the workspace log.",
		promptGuidelines: [
			"Checkpoint each coherent completed slice before changing objectives.",
		],
	},
	workspace_observe_read: {
		label: "Workspace Observe Read",
		promptSnippet: "Capture a native read as provenance and freshness support.",
		promptGuidelines: [
			"Use through read adapters; surface first-class skip reasons.",
		],
	},
} satisfies Record<string, PiMetadata>;

const PI_PAGINATION_NOTICE =
	/\n\n\[\d+ more lines in file\. Use offset=\d+ to continue\.\]$/;

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

async function fileExists(path: string): Promise<boolean> {
	try {
		await access(path);
		return true;
	} catch {
		return false;
	}
}

async function resolveBinary(root: string): Promise<string | undefined> {
	const explicit = process.env.AGENT_WORKSPACE_BIN;
	if (explicit && (await fileExists(explicit))) return explicit;
	for (const dir of (process.env.PATH ?? "").split(delimiter).filter(Boolean)) {
		const candidate = join(dir, "agent-workspace");
		if (await fileExists(candidate)) return candidate;
	}
	const inRepo = join(root, "target", "debug", "agent-workspace");
	return (await fileExists(inRepo)) ? inRepo : undefined;
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

function nativeReadWasTruncated(details: unknown): boolean {
	if (!details || typeof details !== "object") return false;
	const truncation = (details as { truncation?: unknown }).truncation;
	return Boolean(
		truncation &&
			typeof truncation === "object" &&
			(truncation as { truncated?: unknown }).truncated === true,
	);
}

export default async function registerAgentWorkspace(
	pi: ExtensionAPI,
	initialCwd = process.cwd(),
): Promise<void> {
	const roots = new Map<string, string | null>();
	const clients = new Map<string, Promise<RepositoryRuntime>>();
	const pendingReads = new Map<string, ReadParameters>();

	async function repositoryRoot(
		cwd: string,
		signal?: AbortSignal,
	): Promise<string | undefined> {
		if (roots.has(cwd)) return roots.get(cwd) ?? undefined;
		const git = await pi.exec(
			"git",
			["-C", cwd, "rev-parse", "--show-toplevel"],
			{
				signal,
				timeout: 5_000,
			},
		);
		if (git.code !== 0) {
			roots.set(cwd, null);
			return undefined;
		}
		const root = git.stdout.trim();
		roots.set(cwd, root);
		return root;
	}

	async function runtimeFor(
		cwd: string,
		signal?: AbortSignal,
	): Promise<RepositoryRuntime | undefined> {
		const root = await repositoryRoot(cwd, signal);
		if (!root) return undefined;
		const existing = clients.get(root);
		if (existing) return existing;
		const binary = await resolveBinary(root);
		if (!binary) return undefined;
		const connecting = (async () => {
			const transport = new StdioClientTransport({
				command: binary,
				args: ["mcp", "--repository", root],
				cwd: root,
				stderr: "ignore",
			});
			const client = new Client({
				name: "agent-workspace-pi",
				version: "0.1.0",
			});
			await client.connect(transport, { signal, timeout: 10_000 });
			const listed = await client.listTools({}, { signal, timeout: 10_000 });
			return {
				root,
				binary,
				client,
				tools: listed.tools as McpTool[],
			};
		})();
		clients.set(root, connecting);
		try {
			return await connecting;
		} catch (error) {
			clients.delete(root);
			throw error;
		}
	}

	async function callTool(
		cwd: string,
		name: string,
		arguments_: Record<string, unknown>,
		signal?: AbortSignal,
	) {
		const runtime = await runtimeFor(cwd, signal);
		if (!runtime)
			throw new Error("No agent-workspace MCP runtime is available here");
		const result = await runtime.client.callTool(
			{ name, arguments: arguments_ },
			undefined,
			{ signal, timeout: 10_000 },
		);
		const rawContent = (result as { content?: unknown }).content;
		if (!Array.isArray(rawContent))
			throw new Error(`${name} returned a task result`);
		const content = rawContent.filter(
			(block: unknown): block is McpTextContent =>
				typeof block === "object" &&
				block !== null &&
				(block as { type?: unknown }).type === "text" &&
				typeof (block as { text?: unknown }).text === "string",
		);
		if ((result as { isError?: boolean }).isError) {
			throw new Error(
				content.map((block) => block.text).join("\n") || `${name} failed`,
			);
		}
		return {
			content,
			details: { runtime: "mcp", root: runtime.root, binary: runtime.binary },
		};
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
		const canonical = await realpath(
			resolve(cwd, parameters.path.replace(/^@/, "")),
		);
		const repositoryPath = repositoryRelativePath(
			runtime.root,
			runtime.root,
			canonical,
		);
		if (!repositoryPath || repositoryPath.startsWith(".agent-workspace/"))
			return;
		await runtime.client.callTool(
			{
				name: "workspace_observe_read",
				arguments: {
					path: repositoryPath,
					provider: "pi.read",
					offset: parameters.offset,
					limit: parameters.limit,
					model_visible_text: resultText.replace(PI_PAGINATION_NOTICE, ""),
					model_visible_bytes: Buffer.byteLength(resultText, "utf8"),
					truncated: nativeReadWasTruncated(message.details),
				},
			},
			undefined,
			{ signal, timeout: 10_000 },
		);
	}

	pi.on("tool_call", (rawEvent) => {
		const event = rawEvent as ReadToolCallEvent;
		if (event.toolName !== "read") return;
		const parameters = readParameters(event.input);
		if (parameters) pendingReads.set(event.toolCallId, parameters);
	});
	pi.on("session_start", () => pendingReads.clear());
	pi.on("agent_settled", () => pendingReads.clear());
	pi.on("session_shutdown", async () => {
		for (const runtimePromise of clients.values()) {
			try {
				const runtime = await runtimePromise;
				await runtime.client.close();
			} catch {
				// A client that failed during initialization has nothing left to close.
			}
		}
		clients.clear();
	});
	pi.on("context", async (event, ctx) => {
		for (const [toolCallId, parameters] of pendingReads) {
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
				// Capture is observational: transport failure must never fail native read.
			}
		}
	});

	let initial: RepositoryRuntime | undefined;
	try {
		initial = await runtimeFor(initialCwd);
	} catch {
		// A missing or non-MCP binary is harmless: keep the capture hook dormant and
		// advertise no workspace tools rather than registering a broken surface.
		return;
	}
	if (!initial) return;

	for (const tool of initial.tools) {
		if (!tool.name.startsWith("workspace_")) continue;
		const metadata = (TOOL_METADATA as Record<string, PiMetadata>)[
			tool.name
		] ?? {
			label: tool.name,
			promptSnippet: tool.description ?? tool.name,
			promptGuidelines: [],
		};
		pi.registerTool({
			name: tool.name,
			label: metadata.label,
			description: tool.description ?? metadata.promptSnippet,
			promptSnippet: metadata.promptSnippet,
			promptGuidelines: metadata.promptGuidelines,
			parameters: tool.inputSchema as never,
			async execute(_toolCallId, params, signal, _onUpdate, ctx) {
				return callTool(
					ctx.cwd,
					tool.name,
					params as Record<string, unknown>,
					signal,
				);
			},
		});
	}
}
