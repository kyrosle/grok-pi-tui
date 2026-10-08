/** Enhanced Bash execution for grok-pi. Pi owns the agent; Pager owns task UI. */
import { StringEnum, Type } from "@earendil-works/pi-ai";
import { createBashToolDefinition, type ExtensionAPI, type ExtensionContext } from "@earendil-works/pi-coding-agent";
import { type BackgroundTask, type BashParams, ORPHANED_SIGNAL, createBashControl,
  ensureTaskIds, jsonContent, killProcessTree, startTask, taskResult, waitForCompletion } from "./bash-tasks.ts";
import { buildBashPrompts } from "./prompts.ts";
import { MAX_TIMEOUT_SECONDS, resolveMaxWaitMs } from "./shared.ts";

function hostToolNameEnabled(name: string): boolean {
  const configured = process.env.PI_GROK_BUILTIN_TOOLS;
  if (configured !== undefined && !configured.split(",").map((s) => s.trim()).includes(name)) return false;
  return !(process.env.PI_GROK_EXCLUDE_TOOLS ?? "").split(",").map((s) => s.trim()).includes(name);
}

export default function (pi: ExtensionAPI) {
  const enabled = !["0", "false", "off", "no"].includes(process.env.PI_GROK_BASH?.trim().toLowerCase() ?? "1");
  if (!enabled || !hostToolNameEnabled("bash")) return;
  const maxWaitMs = resolveMaxWaitMs();
  const tasks = new Map<string, BackgroundTask>();
  const control = createBashControl(tasks);
  const syncTaskState = () => {
    control.sync();
    const count = [...tasks.values()].filter((task) => task.backgrounded && !task.completed).length;
    pi.events.emit("pi-grok:todo-backing", { source: "bash", count });
  };
  pi.on("session_start", syncTaskState);
  const nativeBash = createBashToolDefinition(process.cwd());
	const BashParameters = Type.Object({
		command: Type.String({
			description:
				"Exact bash to run. No trailing comments (# ...). Put the human-readable UI label in task_name instead.",
		}),
		timeout: Type.Optional(
			Type.Number({
				minimum: 0,
				maximum: MAX_TIMEOUT_SECONDS,
				description: "Timeout in seconds; omit or set 0 to disable the timeout",
			}),
		),
		is_background: Type.Optional(
			Type.Boolean({ description: "true = background task (returns task_id); false/omit = foreground" }),
		),
		// Required (like stock Grok `description`): Pager Execute cards prefer this over raw shell.
		task_name: Type.String({
			description:
				"Short human-readable UI title (3–8 words) for BOTH foreground and background. " +
				"Write it in the same language as the user's messages (not always English). " +
				"Especially required when the command is long, multi-pipeline, or hard to scan (>~40 chars). " +
				"Never put this label in command as a # comment.",
		}),
	});

	const bashPrompts = buildBashPrompts(
		nativeBash.description,
		nativeBash.promptGuidelines ?? [],
	);
	pi.registerTool({
		...nativeBash,
		parameters: BashParameters,
		description: bashPrompts.description,
		promptSnippet: bashPrompts.promptSnippet,
		promptGuidelines: bashPrompts.promptGuidelines,
		async execute(toolCallId, params: BashParams, signal, onUpdate, ctx: ExtensionContext) {
			if (signal?.aborted) throw new Error("aborted");
			const taskName = params.task_name?.trim() || undefined;
			if (params.is_background) {
				const task = await startTask(pi, {
					toolCallId,
					command: params.command,
					description: taskName,
					cwd: ctx.cwd,
					timeout: params.timeout,
					backgrounded: true,
					env: process.env,
					stateChanged: syncTaskState,
					ui: ctx.ui,
				});
				tasks.set(task.toolCallId, task);
				syncTaskState();
				return {
					content: jsonContent({ task_id: task.taskId, status: "running", output_file: task.outputFile }),
					details: {
						taskId: task.taskId,
						background: true,
						command: task.command,
						cwd: task.cwd,
						outputFile: task.outputFile,
						description: task.description,
					},
				};
			}

			let task: BackgroundTask | undefined;
			const managedBash = createBashToolDefinition(ctx.cwd, {
				operations: {
					exec: async (command, cwd, options) => {
						task = await startTask(pi, {
							toolCallId,
							command,
							cwd,
							timeout: options.timeout,
							autoBackgroundMs: maxWaitMs,
							backgrounded: false,
							description: taskName,
							env: options.env ?? process.env,
							onData: options.onData,
							stateChanged: syncTaskState,
							ui: ctx.ui,
						});
						tasks.set(toolCallId, task);
						const activeTask = task;
						return new Promise<{ exitCode: number | null }>((resolve, reject) => {
							const settle = (outcome: "completed" | "backgrounded") => {
								activeTask.foregroundSettler = undefined;
								activeTask.promote = undefined;
								options.signal?.removeEventListener("abort", aborted);
								if (outcome === "backgrounded") {
									resolve({ exitCode: 0 });
									return;
								}
								if (options.signal?.aborted) {
									reject(new Error("aborted"));
									return;
								}
								if (activeTask.timedOut) {
									reject(new Error(`timeout:${options.timeout}`));
									return;
								}
								resolve({ exitCode: activeTask.exitCode ?? null });
							};
							const aborted = () => killProcessTree(activeTask);
							activeTask.foregroundSettler = settle;
							activeTask.promote = () => {
								if (activeTask.completed || activeTask.backgrounded) return;
								if (activeTask.autoBackgroundHandle) clearTimeout(activeTask.autoBackgroundHandle);
								activeTask.autoBackgroundHandle = undefined;
								activeTask.backgrounded = true;
								syncTaskState();
								settle("backgrounded");
							};
							if (options.signal?.aborted) {
								aborted();
							} else {
								options.signal?.addEventListener("abort", aborted, { once: true });
							}
							syncTaskState();
							if (activeTask.completed) settle("completed");
						});
					},
				},
			});
			try {
				const result = await managedBash.execute(toolCallId, params, signal, onUpdate, ctx);
				if (!task?.backgrounded) return result;
				// Promotion must expose the task envelope in content (model-visible):
				// details alone never reach the agent over RPC.
				return {
					content: jsonContent({ task_id: task.taskId, status: "running", output_file: task.outputFile }),
					details: {
						...result.details,
						taskId: task.taskId,
						background: true,
						command: task.command,
						cwd: task.cwd,
						outputFile: task.outputFile,
						description: task.description,
					},
				};
			} finally {
				if (task && !task.backgrounded) {
					tasks.delete(task.toolCallId);
					syncTaskState();
				}
			}
		},
	});

	const findManagedTask = (taskId: string): BackgroundTask | undefined =>
		[...tasks.values()].find((task) => task.taskId === taskId);
	// A stale id (previous session, typo) must not discard results for the valid ids in the same batch.
	const selectManagedTasks = (ids: string[]) => {
		const found: BackgroundTask[] = [];
		const missing: string[] = [];
		for (const id of ids) {
			const managed = findManagedTask(id);
			if (managed) found.push(managed);
			else missing.push(id);
		}
		return { found, missing };
	};
	const capWaitMs = (timeoutMs: number | undefined) => {
		if (maxWaitMs === undefined) return timeoutMs;
		// An explicit 0 stays 0 (non-blocking snapshot); only an omitted value widens to the max-wait cap.
		if (timeoutMs === undefined) return maxWaitMs;
		return Math.min(timeoutMs, maxWaitMs);
	};

	pi.registerTool({
		name: "get_task_output",
		label: "get_task_output",
		description: "Get output for one or more background Bash tasks. Set timeout_ms to wait for completion; omit it to poll. Configured max-wait still caps a blocking wait.",
		parameters: Type.Object({
			task_ids: Type.Array(Type.String({ minLength: 1 })),
			timeout_ms: Type.Optional(Type.Number({ minimum: 0 })),
		}),
		async execute(_toolCallId, params, signal) {
			const ids = ensureTaskIds(params.task_ids);
			const { found, missing } = selectManagedTasks(ids);
			if (found.length === 0) return { content: jsonContent({ task_not_found: missing }) };
			if (params.timeout_ms && params.timeout_ms > 0) {
				await Promise.all(found.map((task) => waitForCompletion(task, capWaitMs(params.timeout_ms), signal)));
			}
			const results = found.map(taskResult);
			if (missing.length > 0) return { content: jsonContent({ task_not_found: missing, results }) };
			return { content: jsonContent(results.length === 1 ? results[0] : { mode: "wait_all", results }) };
		},
	});

	pi.registerTool({
		name: "wait_tasks",
		label: "wait_tasks",
		description: "Wait for background Bash tasks to finish. Configured max-wait returns current running state instead of blocking indefinitely.",
		parameters: Type.Object({
			task_ids: Type.Array(Type.String({ minLength: 1 })),
			mode: StringEnum(["wait_any", "wait_all"] as const),
			timeout_ms: Type.Optional(Type.Number({ minimum: 0 })),
		}),
		async execute(_toolCallId, params, signal) {
			const ids = ensureTaskIds(params.task_ids);
			const { found, missing } = selectManagedTasks(ids);
			if (found.length === 0) return { content: jsonContent({ task_not_found: missing }) };
			const waitMs = capWaitMs(params.timeout_ms);
			// timeout_ms=0 is an explicit non-blocking snapshot; only wait when a real window exists.
			if (waitMs === undefined || waitMs > 0) {
				const waits = found.map((task) => waitForCompletion(task, waitMs, signal));
				if (params.mode === "wait_any") await Promise.race(waits);
				else await Promise.all(waits);
			}
			const results = found.map(taskResult);
			if (missing.length > 0) return { content: jsonContent({ mode: params.mode, results, task_not_found: missing }) };
			return { content: jsonContent({ mode: params.mode, results }) };
		},
	});

	pi.registerTool({
		name: "kill_task",
		label: "kill_task",
		description: "Terminate a running background Bash task by task ID.",
		parameters: Type.Object({ task_id: Type.String({ minLength: 1 }) }),
		async execute(_toolCallId, params) {
			const managed = findManagedTask(params.task_id.trim());
			if (!managed) return { content: jsonContent({ task_not_found: params.task_id }) };
			if (managed.completed) return { content: jsonContent({ task_id: managed.taskId, outcome: "already_exited" }) };
			managed.explicitlyKilled = true;
			managed.signal = "killed";
			killProcessTree(managed);
			return { content: jsonContent({ task_id: managed.taskId, outcome: "killed" }) };
		},
	});

	pi.on("session_shutdown", () => {
		control.close();
		for (const task of tasks.values()) {
			if (task.completed) continue;
			if (task.autoBackgroundHandle) clearTimeout(task.autoBackgroundHandle);
			task.explicitlyKilled = true;
			task.signal = ORPHANED_SIGNAL;
			killProcessTree(task);
		}
	});
}
