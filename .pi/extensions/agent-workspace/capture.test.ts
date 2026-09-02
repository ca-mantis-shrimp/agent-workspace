import assert from "node:assert/strict";
import test from "node:test";
import {
	decodeUtf8,
	isSensitivePath,
	planReadCapture,
	repositoryRelativePath,
} from "./capture.ts";

test("whole-file reads retain whole-file scope and exact visible bytes", () => {
	const content = "alpha\nβeta\n";
	const decision = planReadCapture(content, { path: "notes.txt" }, content);
	assert.equal(decision.capture, true);
	if (!decision.capture) return;
	assert.equal(decision.plan.byteRange, undefined);
	assert.equal(decision.plan.sourceBytes, Buffer.byteLength(content));
	assert.equal(decision.plan.modelVisibleBytes, Buffer.byteLength(content));
	assert.match(decision.plan.expectedRawFingerprint, /^[0-9a-f]{64}$/);
});

test("bounded reads map one-indexed lines to UTF-8 byte ranges and count pagination", () => {
	const content = "zero\nαlpha\nbeta\ntail\n";
	const selected = "αlpha\nbeta";
	const visible = `${selected}\n\n[1 more lines in file. Use offset=4 to continue.]`;
	const decision = planReadCapture(
		content,
		{ path: "src/example.txt", offset: 2, limit: 2 },
		visible,
	);
	assert.equal(decision.capture, true);
	if (!decision.capture) return;
	assert.deepEqual(decision.plan.byteRange, {
		start: Buffer.byteLength("zero\n"),
		end: Buffer.byteLength("zero\nαlpha\nbeta"),
	});
	assert.equal(decision.plan.sourceBytes, Buffer.byteLength(selected));
	assert.equal(decision.plan.modelVisibleBytes, Buffer.byteLength(visible));
});

test("capture fails closed on drift, native truncation, invalid UTF-8, invalid ranges, and sensitive paths", () => {
	assert.equal(decodeUtf8(Uint8Array.from([0xff, 0xfe])), undefined);
	assert.deepEqual(planReadCapture("before\n", { path: "x" }, "after\n"), {
		capture: false,
		reason: "model-visible read result does not match the current file selection",
	});
	assert.deepEqual(
		planReadCapture("value", { path: "x" }, "value", { truncated: true }),
		{ capture: false, reason: "native read result was byte/line truncated" },
	);
	assert.equal(
		planReadCapture("value", { path: "x", offset: 0 }, "value").capture,
		false,
	);
	assert.equal(isSensitivePath(".env"), true);
	assert.equal(isSensitivePath("config/credentials.json"), true);
	assert.equal(isSensitivePath("secrets/token.txt"), true);
	assert.equal(isSensitivePath("src/lib.rs"), false);
});

test("repository paths are contained and normalized", () => {
	assert.equal(
		repositoryRelativePath("/repo", "/repo", "src/lib.rs"),
		"src/lib.rs",
	);
	assert.equal(
		repositoryRelativePath("/repo", "/repo/crate", "src/lib.rs"),
		"crate/src/lib.rs",
	);
	assert.equal(
		repositoryRelativePath("/repo", "/repo", "../outside"),
		undefined,
	);
	assert.equal(
		repositoryRelativePath("/repo", "/repo", "@README.md"),
		"README.md",
	);
});
