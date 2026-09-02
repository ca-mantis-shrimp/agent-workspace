import assert from "node:assert/strict";
import { mkdtemp, mkdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";
import type { ExtensionAPI } from "@earendil-works/pi-coding-agent";
import registerAgentWorkspace from "./index.ts";

test("successful bounded read results invoke the kernel with provider and visible-byte metadata", async () => {
	const root = await mkdtemp(join(tmpdir(), "agent-workspace-extension-"));
	await mkdir(join(root, "src"));
	await writeFile(
		join(root, "src", "example.txt"),
		"zero\nαlpha\nbeta\ntail\n",
		"utf8",
	);

	let toolCallHandler: ((event: unknown) => void) | undefined;
	let contextHandler:
		| ((
				event: { messages: unknown[] },
				ctx: { cwd: string; signal?: AbortSignal },
		  ) => Promise<void>)
		| undefined;
	let observedArguments: string[] | undefined;
	const fakePi = {
		on(event: string, handler: unknown) {
			if (event === "tool_call")
				toolCallHandler = handler as typeof toolCallHandler;
			if (event === "context") contextHandler = handler as typeof contextHandler;
		},
		registerTool() {},
		async exec(command: string, args: string[]) {
			if (command === "git")
				return { code: 0, stdout: `${root}\n`, stderr: "", killed: false };
			observedArguments = args;
			return { code: 0, stdout: "{}", stderr: "", killed: false };
		},
	} as unknown as ExtensionAPI;

	registerAgentWorkspace(fakePi);
	assert.ok(toolCallHandler);
	assert.ok(contextHandler);
	const selected = "αlpha\nbeta";
	const visible = `${selected}\n\n[1 more lines in file. Use offset=4 to continue.]`;
	toolCallHandler({
		toolName: "read",
		toolCallId: "read-1",
		input: { path: "src/example.txt", offset: 2, limit: 2 },
	});
	await contextHandler(
		{
			messages: [{
				role: "toolResult",
				toolCallId: "read-1",
				toolName: "read",
				content: [{ type: "text", text: visible }],
				isError: false,
				timestamp: Date.now(),
			}],
		},
		{ cwd: root },
	);

	assert.ok(observedArguments);
	assert.deepEqual(observedArguments.slice(0, 7), [
		"observe",
		"--repository",
		root,
		"--workspace",
		join(root, ".agent-workspace"),
		"--path",
		"src/example.txt",
	]);
	assert.equal(
		observedArguments[observedArguments.indexOf("--provider") + 1],
		"pi.read",
	);
	assert.equal(
		observedArguments[observedArguments.indexOf("--model-visible-bytes") + 1],
		Buffer.byteLength(visible).toString(),
	);
	assert.equal(
		observedArguments[observedArguments.indexOf("--range") + 1],
		`${Buffer.byteLength("zero\n")}:${Buffer.byteLength("zero\nαlpha\nbeta")}`,
	);
	assert.match(
		observedArguments[observedArguments.indexOf("--expected-raw-fingerprint") + 1],
		/^[0-9a-f]{64}$/,
	);
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

function repositoryRootStub(root: string): FakeExec {
	return (command, args) => {
		if (command === "git")
			return Promise.resolve({ code: 0, stdout: `${root}\n`, stderr: "" });
		return Promise.resolve({ code: 0, stdout: `executed:${args[0]}`, stderr: "" });
	};
}

test("workspace_status projects the kernel brief status by default and supports full", async () => {
	const root = await mkdtemp(join(tmpdir(), "agent-workspace-tools-"));
	const calls: string[][] = [];
	const exec: FakeExec = (command, args) => {
		if (command !== "git") calls.push(args);
		return repositoryRootStub(root)(command, args);
	};
	const tools = registerWithFakePi(exec);
	const status = tools.get("workspace_status");
	assert.ok(status, "workspace_status must be registered");

	const brief = await status.execute("call-1", {}, undefined, undefined, { cwd: root });
	assert.deepEqual(calls[0].slice(0, 5), [
		"status",
		"--repository",
		root,
		"--workspace",
		join(root, ".agent-workspace"),
	]);
	assert.equal(calls[0].length, 5, "brief status adds no extra flags");
	assert.equal(brief.content[0].text, "executed:status");

	await status.execute("call-2", { full: true }, undefined, undefined, { cwd: root });
	assert.deepEqual(calls[1].slice(0, 6), [
		"status",
		"--full",
		"--repository",
		root,
		"--workspace",
		join(root, ".agent-workspace"),
	]);
});

test("workspace_delta passes the checkpoint selector through to the kernel", async () => {
	const root = await mkdtemp(join(tmpdir(), "agent-workspace-tools-"));
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
	assert.ok(!calls[0].includes("--since"), "default delta diffs against the latest checkpoint");

	await delta.execute("call-2", { since: "session-8-claims-curated" }, undefined, undefined, {
		cwd: root,
	});
	const since = calls[1].indexOf("--since");
	assert.notEqual(since, -1);
	assert.equal(calls[1][since + 1], "session-8-claims-curated");
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

	const absent = await status.execute("call-1", {}, undefined, undefined, { cwd: outside });
	assert.match(absent.content[0].text, /not inside a Git repository/);

	const root = await mkdtemp(join(tmpdir(), "agent-workspace-tools-"));
	const failing = registerWithFakePi(async (command, args) => {
		if (command === "git") return { code: 0, stdout: `${root}\n`, stderr: "" };
		return { code: 2, stdout: "", stderr: "workspace error" };
	});
	const failingStatus = failing.get("workspace_status");
	assert.ok(failingStatus);
	await assert.rejects(
		failingStatus.execute("call-2", {}, undefined, undefined, { cwd: root }),
		/workspace error/,
	);
});
