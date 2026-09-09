import assert from "node:assert/strict";
import { chmod, mkdtemp, mkdir, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import registerAgentWorkspace from "./index.ts";

interface RegisteredTool {
	name: string;
	execute: (
		toolCallId: string,
		params: Record<string, unknown>,
		signal: AbortSignal | undefined,
		onUpdate: undefined,
		ctx: { cwd: string; signal?: AbortSignal },
	) => Promise<{ content: { type: string; text: string }[]; details: unknown }>;
}

interface FakeHarness {
	pi: ExtensionAPI;
	tools: Map<string, RegisteredTool>;
	handlers: Map<string, (...args: any[]) => unknown>;
}

function fakeHarness(root: string, gitSucceeds = true): FakeHarness {
	const tools = new Map<string, RegisteredTool>();
	const handlers = new Map<string, (...args: any[]) => unknown>();
	const pi = {
		on(event: string, handler: (...args: any[]) => unknown) {
			handlers.set(event, handler);
		},
		registerTool(definition: RegisteredTool) {
			tools.set(definition.name, definition);
		},
		async exec(command: string) {
			assert.equal(command, "git");
			return gitSucceeds
				? { code: 0, stdout: `${root}\n`, stderr: "", killed: false }
				: { code: 1, stdout: "", stderr: "not a repository", killed: false };
		},
	} as unknown as ExtensionAPI;
	return { pi, tools, handlers };
}

const TOOL_NAMES = [
	"workspace_status",
	"workspace_delta",
	"workspace_working_set",
	"workspace_findings",
	"workspace_transaction_preview",
	"workspace_record_belief",
	"workspace_set_intent",
	"workspace_supersede_claim",
	"workspace_checkpoint",
	"workspace_observe_read",
];

async function installFakeMcp(
	root: string,
	capturePath: string,
): Promise<string> {
	await mkdir(join(root, "target", "debug"), { recursive: true });
	const binary = join(root, "target", "debug", "agent-workspace");
	const tools = TOOL_NAMES.map((name) => ({
		name,
		description: `MCP description for ${name}`,
		inputSchema: {
			type: "object",
			properties:
				name === "workspace_status"
					? { full: { type: "boolean" } }
					: name === "workspace_record_belief"
						? { statement: { type: "string" }, rests_on: { type: "array" } }
						: {},
			required:
				name === "workspace_record_belief" ? ["statement", "rests_on"] : [],
		},
	}));
	await writeFile(
		binary,
		`#!/usr/bin/env node
const fs = require("node:fs");
const readline = require("node:readline");
const tools = ${JSON.stringify(tools)};
const capture = ${JSON.stringify(capturePath)};
function send(value) { process.stdout.write(JSON.stringify(value) + "\\n"); }
readline.createInterface({ input: process.stdin }).on("line", line => {
  const message = JSON.parse(line);
  if (message.method === "initialize") {
    send({jsonrpc:"2.0", id:message.id, result:{protocolVersion:message.params.protocolVersion, capabilities:{tools:{}}, serverInfo:{name:"fake-agent-workspace",version:"0"}}});
  } else if (message.method === "tools/list") {
    send({jsonrpc:"2.0", id:message.id, result:{tools}});
  } else if (message.method === "tools/call") {
    fs.appendFileSync(capture, JSON.stringify(message.params) + "\\n");
    const fail = message.params.arguments && message.params.arguments.fail === true;
    send({jsonrpc:"2.0", id:message.id, result:{content:[{type:"text",text:fail ? "strict kernel rejection" : JSON.stringify(message.params.arguments)}],isError:fail}});
  }
});
`,
	);
	await chmod(binary, 0o755);
	return binary;
}

async function capturedCalls(path: string): Promise<any[]> {
	try {
		return (await readFile(path, "utf8"))
			.trim()
			.split("\n")
			.filter(Boolean)
			.map((line) => JSON.parse(line));
	} catch {
		return [];
	}
}

test("discovers the complete MCP surface and routes Pi tools through the official client", async () => {
	const root = await mkdtemp(join(tmpdir(), "agent-workspace-mcp-extension-"));
	const capturePath = join(root, "calls.jsonl");
	const binary = await installFakeMcp(root, capturePath);
	const previousBinary = process.env.AGENT_WORKSPACE_BIN;
	process.env.AGENT_WORKSPACE_BIN = binary;
	try {
		const harness = fakeHarness(root);
		await registerAgentWorkspace(harness.pi, root);
		assert.deepEqual([...harness.tools.keys()].sort(), [...TOOL_NAMES].sort());

		const status = harness.tools.get("workspace_status");
		assert.ok(status);
		const result = await status.execute(
			"call-1",
			{ full: true },
			undefined,
			undefined,
			{ cwd: root },
		);
		assert.equal(result.content[0].text, JSON.stringify({ full: true }));

		const belief = harness.tools.get("workspace_record_belief");
		assert.ok(belief);
		await belief.execute(
			"call-2",
			{ statement: "fixture belief", rests_on: ["src/example.txt"] },
			undefined,
			undefined,
			{ cwd: root },
		);
		const calls = await capturedCalls(capturePath);
		assert.deepEqual(calls[0], {
			name: "workspace_status",
			arguments: { full: true },
		});
		assert.deepEqual(calls[1], {
			name: "workspace_record_belief",
			arguments: {
				statement: "fixture belief",
				rests_on: ["src/example.txt"],
			},
		});

		await assert.rejects(
			status.execute("call-3", { fail: true }, undefined, undefined, {
				cwd: root,
			}),
			/strict kernel rejection/,
		);
		await harness.handlers.get("session_shutdown")?.();
	} finally {
		if (previousBinary === undefined) delete process.env.AGENT_WORKSPACE_BIN;
		else process.env.AGENT_WORKSPACE_BIN = previousBinary;
	}
});

test("successful reads preserve model-visible byte accounting through workspace_observe_read", async () => {
	const root = await mkdtemp(join(tmpdir(), "agent-workspace-mcp-capture-"));
	await mkdir(join(root, "src"));
	await writeFile(
		join(root, "src", "example.txt"),
		"zero\nαlpha\nbeta\ntail\n",
	);
	const capturePath = join(root, "calls.jsonl");
	const binary = await installFakeMcp(root, capturePath);
	const previousBinary = process.env.AGENT_WORKSPACE_BIN;
	process.env.AGENT_WORKSPACE_BIN = binary;
	try {
		const harness = fakeHarness(root);
		await registerAgentWorkspace(harness.pi, root);
		const toolCall = harness.handlers.get("tool_call");
		const context = harness.handlers.get("context");
		assert.ok(toolCall);
		assert.ok(context);
		const visible =
			"αlpha\nbeta\n\n[1 more lines in file. Use offset=4 to continue.]";
		toolCall({
			toolName: "read",
			toolCallId: "read-1",
			input: { path: "src/example.txt", offset: 2, limit: 2 },
		});
		await context(
			{
				messages: [
					{
						role: "toolResult",
						toolCallId: "read-1",
						toolName: "read",
						content: [{ type: "text", text: visible }],
						isError: false,
						details: {},
					},
				],
			},
			{ cwd: root },
		);
		const calls = await capturedCalls(capturePath);
		const capture = calls.find(
			(call) => call.name === "workspace_observe_read",
		);
		assert.ok(capture);
		assert.deepEqual(capture.arguments, {
			path: "src/example.txt",
			provider: "pi.read",
			offset: 2,
			limit: 2,
			model_visible_text: "αlpha\nbeta",
			model_visible_bytes: Buffer.byteLength(visible),
			truncated: false,
		});
		await harness.handlers.get("session_shutdown")?.();
	} finally {
		if (previousBinary === undefined) delete process.env.AGENT_WORKSPACE_BIN;
		else process.env.AGENT_WORKSPACE_BIN = previousBinary;
	}
});

test("absence of a repository is harmless and advertises no broken tools", async () => {
	const outside = await mkdtemp(join(tmpdir(), "agent-workspace-no-repo-"));
	const harness = fakeHarness(outside, false);
	await registerAgentWorkspace(harness.pi, outside);
	assert.equal(harness.tools.size, 0);
	assert.ok(
		harness.handlers.has("tool_call"),
		"capture hook remains installed",
	);
});
