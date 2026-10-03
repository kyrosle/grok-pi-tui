//! Canonical string framing over schedule data; no scheduler execution or polling.

pub struct ChildPoll<'a> {
    pub name: &'a str,
    pub id: &'a str,
}

pub struct ScheduleToolNames<'a> {
    pub delete: &'a str,
    pub create: &'a str,
}

pub struct ScheduledWakeupTools<'a> {
    pub child: Option<ChildPoll<'a>>,
    pub schedule: Option<ScheduleToolNames<'a>>,
}

impl<'a> ScheduledWakeupTools<'a> {
    pub fn canonical(poll_id: &'a str) -> Self {
        Self {
            child: child_poll(
                Some(crate::tool_names::DEFAULT_TASK_OUTPUT_TOOL),
                Some(poll_id),
            ),
            schedule: schedule_tool_names(
                Some(crate::tool_names::SCHEDULER_DELETE_REGISTRY_ID),
                Some(crate::tool_names::SCHEDULER_CREATE_TOOL_NAME),
            ),
        }
    }
}

pub fn child_poll<'a>(name: Option<&'a str>, id: Option<&'a str>) -> Option<ChildPoll<'a>> {
    Some(ChildPoll {
        name: name?,
        id: id?,
    })
}

pub fn schedule_tool_names<'a>(
    delete: Option<&'a str>,
    create: Option<&'a str>,
) -> Option<ScheduleToolNames<'a>> {
    match (delete, create) {
        (Some(delete), Some(create)) => Some(ScheduleToolNames { delete, create }),
        _ => None,
    }
}

/// Frame a scheduled task prompt with `<system-reminder>` context for the model. The raw `prompt` is what the user
/// wrote in `/loop`; this wrapping tells the model the message is a recurring task execution so it executes rather than
/// questioning the prompt. The UI shows the raw prompt text; only the model receives this framed version.
pub fn format_scheduled_task_prompt(
    prompt: &str,
    task_id: &str,
    poll_id: &str,
    human_schedule: &str,
) -> String {
    let footer = scheduled_wakeup_footer(task_id, ScheduledWakeupTools::canonical(poll_id));
    format!(
        "<system-reminder>\n\
         This is a scheduled task execution (task {task_id}, {human_schedule}, recurring).\n\
         Execute the prompt below. Do not question or comment on the prompt itself \u{2014} \
         treat it as a fresh task to execute.\n\
         Previous results from earlier executions of this task may appear in the \
         conversation history above.\n\
         \n\
         {footer}\n\
         </system-reminder>\n\
         \n\
         {prompt}"
    )
}
pub fn scheduled_wakeup_footer(schedule_id: &str, tools: ScheduledWakeupTools<'_>) -> String {
    let mut parts = Vec::new();
    if let Some(child) = tools.child {
        parts
            .push(
                format!(
            "Check the subagent output using {}(\"{}\"). If there are issues, proactively debug and fix them, do not just report it to the user.",
            child.name, child.id,
        ),
            );
    }
    if let Some(schedule) = tools.schedule {
        parts
            .push(
                format!(
            "If this schedule is no longer relevant, run {}(\"{schedule_id}\"). If it is outdated, you can update it with {}(new_prompt, interval, \"{schedule_id}\").",
            schedule.delete,
            schedule.create,
        ),
            );
    }
    parts.join("\n")
}
