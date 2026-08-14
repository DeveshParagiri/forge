//! Stable, browser-facing projection of a live pager session.
//!
//! This is intentionally a view model rather than serialized ACP or ratatui
//! output.  The pager remains the source of truth, while the web client gets
//! stable entry IDs and typed content that can be reconstructed after any
//! disconnect.

use std::collections::{HashMap, HashSet};

use serde::Serialize;
use xai_grok_shell::remote_control::RemoteWorkDisclosure;

use crate::app::agent::{AgentState, BgTaskStatus};
use crate::app::agent_view::AgentView;
use crate::forge::remote_usage::RemoteUsageSnapshot;
use crate::scrollback::block::RenderBlock;
use crate::scrollback::blocks::{
    BgTaskKind, SessionEvent, SubagentBlockKind, ToolCallBlock, WorkflowBlockStatus,
};

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteSessionSnapshot {
    pub session_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    pub status: RemoteSessionStatus,
    pub transcript: Vec<RemoteTimelineItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_model: Option<RemoteModel>,
    pub available_models: Vec<RemoteModel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<RemoteReasoningEffort>,
    #[serde(skip_serializing_if = "is_false")]
    pub model_switch_pending: bool,
    pub fast_mode: RemoteFastMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan_mode: Option<RemotePlanMode>,
    pub active_interactions: Vec<RemoteInteraction>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub queue: Vec<RemoteQueueItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub task_state: Option<RemoteTaskState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<RemoteUsageSnapshot>,
    pub capabilities: RemoteCapabilities,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RemoteSessionStatus {
    Idle,
    Running,
    WaitingForInput,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteModel {
    pub id: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reasoning_effort: Option<RemoteModelReasoningEffort>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteModelReasoningEffort {
    pub options: Vec<RemoteReasoningOption>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteReasoningEffort {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current: Option<String>,
    pub options: Vec<RemoteReasoningOption>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteReasoningOption {
    pub id: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemotePlanMode {
    pub active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub plan: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteQueueItem {
    pub id: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<usize>,
    pub source: RemoteQueueSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<String>,
    pub actions: RemoteQueueActions,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RemoteQueueSource {
    Local,
    Shared,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteQueueActions {
    pub edit: bool,
    pub steer: bool,
    pub cancel: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteTaskState {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background_count: Option<usize>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteCapabilities {
    pub prompt: bool,
    pub new_session: bool,
    pub cancel: bool,
    pub set_model: bool,
    pub reasoning: bool,
    pub btw: bool,
    pub resolve_interactions: bool,
    pub usage: bool,
    pub fast_mode: bool,
    pub queue_control: bool,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteFastMode {
    pub supported: bool,
    pub enabled: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub pending: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum RemoteTimelineItem {
    User {
        id: String,
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<RemoteItemStatus>,
        #[serde(rename = "createdAt", skip_serializing_if = "Option::is_none")]
        created_at: Option<String>,
    },
    Assistant {
        id: String,
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<RemoteItemStatus>,
        #[serde(rename = "createdAt", skip_serializing_if = "Option::is_none")]
        created_at: Option<String>,
    },
    Reasoning {
        id: String,
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<RemoteItemStatus>,
        #[serde(rename = "createdAt", skip_serializing_if = "Option::is_none")]
        created_at: Option<String>,
    },
    System {
        id: String,
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<RemoteItemStatus>,
        #[serde(rename = "createdAt", skip_serializing_if = "Option::is_none")]
        created_at: Option<String>,
        #[serde(rename = "workDisclosure", skip_serializing_if = "Option::is_none")]
        work_disclosure: Option<RemoteWorkDisclosure>,
    },
    Error {
        id: String,
        text: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<RemoteItemStatus>,
        #[serde(rename = "createdAt", skip_serializing_if = "Option::is_none")]
        created_at: Option<String>,
    },
    Tool {
        id: String,
        title: String,
        status: RemoteItemStatus,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        input: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        output: Option<String>,
        #[serde(rename = "createdAt", skip_serializing_if = "Option::is_none")]
        created_at: Option<String>,
    },
    Plan {
        id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        text: Option<String>,
        #[serde(skip_serializing_if = "Vec::is_empty")]
        steps: Vec<RemotePlanStep>,
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<RemoteItemStatus>,
        #[serde(rename = "createdAt", skip_serializing_if = "Option::is_none")]
        created_at: Option<String>,
    },
    Btw {
        id: String,
        question: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        response: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        status: Option<RemoteItemStatus>,
        #[serde(rename = "createdAt", skip_serializing_if = "Option::is_none")]
        created_at: Option<String>,
    },
    Background {
        id: String,
        title: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        detail: Option<String>,
        status: RemoteItemStatus,
        #[serde(rename = "createdAt", skip_serializing_if = "Option::is_none")]
        created_at: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemotePlanStep {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<RemoteItemStatus>,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RemoteItemStatus {
    Pending,
    Running,
    Complete,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum RemoteInteraction {
    Permission {
        #[serde(rename = "interactionId")]
        interaction_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        status: RemoteInteractionStatus,
        options: Vec<RemotePermissionOption>,
        #[serde(rename = "allowFollowup", skip_serializing_if = "is_false")]
        allow_followup: bool,
    },
    Question {
        #[serde(rename = "interactionId")]
        interaction_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        status: RemoteInteractionStatus,
        questions: Vec<RemoteQuestion>,
    },
    Plan {
        #[serde(rename = "interactionId")]
        interaction_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        status: RemoteInteractionStatus,
        plan: String,
        #[serde(rename = "allowFeedback")]
        allow_feedback: bool,
    },
    Unsupported {
        #[serde(rename = "interactionId")]
        interaction_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        description: Option<String>,
        status: RemoteInteractionStatus,
        #[serde(skip_serializing_if = "Option::is_none")]
        method: Option<String>,
    },
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RemoteInteractionStatus {
    Pending,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemotePermissionOption {
    pub id: String,
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteQuestion {
    pub prompt: String,
    pub options: Vec<RemoteQuestionOption>,
    #[serde(skip_serializing_if = "is_false")]
    pub multiple: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub allow_freeform: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RemoteQuestionOption {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Assign stable, opaque browser IDs to pager interaction keys for the life of
/// one pairing. The map belongs to the bridge generation and is discarded on
/// re-arm, so a stale browser cannot address a future pairing's request.
pub(crate) fn project_session(
    agent: &AgentView,
    interaction_ids: &mut HashMap<String, String>,
) -> Option<RemoteSessionSnapshot> {
    let session_id = agent.session.session_id.as_ref()?.0.to_string();
    let mut active_keys = HashSet::new();
    let active_interactions = project_interactions(agent, interaction_ids, &mut active_keys);
    interaction_ids.retain(|key, _| active_keys.contains(key));

    let available_models = agent
        .session
        .models
        .available
        .iter()
        .map(|(id, info)| {
            let options = agent.session.models.reasoning_effort_options_for(id);
            RemoteModel {
                id: id.0.to_string(),
                label: info.name.clone(),
                description: info.description.clone(),
                reasoning_effort: (!options.is_empty()).then(|| RemoteModelReasoningEffort {
                    options: options
                        .into_iter()
                        .map(|option| RemoteReasoningOption {
                            id: option.id,
                            label: option.label,
                            description: option.description,
                        })
                        .collect(),
                }),
            }
        })
        .collect::<Vec<_>>();
    let current_model = agent.session.models.current.as_ref().map(|id| {
        let info = agent.session.models.available.get(id);
        let options = agent.session.models.reasoning_effort_options_for(id);
        RemoteModel {
            id: id.0.to_string(),
            label: info
                .map(|info| info.name.clone())
                .unwrap_or_else(|| id.0.to_string()),
            description: info.and_then(|info| info.description.clone()),
            reasoning_effort: (!options.is_empty()).then(|| RemoteModelReasoningEffort {
                options: options
                    .into_iter()
                    .map(|option| RemoteReasoningOption {
                        id: option.id,
                        label: option.label,
                        description: option.description,
                    })
                    .collect(),
            }),
        }
    });
    let effort_options = agent.session.models.reasoning_effort_options();
    let current_effort_id = agent.session.models.reasoning_effort.and_then(|current| {
        effort_options
            .iter()
            .find(|option| option.value == current)
            .map(|option| option.id.clone())
    });
    let reasoning_effort = (!effort_options.is_empty()).then(|| RemoteReasoningEffort {
        current: current_effort_id,
        options: effort_options
            .into_iter()
            .map(|option| RemoteReasoningOption {
                id: option.id,
                label: option.label,
                description: option.description,
            })
            .collect(),
    });
    let has_interaction = !active_interactions.is_empty();
    let status = if has_interaction {
        RemoteSessionStatus::WaitingForInput
    } else {
        match agent.session.state {
            AgentState::Idle => RemoteSessionStatus::Idle,
            AgentState::TurnRunning
            | AgentState::TurnCancelling
            | AgentState::CommandRunning { .. }
            | AgentState::CommandCancelling { .. } => RemoteSessionStatus::Running,
        }
    };
    // The canonical execution order is server-owned shared rows first, then
    // client-local mirrors. Their source-relative positions are not a common
    // coordinate space, so normalize one global order for remote clients.
    let mut shared_queue = agent.shared_queue.iter().collect::<Vec<_>>();
    shared_queue.sort_by_key(|item| item.position);
    let mut queue = shared_queue
        .into_iter()
        .enumerate()
        .map(|(position, item)| RemoteQueueItem {
            id: item.id.clone(),
            text: item.text.clone(),
            position: Some(position),
            source: RemoteQueueSource::Shared,
            version: Some(item.version),
            kind: Some(item.kind.clone()),
            actions: RemoteQueueActions {
                edit: item.kind == "prompt",
                steer: agent.session.state.is_turn_running(),
                cancel: true,
            },
        })
        .collect::<Vec<_>>();
    let shared_count = queue.len();
    queue.extend(
        agent
            .session
            .pending_prompts
            .iter()
            .enumerate()
            .map(|(local_position, item)| RemoteQueueItem {
                id: format!("local-{}", item.id),
                text: item.text.clone(),
                position: Some(shared_count + local_position),
                source: RemoteQueueSource::Local,
                version: None,
                kind: Some(item.kind.as_label().into()),
                actions: RemoteQueueActions {
                    edit: false,
                    steer: false,
                    cancel: false,
                },
            }),
    );

    let running_background = agent
        .session
        .bg_tasks
        .values()
        .filter(|task| matches!(task.status, BgTaskStatus::Running))
        .count();
    let task_state = (!agent.session.bg_tasks.is_empty()).then(|| RemoteTaskState {
        label: (running_background > 0).then(|| format!("{running_background} running")),
        progress: None,
        background_count: Some(running_background),
    });
    let plan = agent
        .plan_approval_view
        .as_ref()
        .and_then(|view| view.plan_content.clone())
        .or_else(|| agent.latest_inline_plan_content.clone());
    let plan_active = agent.plan_mode_pending.unwrap_or(agent.plan_mode_active);
    let fast_mode_supported = crate::forge::fast_mode::is_supported(&agent.session.models);

    let mut transcript =
        project_transcript(agent.scrollback.iter_entries().map(|(_, entry)| entry));
    if let Some(btw) = agent.btw_state.as_ref() {
        let question = btw.question().to_owned();
        // Dismissal persists the overlay and clears `btw_state` in one pager
        // reducer. While state is present it is always a distinct live request,
        // even when the same question appeared earlier in the transcript.
        use crate::views::btw_overlay::BtwOverlayState;
        let (response, status) = match btw {
            BtwOverlayState::Loading { .. } => (None, RemoteItemStatus::Running),
            BtwOverlayState::Done { content, .. } => {
                (Some(content.text()), RemoteItemStatus::Complete)
            }
            BtwOverlayState::Error { error, .. } => (Some(error.clone()), RemoteItemStatus::Failed),
        };
        transcript.push(RemoteTimelineItem::Btw {
            id: "btw:overlay".into(),
            question,
            response,
            status: Some(status),
            created_at: None,
        });
    }
    if let Some(plan_text) = plan.as_ref() {
        let plan_id = agent
            .plan_approval_view
            .as_ref()
            .map(|view| format!("plan:{}", view.tool_call_id))
            .unwrap_or_else(|| "plan:current".into());
        transcript.push(RemoteTimelineItem::Plan {
            id: plan_id,
            title: Some("Plan".into()),
            text: Some(plan_text.clone()),
            steps: Vec::new(),
            status: Some(if agent.plan_approval_view.is_some() {
                RemoteItemStatus::Pending
            } else {
                RemoteItemStatus::Complete
            }),
            created_at: None,
        });
    }

    Some(RemoteSessionSnapshot {
        session_id,
        title: agent
            .display_name
            .clone()
            .or_else(|| agent.generated_session_title.clone()),
        cwd: Some(agent.session.cwd.to_string_lossy().into_owned()),
        status,
        transcript,
        current_model,
        available_models,
        reasoning_effort,
        model_switch_pending: agent.session.model_switch_pending,
        fast_mode: RemoteFastMode {
            supported: fast_mode_supported,
            enabled: fast_mode_supported && agent.session.models.fast_mode,
            pending: fast_mode_supported && agent.session.models.fast_mode_pending(),
        },
        plan_mode: Some(RemotePlanMode {
            active: plan_active,
            plan,
        }),
        active_interactions,
        queue,
        task_state,
        usage: agent.remote_usage.snapshot().cloned(),
        capabilities: RemoteCapabilities {
            prompt: !agent.session.loading_replay,
            new_session: !agent.session.loading_replay,
            cancel: agent.session.state.is_busy(),
            set_model: !agent.session.models.available.is_empty(),
            reasoning: agent.session.models.reasoning_effort_options().len() > 1,
            btw: agent.btw_state.is_none(),
            resolve_interactions: true,
            usage: true,
            fast_mode: fast_mode_supported,
            queue_control: true,
        },
    })
}

fn project_interactions(
    agent: &AgentView,
    interaction_ids: &mut HashMap<String, String>,
    active_keys: &mut HashSet<String>,
) -> Vec<RemoteInteraction> {
    let mut out = Vec::new();
    if let Some(permission) = agent.permission_queue.front() {
        let tool_call_id = permission
            .request
            .request
            .tool_call
            .tool_call_id
            .0
            .to_string();
        let key = format!("permission:{tool_call_id}");
        let interaction_id = opaque_interaction_id(&key, interaction_ids, active_keys);
        let mut description = permission.description.clone();
        if let Some(command) = permission.bash_command_raw.as_ref() {
            description.push(command.clone());
        }
        out.push(RemoteInteraction::Permission {
            interaction_id,
            title: Some(permission.title.clone()),
            description: (!description.is_empty()).then(|| description.join("\n")),
            status: RemoteInteractionStatus::Pending,
            options: permission
                .options
                .iter()
                .map(|option| RemotePermissionOption {
                    id: option.option_id.0.to_string(),
                    label: option.name.clone(),
                    description: None,
                })
                .collect(),
            allow_followup: permission.options.iter().any(|option| {
                option.kind == agent_client_protocol::PermissionOptionKind::RejectOnce
            }),
        });
    }
    if let Some(question) = agent
        .question_view
        .as_ref()
        .filter(|view| view.response_tx.is_some())
    {
        let allow_freeform = !question.no_freeform;
        let key = format!("question:{}", question.tool_call_id);
        let interaction_id = opaque_interaction_id(&key, interaction_ids, active_keys);
        out.push(RemoteInteraction::Question {
            interaction_id,
            title: Some("Forge needs your input".into()),
            description: None,
            status: RemoteInteractionStatus::Pending,
            questions: question
                .questions
                .iter()
                .map(|question| RemoteQuestion {
                    prompt: question.question.clone(),
                    options: question
                        .options
                        .iter()
                        .map(|option| RemoteQuestionOption {
                            label: option.label.clone(),
                            description: Some(option.description.clone()).filter(|s| !s.is_empty()),
                        })
                        .collect(),
                    multiple: question.multi_select.unwrap_or(false),
                    allow_freeform,
                })
                .collect(),
        });
    }
    if let Some(plan) = agent
        .plan_approval_view
        .as_ref()
        .filter(|view| view.response_tx.is_some())
    {
        let key = format!("plan:{}", plan.tool_call_id);
        let interaction_id = opaque_interaction_id(&key, interaction_ids, active_keys);
        out.push(RemoteInteraction::Plan {
            interaction_id,
            title: Some("Review plan".into()),
            description: None,
            status: RemoteInteractionStatus::Pending,
            plan: plan.plan_content.clone().unwrap_or_default(),
            allow_feedback: true,
        });
    }
    if let Some(cancel) = agent.cancel_turn_view.as_ref() {
        let key = format!(
            "cancel_subagents:{}",
            agent
                .session
                .current_prompt_id
                .as_deref()
                .unwrap_or("current")
        );
        let interaction_id = opaque_interaction_id(&key, interaction_ids, active_keys);
        out.push(RemoteInteraction::Unsupported {
            interaction_id,
            title: Some("Stop running subagents?".into()),
            description: Some(format!(
                "Choose in the Forge TUI whether to stop {} running subagent{}.",
                cancel.running_count,
                if cancel.running_count == 1 { "" } else { "s" }
            )),
            status: RemoteInteractionStatus::Pending,
            method: Some("cancelSubagents".into()),
        });
    }
    out
}

fn opaque_interaction_id(
    key: &str,
    interaction_ids: &mut HashMap<String, String>,
    active_keys: &mut HashSet<String>,
) -> String {
    active_keys.insert(key.to_owned());
    interaction_ids
        .entry(key.to_owned())
        .or_insert_with(|| uuid::Uuid::new_v4().to_string())
        .clone()
}

#[derive(Default)]
struct RemoteTurnProjection {
    active: bool,
    work_item_ids: Vec<String>,
    final_response_item_id: Option<String>,
}

impl RemoteTurnProjection {
    fn begin(&mut self) {
        self.active = true;
        self.work_item_ids.clear();
        self.final_response_item_id = None;
    }

    fn finish(&mut self) {
        self.active = false;
        self.work_item_ids.clear();
        self.final_response_item_id = None;
    }

    fn work_disclosure(&self, elapsed: std::time::Duration) -> RemoteWorkDisclosure {
        RemoteWorkDisclosure {
            duration_ms: u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
            final_response_item_id: self
                .active
                .then(|| self.final_response_item_id.clone())
                .flatten(),
            work_item_ids: self
                .active
                .then(|| self.work_item_ids.clone())
                .unwrap_or_default(),
        }
    }
}

fn project_transcript<'a>(
    entries: impl IntoIterator<Item = &'a crate::scrollback::entry::ScrollbackEntry>,
) -> Vec<RemoteTimelineItem> {
    let mut turn = RemoteTurnProjection::default();
    let mut transcript = Vec::new();

    for entry in entries {
        if is_remote_pairing_notice(entry) {
            continue;
        }

        let id = entry.id.value().to_string();
        let mut work_disclosure = None;
        match &entry.block {
            RenderBlock::UserPrompt(block) if !block.is_interjection => turn.begin(),
            RenderBlock::UserPrompt(_) => {}
            RenderBlock::AgentMessage(_) if turn.active => {
                turn.final_response_item_id = Some(id);
            }
            RenderBlock::Thinking(_)
            | RenderBlock::ToolCall(_)
            | RenderBlock::BgTask(_)
            | RenderBlock::Subagent(_)
            | RenderBlock::Workflow(_)
                if turn.active =>
            {
                turn.work_item_ids.push(id);
            }
            RenderBlock::SessionEvent(block) => match &block.event {
                SessionEvent::TurnCompleted { elapsed } => {
                    work_disclosure = elapsed.map(|elapsed| turn.work_disclosure(elapsed));
                    turn.finish();
                }
                event if event.is_turn_terminal() => turn.finish(),
                _ => {}
            },
            _ => {}
        }

        transcript.push(project_entry_with_work_disclosure(entry, work_disclosure));
    }

    transcript
}

fn project_entry(entry: &crate::scrollback::entry::ScrollbackEntry) -> RemoteTimelineItem {
    project_entry_with_work_disclosure(entry, None)
}

fn project_entry_with_work_disclosure(
    entry: &crate::scrollback::entry::ScrollbackEntry,
    work_disclosure: Option<RemoteWorkDisclosure>,
) -> RemoteTimelineItem {
    let id = entry.id.value().to_string();
    let created_at = entry.created_at.as_ref().map(|value| value.to_rfc3339());
    let status = entry_status(entry);
    match &entry.block {
        RenderBlock::UserPrompt(block) => RemoteTimelineItem::User {
            id,
            text: block.text.clone(),
            status: Some(status),
            created_at,
        },
        RenderBlock::AgentMessage(block) => RemoteTimelineItem::Assistant {
            id,
            text: block.text(),
            status: Some(status),
            created_at,
        },
        RenderBlock::Thinking(block) => RemoteTimelineItem::Reasoning {
            id,
            text: block.text(),
            status: Some(status),
            created_at,
        },
        RenderBlock::ToolCall(block) => project_tool(id, created_at, status, block),
        RenderBlock::System(block) => RemoteTimelineItem::System {
            id,
            text: block.text.clone(),
            status: Some(status),
            created_at,
            work_disclosure: None,
        },
        RenderBlock::SessionEvent(block) => {
            let text = block.event.message();
            if matches!(status, RemoteItemStatus::Failed) {
                RemoteTimelineItem::Error {
                    id,
                    text,
                    status: Some(status),
                    created_at,
                }
            } else {
                RemoteTimelineItem::System {
                    id,
                    text,
                    status: Some(status),
                    created_at,
                    work_disclosure,
                }
            }
        }
        RenderBlock::BgTask(block) => RemoteTimelineItem::Background {
            id,
            title: block
                .description
                .clone()
                .unwrap_or_else(|| block.command.clone()),
            detail: Some(format!("Task {}", block.task_id)),
            status: match block.kind {
                BgTaskKind::Started => RemoteItemStatus::Running,
                BgTaskKind::Completed { .. } => RemoteItemStatus::Complete,
                BgTaskKind::Failed { .. } => RemoteItemStatus::Failed,
            },
            created_at,
        },
        RenderBlock::Subagent(block) => RemoteTimelineItem::Background {
            id,
            title: block.description.clone(),
            detail: block
                .activity_label
                .clone()
                .or_else(|| Some(block.subagent_type.clone())),
            status: match block.kind {
                SubagentBlockKind::Started => RemoteItemStatus::Running,
                SubagentBlockKind::Completed { .. } => RemoteItemStatus::Complete,
                SubagentBlockKind::Failed { .. } => RemoteItemStatus::Failed,
                SubagentBlockKind::Cancelled { .. } => RemoteItemStatus::Cancelled,
            },
            created_at,
        },
        RenderBlock::Workflow(block) => RemoteTimelineItem::Background {
            id,
            title: block.name.clone(),
            detail: Some(block.objective.clone()),
            status: match block.status {
                WorkflowBlockStatus::Running => RemoteItemStatus::Running,
                WorkflowBlockStatus::Done { .. } => RemoteItemStatus::Complete,
                WorkflowBlockStatus::Failed { .. } => RemoteItemStatus::Failed,
                WorkflowBlockStatus::Cancelled { .. } => RemoteItemStatus::Cancelled,
                WorkflowBlockStatus::Paused { .. } => RemoteItemStatus::Pending,
            },
            created_at,
        },
        RenderBlock::Btw(block) => RemoteTimelineItem::Btw {
            id,
            question: block.question.clone(),
            response: Some(block.content().text()),
            status: Some(status),
            created_at,
        },
        RenderBlock::Stub(block) => RemoteTimelineItem::System {
            id,
            text: block.text.clone(),
            status: Some(status),
            created_at,
            work_disclosure: None,
        },
        RenderBlock::ContextInfo(_) | RenderBlock::CreditLimit(_) => RemoteTimelineItem::System {
            id,
            text: entry.block.searchable_text().unwrap_or_default(),
            status: Some(status),
            created_at,
            work_disclosure: None,
        },
    }
}

fn is_remote_pairing_notice(entry: &crate::scrollback::entry::ScrollbackEntry) -> bool {
    const PREFIX: &str = "Forge Remote is live for this exact session until";
    matches!(&entry.block, RenderBlock::System(block) if block.text.starts_with(PREFIX))
}

fn project_tool(
    id: String,
    created_at: Option<String>,
    status: RemoteItemStatus,
    block: &ToolCallBlock,
) -> RemoteTimelineItem {
    let (title, detail, input, output) = match block {
        ToolCallBlock::Execute(tool) => (
            "Run command".to_owned(),
            tool.description.clone(),
            Some(tool.command.clone()),
            tool.error.clone().or_else(|| tool.output.clone()),
        ),
        ToolCallBlock::Read(tool) => (
            "Read file".to_owned(),
            Some(tool.path.clone()),
            tool.line_range.map(|range| range.to_string()),
            tool.error.clone().or_else(|| tool.content.clone()),
        ),
        ToolCallBlock::Edit(tool) => (
            "Edit file".to_owned(),
            Some(tool.path.clone()),
            Some(tool.copy_text()),
            tool.error.clone(),
        ),
        ToolCallBlock::ListDir(tool) => (
            "List directory".to_owned(),
            Some(tool.path.clone()),
            None,
            tool.error.clone().or_else(|| Some(tool.output.clone())),
        ),
        ToolCallBlock::Search(tool) => (
            "Search".to_owned(),
            Some(format!("{} matches", tool.match_count)),
            Some(tool.pattern.clone()),
            tool.error.clone().or_else(|| block.searchable_text()),
        ),
        ToolCallBlock::WebFetch(tool) => (
            "Fetch website".to_owned(),
            Some(tool.url.clone()),
            None,
            tool.error.clone().or_else(|| tool.output.clone()),
        ),
        ToolCallBlock::WebSearch(tool) => (
            tool.label.clone().unwrap_or_else(|| "Web search".into()),
            None,
            Some(tool.query.clone()),
            tool.error.clone().or_else(|| tool.content.clone()),
        ),
        ToolCallBlock::IntegrationSearch(tool) => (
            "Find integrations".to_owned(),
            Some(format!("{} results", tool.result_count)),
            Some(tool.query.clone()),
            tool.error.clone().or_else(|| tool.content.clone()),
        ),
        ToolCallBlock::UseTool(tool) => (
            tool.tool_name.clone(),
            None,
            serde_json::to_string_pretty(&tool.input_args).ok(),
            tool.error.clone().or_else(|| tool.output.clone()),
        ),
        ToolCallBlock::MemorySearch(tool) => (
            "Search memory".to_owned(),
            Some(format!("{} results", tool.results.len())),
            Some(tool.query.clone()),
            tool.error.clone().or_else(|| block.searchable_text()),
        ),
        ToolCallBlock::Skill(tool) | ToolCallBlock::Other(tool) => (
            tool.name.clone(),
            Some(tool.summary.clone()),
            None,
            tool.error.clone().or_else(|| tool.output.clone()),
        ),
        ToolCallBlock::Lifecycle(tool) => (
            "Session lifecycle".to_owned(),
            Some(tool.name.clone()),
            None,
            None,
        ),
    };
    RemoteTimelineItem::Tool {
        id,
        title,
        status,
        detail,
        input,
        output,
        created_at,
    }
}

fn entry_status(entry: &crate::scrollback::entry::ScrollbackEntry) -> RemoteItemStatus {
    if entry.is_pending_user_input {
        return RemoteItemStatus::Pending;
    }
    if entry.is_running {
        return RemoteItemStatus::Running;
    }
    match &entry.block {
        RenderBlock::ToolCall(tool) if !tool.is_success() => RemoteItemStatus::Failed,
        RenderBlock::SessionEvent(event)
            if matches!(
                event.event,
                SessionEvent::TurnFailed { .. }
                    | SessionEvent::RetryFailed { .. }
                    | SessionEvent::RequestFailed { .. }
                    | SessionEvent::CompactionFailed { .. }
            ) =>
        {
            RemoteItemStatus::Failed
        }
        RenderBlock::SessionEvent(event)
            if matches!(
                event.event,
                SessionEvent::TurnCancelled { .. } | SessionEvent::CompactionCancelled
            ) =>
        {
            RemoteItemStatus::Cancelled
        }
        _ => RemoteItemStatus::Complete,
    }
}

pub(crate) fn interaction_key_for_browser_id<'a>(
    interaction_ids: &'a HashMap<String, String>,
    browser_id: &str,
) -> Option<&'a str> {
    interaction_ids
        .iter()
        .find_map(|(key, id)| (id == browser_id).then_some(key.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scrollback::entry::{EntryId, ScrollbackEntry};
    use agent_client_protocol as acp;

    #[test]
    fn interaction_ids_are_opaque_stable_and_retired() {
        let mut ids = HashMap::new();
        let mut active = HashSet::new();
        let first = opaque_interaction_id("permission:tool-1", &mut ids, &mut active);
        let again = opaque_interaction_id("permission:tool-1", &mut ids, &mut active);
        assert_eq!(first, again);
        assert_ne!(first, "permission:tool-1");
        assert_eq!(
            interaction_key_for_browser_id(&ids, &first),
            Some("permission:tool-1")
        );
        active.clear();
        ids.retain(|key, _| active.contains(key));
        assert!(interaction_key_for_browser_id(&ids, &first).is_none());
    }

    #[test]
    fn snapshot_serializes_the_exact_camel_case_browser_contract() {
        let snapshot = RemoteSessionSnapshot {
            session_id: "s1".into(),
            title: None,
            cwd: None,
            status: RemoteSessionStatus::WaitingForInput,
            transcript: vec![RemoteTimelineItem::Assistant {
                id: "4".into(),
                text: "streaming".into(),
                status: Some(RemoteItemStatus::Running),
                created_at: None,
            }],
            current_model: None,
            available_models: Vec::new(),
            reasoning_effort: None,
            model_switch_pending: false,
            fast_mode: RemoteFastMode {
                supported: true,
                enabled: true,
                pending: false,
            },
            plan_mode: None,
            active_interactions: Vec::new(),
            queue: Vec::new(),
            task_state: None,
            usage: None,
            capabilities: RemoteCapabilities {
                prompt: true,
                new_session: true,
                cancel: true,
                set_model: false,
                reasoning: false,
                btw: true,
                resolve_interactions: true,
                usage: true,
                fast_mode: true,
                queue_control: true,
            },
        };
        let value = serde_json::to_value(snapshot).unwrap();
        assert_eq!(value["sessionId"], "s1");
        assert_eq!(value["status"], "waiting_for_input");
        assert!(value.get("activeInteractions").unwrap().is_array());
        assert_eq!(value["transcript"][0]["kind"], "assistant");
        assert_eq!(value["transcript"][0]["status"], "running");
        assert!(value.get("model_switch_pending").is_none());
        assert_eq!(
            value["fastMode"],
            serde_json::json!({"supported": true, "enabled": true})
        );
        assert_eq!(value["capabilities"]["fastMode"], true);
        assert_eq!(value["capabilities"]["newSession"], true);
    }

    #[test]
    fn fast_mode_projection_is_capability_gated_and_exposes_pending_state() {
        let mut agent = crate::app::agent_view::test_agent_view(
            Some("fast-session"),
            std::path::PathBuf::from("/workspace"),
        );
        let supported_id = acp::ModelId::new("fast-model");
        let supported = acp::ModelInfo::new(supported_id.clone(), "Fast Model").meta(
            serde_json::json!({"supportsFastMode": true})
                .as_object()
                .cloned(),
        );
        agent
            .session
            .models
            .available
            .insert(supported_id.clone(), supported);
        agent.session.models.set_current(supported_id, None);

        let initial = serde_json::to_value(
            project_session(&agent, &mut HashMap::new()).expect("supported session projects"),
        )
        .unwrap();
        assert_eq!(
            initial["fastMode"],
            serde_json::json!({"supported": true, "enabled": false})
        );
        assert_eq!(initial["capabilities"]["fastMode"], true);

        agent
            .session
            .models
            .begin_fast_mode_change()
            .expect("first mutation starts");
        let pending = serde_json::to_value(
            project_session(&agent, &mut HashMap::new()).expect("pending session projects"),
        )
        .unwrap();
        assert_eq!(pending["fastMode"]["pending"], true);

        let unsupported_id = acp::ModelId::new("standard-model");
        agent.session.models.available.insert(
            unsupported_id.clone(),
            acp::ModelInfo::new(unsupported_id.clone(), "Standard Model"),
        );
        agent.session.models.fast_mode = true;
        agent.session.models.set_current(unsupported_id, None);
        let unsupported = serde_json::to_value(
            project_session(&agent, &mut HashMap::new()).expect("unsupported session projects"),
        )
        .unwrap();
        assert_eq!(
            unsupported["fastMode"],
            serde_json::json!({"supported": false, "enabled": false})
        );
        assert_eq!(unsupported["capabilities"]["fastMode"], false);
    }

    #[test]
    fn mixed_scrollback_projects_typed_stable_timeline_items() {
        let user = ScrollbackEntry::with_id(EntryId::new(7), RenderBlock::user_prompt("hello"));
        let mut assistant = ScrollbackEntry::running_with_id(
            EntryId::new(8),
            RenderBlock::agent_message("partial answer"),
        );
        assistant.created_at = None;
        let reasoning = ScrollbackEntry::with_id(
            EntryId::new(9),
            RenderBlock::thinking("private reasoning summary"),
        );
        let tool = ScrollbackEntry::with_id(
            EntryId::new(10),
            RenderBlock::execute_with_output("cargo test", "ok", None::<String>),
        );
        let btw = ScrollbackEntry::with_id(
            EntryId::new(11),
            RenderBlock::Btw(crate::scrollback::blocks::BtwBlock::new(
                "side question",
                "side answer",
            )),
        );
        let background = ScrollbackEntry::running_with_id(
            EntryId::new(12),
            RenderBlock::BgTask(crate::scrollback::blocks::BgTaskBlock::started(
                "cargo check",
                "task-1",
            )),
        );

        let projected = [&user, &assistant, &reasoning, &tool, &btw, &background]
            .into_iter()
            .map(project_entry)
            .map(|item| serde_json::to_value(item).unwrap())
            .collect::<Vec<_>>();

        assert_eq!(projected[0]["id"], "7");
        assert_eq!(projected[0]["kind"], "user");
        assert_eq!(projected[1]["id"], "8");
        assert_eq!(projected[1]["kind"], "assistant");
        assert_eq!(projected[1]["status"], "running");
        assert_eq!(projected[2]["kind"], "reasoning");
        assert_eq!(projected[3]["kind"], "tool");
        assert_eq!(projected[3]["title"], "Run command");
        assert_eq!(projected[3]["input"], "cargo test");
        assert_eq!(projected[3]["output"], "ok");
        assert_eq!(projected[4]["kind"], "btw");
        assert_eq!(projected[4]["question"], "side question");
        assert_eq!(projected[5]["kind"], "background");
        assert_eq!(projected[5]["status"], "running");
    }

    #[test]
    fn completed_turn_disclosure_uses_exact_entry_ids_and_keeps_interjections_in_turn() {
        let entries = vec![
            ScrollbackEntry::with_id(EntryId::new(1), RenderBlock::user_prompt("start")),
            ScrollbackEntry::with_id(EntryId::new(2), RenderBlock::thinking("inspect")),
            ScrollbackEntry::with_id(EntryId::new(3), RenderBlock::agent_message("draft")),
            ScrollbackEntry::with_id(
                EntryId::new(4),
                RenderBlock::interjection_prompt("also verify tests"),
            ),
            ScrollbackEntry::with_id(
                EntryId::new(5),
                RenderBlock::execute_with_output("cargo test", "ok", None::<String>),
            ),
            ScrollbackEntry::with_id(
                EntryId::new(6),
                RenderBlock::BgTask(crate::scrollback::blocks::BgTaskBlock::started(
                    "cargo check",
                    "task-1",
                )),
            ),
            ScrollbackEntry::with_id(EntryId::new(7), RenderBlock::agent_message("final")),
            ScrollbackEntry::with_id(
                EntryId::new(8),
                RenderBlock::session_event(SessionEvent::TurnCompleted {
                    elapsed: Some(std::time::Duration::from_millis(1_250)),
                }),
            ),
        ];

        let transcript = project_transcript(entries.iter());
        let marker = serde_json::to_value(&transcript[7]).unwrap();
        assert_eq!(marker["kind"], "system");
        assert_eq!(marker["id"], "8");
        assert_eq!(
            marker["workDisclosure"],
            serde_json::json!({
                "durationMs": 1250,
                "finalResponseItemId": "7",
                "workItemIds": ["2", "5", "6"]
            })
        );
    }

    #[test]
    fn completed_turn_disclosure_resets_at_real_prompt_boundaries() {
        let entries = vec![
            ScrollbackEntry::with_id(EntryId::new(1), RenderBlock::user_prompt("abandoned")),
            ScrollbackEntry::with_id(EntryId::new(2), RenderBlock::thinking("old work")),
            ScrollbackEntry::with_id(EntryId::new(3), RenderBlock::user_prompt("current")),
            ScrollbackEntry::with_id(EntryId::new(4), RenderBlock::thinking("current work")),
            ScrollbackEntry::with_id(EntryId::new(5), RenderBlock::agent_message("answer")),
            ScrollbackEntry::with_id(
                EntryId::new(6),
                RenderBlock::session_event(SessionEvent::TurnCompleted {
                    elapsed: Some(std::time::Duration::from_secs(2)),
                }),
            ),
        ];

        let marker =
            serde_json::to_value(project_transcript(entries.iter()).last().unwrap()).unwrap();
        assert_eq!(marker["workDisclosure"]["finalResponseItemId"], "5");
        assert_eq!(
            marker["workDisclosure"]["workItemIds"],
            serde_json::json!(["4"])
        );
    }

    #[test]
    fn arbitrary_worked_for_text_never_gains_typed_disclosure() {
        let entry =
            ScrollbackEntry::with_id(EntryId::new(1), RenderBlock::system("Worked for 9m 41s"));
        let item = serde_json::to_value(project_transcript([&entry]).pop().unwrap()).unwrap();
        assert_eq!(item["kind"], "system");
        assert!(item.get("workDisclosure").is_none());
    }

    #[test]
    fn completed_turn_without_work_has_empty_typed_associations() {
        let entries = vec![
            ScrollbackEntry::with_id(EntryId::new(1), RenderBlock::user_prompt("do it")),
            ScrollbackEntry::with_id(
                EntryId::new(2),
                RenderBlock::session_event(SessionEvent::TurnCompleted {
                    elapsed: Some(std::time::Duration::from_millis(80)),
                }),
            ),
        ];

        let marker =
            serde_json::to_value(project_transcript(entries.iter()).last().unwrap()).unwrap();
        assert_eq!(
            marker["workDisclosure"],
            serde_json::json!({
                "durationMs": 80,
                "finalResponseItemId": null,
                "workItemIds": []
            })
        );
    }

    #[test]
    fn unknown_or_unsuccessful_turn_terminals_omit_work_disclosure() {
        let entries = vec![
            ScrollbackEntry::with_id(EntryId::new(1), RenderBlock::user_prompt("first")),
            ScrollbackEntry::with_id(EntryId::new(2), RenderBlock::thinking("work")),
            ScrollbackEntry::with_id(
                EntryId::new(3),
                RenderBlock::session_event(SessionEvent::TurnCompleted { elapsed: None }),
            ),
            ScrollbackEntry::with_id(EntryId::new(4), RenderBlock::user_prompt("second")),
            ScrollbackEntry::with_id(
                EntryId::new(5),
                RenderBlock::session_event(SessionEvent::TurnCancelled {
                    elapsed: std::time::Duration::from_secs(1),
                }),
            ),
            ScrollbackEntry::with_id(EntryId::new(6), RenderBlock::user_prompt("third")),
            ScrollbackEntry::with_id(
                EntryId::new(7),
                RenderBlock::session_event(SessionEvent::TurnHalted {
                    elapsed: std::time::Duration::from_secs(1),
                }),
            ),
            ScrollbackEntry::with_id(EntryId::new(8), RenderBlock::user_prompt("fourth")),
            ScrollbackEntry::with_id(
                EntryId::new(9),
                RenderBlock::session_event(SessionEvent::TurnFailed {
                    error: "request failed".into(),
                    elapsed: Some(std::time::Duration::from_secs(1)),
                }),
            ),
        ];

        let transcript = project_transcript(entries.iter())
            .into_iter()
            .map(|item| serde_json::to_value(item).unwrap())
            .collect::<Vec<_>>();
        assert!(transcript[2].get("workDisclosure").is_none());
        assert!(transcript[4].get("workDisclosure").is_none());
        assert!(transcript[6].get("workDisclosure").is_none());
        assert!(transcript[8].get("workDisclosure").is_none());
    }

    #[test]
    fn live_projection_omits_the_pairing_qr_that_opened_the_phone_client() {
        let mut agent = crate::app::agent_view::test_agent_view(
            Some("session-1"),
            std::path::PathBuf::from("/workspace"),
        );
        agent.scrollback.push_block(RenderBlock::system(
            "Forge Remote is live for this exact session until you run `/rc stop`.\n\n████ QR ████",
        ));
        agent
            .scrollback
            .push_block(RenderBlock::system("Keep this useful system message."));

        let snapshot = project_session(&agent, &mut HashMap::new()).expect("session projection");
        let transcript = serde_json::to_value(snapshot.transcript).unwrap();
        assert_eq!(transcript.as_array().unwrap().len(), 1);
        assert_eq!(transcript[0]["text"], "Keep this useful system message.");
    }

    #[test]
    fn live_projection_includes_models_queue_tasks_plan_and_all_interactions() {
        let mut agent = crate::app::agent_view::test_agent_view(
            Some("session-1"),
            std::path::PathBuf::from("/workspace"),
        );
        agent.session.state = AgentState::TurnRunning;
        agent.session.enqueue_prompt("queued locally".into());
        agent.session.enqueue_prompt("queued locally second".into());
        agent
            .shared_queue
            .push(crate::app::prompt_queue::QueueEntryWire {
                id: "shared-1".into(),
                version: 1,
                owner: None,
                last_editor: None,
                kind: "prompt".into(),
                text: "queued remotely".into(),
                combined_texts: None,
                position: 2,
            });
        agent
            .shared_queue
            .push(crate::app::prompt_queue::QueueEntryWire {
                id: "shared-0".into(),
                version: 3,
                owner: None,
                last_editor: None,
                kind: "prompt".into(),
                text: "queued remotely first".into(),
                combined_texts: None,
                position: 0,
            });

        let model_id = acp::ModelId::new("model-1");
        let model = acp::ModelInfo::new(model_id.clone(), "Model One").meta(
            serde_json::json!({
                "supportsReasoningEffort": true,
                "reasoningEffort": "high",
                "reasoningEfforts": [
                    {"id": "high", "value": "high", "label": "High"},
                    {"id": "deep", "value": "xhigh", "label": "Deep"}
                ]
            })
            .as_object()
            .cloned(),
        );
        agent
            .session
            .models
            .available
            .insert(model_id.clone(), model);
        agent.session.models.set_current(
            model_id,
            Some(xai_grok_shell::sampling::types::ReasoningEffort::Xhigh),
        );
        agent.session.model_switch_pending = true;

        agent.session.bg_tasks.insert(
            "task-1".into(),
            crate::app::agent::BgTaskState {
                task_id: "task-1".into(),
                tool_call_id: "bg-call-1".into(),
                command: "cargo check".into(),
                description: Some("Checking".into()),
                cwd: "/workspace".into(),
                output_file: String::new(),
                status: BgTaskStatus::Running,
                start_time: std::time::SystemTime::now(),
                end_time: None,
                exit_code: None,
                signal: None,
                stdout: String::new(),
                stdout_line_count: 0,
                truncated: false,
                pending_kill: false,
                kill_requested_at: None,
                scrollback_entry_id: None,
                is_monitor: false,
                restored_from_replay: false,
            },
        );

        let mut permission =
            crate::app::agent_view::test_fixtures::make_followup_permission_state();
        permission.title = "Run command?".into();
        permission.options = vec![
            acp::PermissionOption::new(
                acp::PermissionOptionId::new("allow-once"),
                String::from("Allow once"),
                acp::PermissionOptionKind::AllowOnce,
            ),
            acp::PermissionOption::new(
                acp::PermissionOptionId::new("reject-once"),
                String::from("Reject with feedback"),
                acp::PermissionOptionKind::RejectOnce,
            ),
        ];
        agent.permission_queue.push_back(permission);

        let question = xai_grok_tools::implementations::grok_build::ask_user_question::Question {
            question: String::from("Pick one?"),
            options: vec![
                xai_grok_tools::implementations::grok_build::ask_user_question::QuestionOption {
                    label: String::from("A"),
                    description: String::from("First option"),
                    preview: None,
                    id: None,
                },
            ],
            multi_select: Some(false),
            id: None,
        };
        let (question_tx, _question_rx) = tokio::sync::oneshot::channel();
        agent.question_view = Some(
            crate::views::question_view::QuestionViewState::with_response_tx(
                "question-call-1".into(),
                vec![question],
                crate::views::prompt_widget::StashedPrompt {
                    text: String::new(),
                    cursor: 0,
                    images: Vec::new(),
                    chip_elements: Vec::new(),
                    image_counter: 0,
                    image_undo_stash: Vec::new(),
                },
                Some(question_tx),
                crate::views::question_view::AskUserQuestionMode::Default,
            ),
        );
        agent.plan_approval_view =
            Some(crate::app::agent_view::test_fixtures::make_plan_approval_view_state());

        let snapshot = project_session(&agent, &mut HashMap::new()).unwrap();
        let value = serde_json::to_value(snapshot).unwrap();
        assert_eq!(value["sessionId"], "session-1");
        assert_eq!(value["status"], "waiting_for_input");
        assert_eq!(value["currentModel"]["id"], "model-1");
        assert_eq!(value["availableModels"][0]["label"], "Model One");
        assert_eq!(value["reasoningEffort"]["current"], "deep");
        assert_eq!(value["reasoningEffort"]["options"][1]["id"], "deep");
        assert_eq!(value["modelSwitchPending"], true);
        assert_eq!(value["queue"].as_array().unwrap().len(), 4);
        assert_eq!(value["queue"][0]["source"], "shared");
        assert_eq!(value["queue"][0]["id"], "shared-0");
        assert_eq!(value["queue"][0]["position"], 0);
        assert_eq!(
            value["queue"][0]["actions"],
            serde_json::json!({"edit": true, "steer": true, "cancel": true})
        );
        assert_eq!(value["queue"][1]["source"], "shared");
        assert_eq!(value["queue"][1]["version"], 1);
        assert_eq!(value["queue"][1]["kind"], "prompt");
        assert_eq!(
            value["queue"][1]["actions"],
            serde_json::json!({"edit": true, "steer": true, "cancel": true})
        );
        assert_eq!(value["queue"][2]["source"], "local");
        assert_eq!(value["queue"][2]["text"], "queued locally");
        assert_eq!(value["queue"][2]["position"], 2);
        assert_eq!(
            value["queue"][2]["actions"],
            serde_json::json!({"edit": false, "steer": false, "cancel": false})
        );
        assert_eq!(value["queue"][3]["source"], "local");
        assert_eq!(value["queue"][3]["text"], "queued locally second");
        assert_eq!(value["queue"][3]["position"], 3);
        assert_eq!(value["capabilities"]["queueControl"], true);
        assert_eq!(value["taskState"]["backgroundCount"], 1);
        assert_eq!(value["activeInteractions"].as_array().unwrap().len(), 3);
        assert!(
            value["activeInteractions"]
                .as_array()
                .unwrap()
                .iter()
                .all(|interaction| interaction["interactionId"]
                    .as_str()
                    .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok()))
        );
        assert!(
            value["transcript"]
                .as_array()
                .unwrap()
                .iter()
                .any(|item| item["kind"] == "plan" && item["status"] == "pending")
        );
    }

    #[test]
    fn btw_overlay_projects_loading_done_and_error_without_laptop_dismissal() {
        let mut agent = crate::app::agent_view::test_agent_view(
            Some("btw-session"),
            std::path::PathBuf::from("/workspace"),
        );
        let project_btw = |agent: &AgentView| {
            let snapshot = project_session(agent, &mut HashMap::new()).unwrap();
            serde_json::to_value(snapshot).unwrap()["transcript"]
                .as_array()
                .unwrap()
                .iter()
                .find(|item| item["kind"] == "btw")
                .cloned()
                .expect("overlay must be projected")
        };

        agent.btw_state = Some(crate::views::btw_overlay::BtwOverlayState::Loading {
            question: "What changed?".into(),
        });
        assert!(
            !project_session(&agent, &mut HashMap::new())
                .unwrap()
                .capabilities
                .btw
        );
        let loading = project_btw(&agent);
        assert_eq!(loading["status"], "running");
        assert!(loading.get("response").is_none());
        let stable_id = loading["id"].clone();

        agent.btw_state = Some(crate::views::btw_overlay::BtwOverlayState::done(
            "What changed?".into(),
            "The bridge changed.".into(),
        ));
        let done = project_btw(&agent);
        assert_eq!(done["id"], stable_id);
        assert_eq!(done["status"], "complete");
        assert_eq!(done["response"], "The bridge changed.");

        agent.btw_state = Some(crate::views::btw_overlay::BtwOverlayState::Error {
            question: "What changed?".into(),
            error: "request failed".into(),
        });
        let failed = project_btw(&agent);
        assert_eq!(failed["id"], stable_id);
        assert_eq!(failed["status"], "failed");
        assert_eq!(failed["response"], "request failed");
    }

    #[test]
    fn repeated_identical_btw_still_projects_the_new_live_overlay() {
        let mut agent = crate::app::agent_view::test_agent_view(
            Some("btw-repeat-session"),
            std::path::PathBuf::from("/workspace"),
        );
        agent
            .scrollback
            .push_block(RenderBlock::Btw(crate::scrollback::blocks::BtwBlock::new(
                "status?",
                "old answer",
            )));
        agent.btw_state = Some(crate::views::btw_overlay::BtwOverlayState::Loading {
            question: "status?".into(),
        });

        let value =
            serde_json::to_value(project_session(&agent, &mut HashMap::new()).unwrap()).unwrap();
        let btw_items = value["transcript"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| item["kind"] == "btw")
            .collect::<Vec<_>>();
        assert_eq!(btw_items.len(), 2);
        assert_eq!(btw_items[0]["status"], "complete");
        assert_eq!(btw_items[1]["id"], "btw:overlay");
        assert_eq!(btw_items[1]["status"], "running");
    }

    #[test]
    fn cancel_subagent_choice_projects_as_an_unsupported_interaction() {
        let mut agent = crate::app::agent_view::test_agent_view(
            Some("cancel-session"),
            std::path::PathBuf::from("/workspace"),
        );
        agent.cancel_turn_view = Some(crate::views::modal::CancelTurnViewState {
            active_idx: 0,
            running_count: 2,
        });

        let value = serde_json::to_value(
            project_session(&agent, &mut HashMap::new()).expect("live session projects"),
        )
        .unwrap();
        assert_eq!(value["status"], "waiting_for_input");
        assert_eq!(value["activeInteractions"][0]["kind"], "unsupported");
        assert_eq!(value["activeInteractions"][0]["method"], "cancelSubagents");
    }
}
