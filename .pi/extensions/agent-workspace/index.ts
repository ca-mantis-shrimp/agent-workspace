import type { ToolResultMessage } from "@earendil-works/pi-ai";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import { readFile, realpath } from "node:fs/promises";
import { join, resolve } from "node:path";
import {
	decodeUtf8,
	isSensitivePath,
	planReadCapture,
	repositoryRelativePath,
	type ReadParameters,
} from "./capture.ts";

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
	if (value.offset !== undefined && typeof value.offset !== "number") return undefined;
	if (value.limit !== undefined && typeof value.limit !== "number") return undefined;
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

export default function (pi: ExtensionAPI) {
	const runtimes = new Map<string, RepositoryRuntime | null>();
	const pendingReads = new Map<string, ReadParameters>();

	async function runtimeFor(
		cwd: string,
		signal?: AbortSignal,
	): Promise<RepositoryRuntime | undefined> {
		if (runtimes.has(cwd)) return runtimes.get(cwd) ?? undefined;
		const git = await pi.exec("git", ["-C", cwd, "rev-parse", "--show-toplevel"], {
			signal,
			timeout: 5_000,
		});
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
		runtimes.set(cwd, runtime);
		return runtime;
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
		const repositoryPath = repositoryRelativePath(runtime.root, runtime.root, canonical);
		if (
			!repositoryPath ||
			repositoryPath.startsWith(".agent-workspace/") ||
			isSensitivePath(repositoryPath)
		) {
			return;
		}

		const bytes = await readFile(canonical);
		const fileContent = decodeUtf8(bytes);
		if (fileContent === undefined) return;
		const decision = planReadCapture(
			fileContent,
			{ ...parameters, path: repositoryPath },
			resultText,
			{ truncated: nativeReadWasTruncated(message.details) },
		);
		if (!decision.capture) return;

		const args = [
			"observe",
			"--repository",
			runtime.root,
			"--workspace",
			runtime.workspace,
			"--path",
			decision.plan.repositoryPath,
			"--provider",
			"pi.read",
			"--model-visible-bytes",
			decision.plan.modelVisibleBytes.toString(),
			"--expected-raw-fingerprint",
			decision.plan.expectedRawFingerprint,
		];
		if (decision.plan.byteRange) {
			args.push("--range", `${decision.plan.byteRange.start}:${decision.plan.byteRange.end}`);
		}

		const captured = await pi.exec(runtime.binary, args, { signal, timeout: 10_000 });
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
}
