import assert from "node:assert/strict";
import { chmod, mkdtemp, mkdir, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import registerAgentWorkspace from "./index.ts";

test("successful reads stream model-visible text to kernel-owned observe-read", async () => {
	const root = await mkdtemp(join(tmpdir(), "agent-workspace-extension-"));
	await mkdir(join(root, "src"));
	await mkdir(join(root, "target", "debug"), { recursive: true });
	await writeFile(
		join(root, "src", "example.txt"),
		"zero\nαlpha\nbeta\ntail\n",
		"utf8",
	);
	const capturePath = join(root, "capture.json");
	const binary = join(root, "target", "debug", "agent-workspace");
	await writeFile(
		binary,
		`#!/usr/bin/env node
let stdin = "";
process.stdin.setEncoding("utf8");
process.stdin.on("data", chunk => stdin += chunk);
process.stdin.on("end", () => {
  require("node:fs").writeFileSync(${JSON.stringify(capturePath)}, JSON.stringify({ args: process.argv.slice(2), stdin }));
  process.stdout.write("{}\\n");
});
`,
	);
	await chmod(binary, 0o755);

	let toolCallHandler: ((event: unknown) => void) | undefined;
	let contextHandler:
		| ((
				event: { messages: unknown[] },
				ctx: { cwd: string; signal?: AbortSignal },
		  ) => Promise<void>)
		| undefined;
	const fakePi = {
		on(event: string, handler: unknown) {
			if (event === "tool_call")
				toolCallHandler = handler as typeof toolCallHandler;
			if (event === "context") contextHandler = handler as typeof contextHandler;
		},
		registerTool() {},
		async exec(command: string) {
			assert.equal(command, "git");
			return { code: 0, stdout: `${root}\n`, stderr: "", killed: false };
		},
	} as unknown as ExtensionAPI;

	registerAgentWorkspace(fakePi);
	assert.ok(toolCallHandler);
	assert.ok(contextHandler);
	const visible =
		"αlpha\nbeta\n\n[1 more lines in file. Use offset=4 to continue.]";
	toolCallHandler({
		toolName: "read",
		toolCallId: "read-1",
		input: { path: "src/example.txt", offset: 2, limit: 2 },
	});
	await contextHandler(
		{
			messages: [
				{
					role: "toolResult",
					toolCallId: "read-1",
					toolName: "read",
					content: [{ type: "text", text: visible }],
					isError: false,
					timestamp: Date.now(),
				},
			],
		},
		{ cwd: root },
	);

	const captured = JSON.parse(await readFile(capturePath, "utf8")) as {
		args: string[];
		stdin: string;
	};
	assert.equal(captured.args[0], "observe-read");
	assert.equal(
		captured.args[captured.args.indexOf("--path") + 1],
		"src/example.txt",
	);
	assert.equal(
		captured.args[captured.args.indexOf("--provider") + 1],
		"pi.read",
	);
	assert.equal(captured.args[captured.args.indexOf("--offset") + 1], "2");
	assert.equal(captured.args[captured.args.indexOf("--limit") + 1], "2");
	assert.equal(
		captured.args[captured.args.indexOf("--model-visible-bytes") + 1],
		Buffer.byteLength(visible).toString(),
	);
	assert.equal(captured.stdin, "αlpha\nbeta");
});

type FakeExec = (command: string, args: string[]) => Promise<ExecOutcome>;

interface ExecOutcome {
	code: number;
	stdout: string;
	stderr: string;
}

interface RegisteredTool {
	name: string;
	parameters: unknown;
	execute: (
		toolCallId: string,
		params: Record<string, unknown>,
		signal: AbortSignal | undefined,
		onUpdate: undefined,
		ctx: { cwd: string; signal?: AbortSignal },
	) => Promise<{ content: { type: string; text: string }[]; details: unknown }>;
}

function registerWithFakePi(exec: FakeExec): Map<string, RegisteredTool> {
	const tools = new Map<string, RegisteredTool>();
	const fakePi = {
		on() {},
		registerTool(definition: RegisteredTool) {
			tools.set(definition.name, definition);
		},
		exec,
	} as unknown as ExtensionAPI;
	registerAgentWorkspace(fakePi);
	return tools;
}

async function installKernelPlaceholder(root: string): Promise<void> {
	await mkdir(join(root, "target", "debug"), { recursive: true });
	await writeFile(join(root, "target", "debug", "agent-workspace"), "stub");
}

function repositoryRootStub(root: string): FakeExec {
	return (command, args) => {
		if (command === "git")
			return Promise.resolve({ code: 0, stdout: `${root}\n`, stderr: "" });
		return Promise.resolve({
			code: 0,
			stdout: `executed:${args[0]}`,
			stderr: "",
		});
	};
}

test("workspace_status projects the kernel brief status by default and supports full", async () => {
	const root = await mkdtemp(join(tmpdir(), "agent-workspace-tools-"));
	await installKernelPlaceholder(root);
	const calls: string[][] = [];
	const exec: FakeExec = (command, args) => {
		if (command !== "git") calls.push(args);
		return repositoryRootStub(root)(command, args);
	};
	const tools = registerWithFakePi(exec);
	const status = tools.get("workspace_status");
	assert.ok(status, "workspace_status must be registered");

	const brief = await status.execute("call-1", {}, undefined, undefined, {
		cwd: root,
	});
	assert.deepEqual(calls[0].slice(0, 4), [
		"status",
		"--compact",
		"--repository",
		root,
	]);
	assert.equal(calls[0].length, 4);
	assert.equal(brief.content[0].text, "executed:status");

	await status.execute("call-2", { full: true }, undefined, undefined, {
		cwd: root,
	});
	assert.deepEqual(calls[1].slice(0, 4), [
		"status",
		"--full",
		"--repository",
		root,
	]);
});

test("workspace_delta passes the checkpoint selector through to the kernel", async () => {
	const root = await mkdtemp(join(tmpdir(), "agent-workspace-tools-"));
	await installKernelPlaceholder(root);
	const calls: string[][] = [];
	const exec: FakeExec = (command, args) => {
		if (command !== "git") calls.push(args);
		return repositoryRootStub(root)(command, args);
	};
	const tools = registerWithFakePi(exec);
	const delta = tools.get("workspace_delta");
	assert.ok(delta, "workspace_delta must be registered");

	await delta.execute("call-1", {}, undefined, undefined, { cwd: root });
	assert.equal(calls[0][0], "delta");
	assert.ok(calls[0].includes("--compact"));
	assert.ok(
		!calls[0].includes("--since"),
		"default delta diffs against the latest checkpoint",
	);

	await delta.execute(
		"call-2",
		{ since: "session-8-claims-curated" },
		undefined,
		undefined,
		{
			cwd: root,
		},
	);
	const since = calls[1].indexOf("--since");
	assert.notEqual(since, -1);
	assert.equal(calls[1][since + 1], "session-8-claims-curated");

	await delta.execute("call-3", { full: true }, undefined, undefined, {
		cwd: root,
	});
	assert.ok(calls[2].includes("--full"));
	assert.ok(!calls[2].includes("--compact"));
});

test("workspace_working_set projects the bounded attention model via the kernel", async () => {
	const root = await mkdtemp(join(tmpdir(), "agent-workspace-tools-"));
	await installKernelPlaceholder(root);
	const calls: string[][] = [];
	const exec: FakeExec = (command, args) => {
		if (command !== "git") calls.push(args);
		return repositoryRootStub(root)(command, args);
	};
	const tools = registerWithFakePi(exec);
	const workingSet = tools.get("workspace_working_set");
	assert.ok(workingSet, "workspace_working_set must be registered");

	const result = await workingSet.execute("call-1", {}, undefined, undefined, {
		cwd: root,
	});
	assert.deepEqual(calls[0], [
		"working-set",
		"--compact",
		"--repository",
		root,
	]);
	assert.equal(result.content[0].text, "executed:working-set");
});

test("workspace_findings projects the bounded quickfix queue via the kernel", async () => {
	const root = await mkdtemp(join(tmpdir(), "agent-workspace-tools-"));
	await installKernelPlaceholder(root);
	const calls: string[][] = [];
	const exec: FakeExec = (command, args) => {
		if (command !== "git") calls.push(args);
		return repositoryRootStub(root)(command, args);
	};
	const tools = registerWithFakePi(exec);
	const findings = tools.get("workspace_findings");
	assert.ok(findings, "workspace_findings must be registered");

	const result = await findings.execute("call-1", {}, undefined, undefined, {
		cwd: root,
	});
	assert.deepEqual(calls[0], [
		"findings",
		"--compact",
		"--repository",
		root,
	]);
	assert.equal(result.content[0].text, "executed:findings");
});

test("workspace_transaction_preview passes the transaction id through to the kernel", async () => {
	const root = await mkdtemp(join(tmpdir(), "agent-workspace-tools-"));
	await installKernelPlaceholder(root);
	const calls: string[][] = [];
	const exec: FakeExec = (command, args) => {
		if (command !== "git") calls.push(args);
		return repositoryRootStub(root)(command, args);
	};
	const tools = registerWithFakePi(exec);
	const preview = tools.get("workspace_transaction_preview");
	assert.ok(preview, "workspace_transaction_preview must be registered");

	const result = await preview.execute(
		"call-1",
		{ transaction: 3 },
		undefined,
		undefined,
		{ cwd: root },
	);
	assert.deepEqual(calls[0], [
		"preview-transaction",
		"--compact",
		"--transaction",
		"3",
		"--repository",
		root,
	]);
	assert.equal(result.content[0].text, "executed:preview-transaction");
});

test("orientation tools degrade to plain text outside a repository and throw on kernel failure", async () => {
	const outside = await mkdtemp(join(tmpdir(), "agent-workspace-outside-"));
	const tools = registerWithFakePi(async (_command, _args) => ({
		code: 1,
		stdout: "",
		stderr: "fatal: not a git repository",
	}));
	const status = tools.get("workspace_status");
	assert.ok(status);

	const absent = await status.execute("call-1", {}, undefined, undefined, {
		cwd: outside,
	});
	assert.match(absent.content[0].text, /not a Git checkout/);

	const missingRoot = await mkdtemp(
		join(tmpdir(), "agent-workspace-no-binary-"),
	);
	const missing = registerWithFakePi(async (command) => {
		assert.equal(command, "git");
		return { code: 0, stdout: `${missingRoot}\n`, stderr: "" };
	});
	const missingStatus = missing.get("workspace_status");
	assert.ok(missingStatus);
	const noBinary = await missingStatus.execute(
		"call-2",
		{},
		undefined,
		undefined,
		{ cwd: missingRoot },
	);
	assert.match(noBinary.content[0].text, /built target\/debug\/agent-workspace/);

	const root = await mkdtemp(join(tmpdir(), "agent-workspace-tools-"));
	await installKernelPlaceholder(root);
	const failing = registerWithFakePi(async (command) => {
		if (command === "git") return { code: 0, stdout: `${root}\n`, stderr: "" };
		return { code: 2, stdout: "", stderr: "workspace error" };
	});
	const failingStatus = failing.get("workspace_status");
	assert.ok(failingStatus);
	await assert.rejects(
		failingStatus.execute("call-3", {}, undefined, undefined, { cwd: root }),
		/workspace error/,
	);
});
