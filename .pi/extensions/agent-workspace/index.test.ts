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
