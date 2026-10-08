export type ToolPromptBundle = { description: string; promptSnippet: string; promptGuidelines: string[]; };

const BASH_TASK_NAME_DESCRIPTION =
	"Always set task_name: a short human-readable UI title (3–8 words) in the user's language " +
	"(match the language of their messages). Required for every call — foreground and background. " +
	"This is what the terminal UI shows instead of the raw shell, especially for long/complex commands. " +
	"Never annotate command with # comments — put the label in task_name only.";
const BASH_BACKGROUND_WAIT_GUIDELINE =
	"A long-running foreground Bash call may be automatically backgrounded and return a task_id. When wait_tasks/get_task_output returns a running task at the configured max-wait cap, continue the agent loop and call it again if the result is still needed.";

export function buildBashPrompts(nativeDescription: string, nativeGuidelines: readonly string[] = []): ToolPromptBundle {
  return {
    description: `${nativeDescription} ${BASH_TASK_NAME_DESCRIPTION}`,
    promptSnippet: "Run shell commands, scripts, builds and background tasks. Always pass task_name in the user's language.",
    promptGuidelines: [...nativeGuidelines, BASH_BACKGROUND_WAIT_GUIDELINE],
  };
}
