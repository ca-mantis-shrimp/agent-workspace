import { Buffer } from "node:buffer";
import { createHash } from "node:crypto";
import { isAbsolute, relative, resolve, sep } from "node:path";

export interface ReadParameters {
	path: string;
	offset?: number;
	limit?: number;
}

export interface ReadCapturePlan {
	repositoryPath: string;
	byteRange?: { start: number; end: number };
	sourceBytes: number;
	modelVisibleBytes: number;
	expectedRawFingerprint: string;
}

export type ReadCaptureDecision =
	| { capture: true; plan: ReadCapturePlan }
	| { capture: false; reason: string };

const SENSITIVE_PATH_PATTERNS = [
	/(^|\/)\.env(?:\.|$)/i,
	/(^|\/)(?:secrets?|credentials?)(?:[/.]|$)/i,
	/(^|\/)\.ssh\//i,
	/(^|\/)\.aws\//i,
	/(^|\/)\.gnupg\//i,
	/\.(?:pem|key|p12|pfx)$/i,
];

const PAGINATION_NOTICE =
	/^\n\n\[\d+ more lines in file\. Use offset=\d+ to continue\.\]$/;

export function repositoryRelativePath(
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

export function decodeUtf8(bytes: Uint8Array): string | undefined {
	try {
		return new TextDecoder("utf-8", { fatal: true }).decode(bytes);
	} catch {
		return undefined;
	}
}

export function isSensitivePath(repositoryPath: string): boolean {
	const normalized = repositoryPath.replaceAll("\\", "/");
	return SENSITIVE_PATH_PATTERNS.some((pattern) => pattern.test(normalized));
}

export function planReadCapture(
	fileContent: string,
	parameters: ReadParameters,
	resultText: string,
	options: { truncated?: boolean } = {},
): ReadCaptureDecision {
	if (options.truncated) {
		return {
			capture: false,
			reason: "native read result was byte/line truncated",
		};
	}
	if (
		!Number.isInteger(parameters.offset ?? 1) ||
		(parameters.offset ?? 1) < 1
	) {
		return { capture: false, reason: "read offset is not a positive integer" };
	}
	if (
		parameters.limit !== undefined &&
		(!Number.isInteger(parameters.limit) || parameters.limit < 1)
	) {
		return { capture: false, reason: "read limit is not a positive integer" };
	}

	const lines = fileContent.split("\n");
	const startLine = (parameters.offset ?? 1) - 1;
	if (startLine >= lines.length) {
		return { capture: false, reason: "read starts beyond the current file" };
	}
	const endLine =
		parameters.limit === undefined
			? lines.length
			: Math.min(lines.length, startLine + parameters.limit);
	const selectedText = lines.slice(startLine, endLine).join("\n");
	const remainder = resultText.slice(selectedText.length);
	if (
		!resultText.startsWith(selectedText) ||
		(remainder !== "" && !PAGINATION_NOTICE.test(remainder))
	) {
		return {
			capture: false,
			reason:
				"model-visible read result does not match the current file selection",
		};
	}

	const prefix =
		startLine === 0 ? "" : `${lines.slice(0, startLine).join("\n")}\n`;
	const start = Buffer.byteLength(prefix, "utf8");
	const sourceBytes = Buffer.byteLength(selectedText, "utf8");
	const end = start + sourceBytes;
	const wholeFile =
		parameters.offset === undefined &&
		parameters.limit === undefined &&
		start === 0 &&
		end === Buffer.byteLength(fileContent, "utf8");

	return {
		capture: true,
		plan: {
			repositoryPath: parameters.path,
			byteRange: wholeFile ? undefined : { start, end },
			sourceBytes,
			modelVisibleBytes: Buffer.byteLength(resultText, "utf8"),
			expectedRawFingerprint: createHash("sha256").update(selectedText, "utf8").digest("hex"),
		},
	};
}
