use crate::approval::{ApprovalDecision, ApprovalGate, ApprovalRequest};
use crate::event::{MobileEvent, MobileEventKind};
use crate::model::{ModelClient, ModelResponse};
use crate::remote_schema::{RemoteToolCall, RemoteToolOutput};
use crate::transport::RemoteToolTransport;
use serde_json::json;

#[derive(Debug, Default)]
pub struct AgentLoop {
    approval_gate: ApprovalGate,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TurnOutcome {
    pub assistant_text: String,
    pub executed_tool_outputs: Vec<RemoteToolOutput>,
    pub pending_approvals: Vec<ApprovalRequest>,
    pub events: Vec<MobileEvent>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolContinuationOutcome {
    pub approval_id: String,
    pub call_id: String,
    pub decision: ApprovalDecision,
    pub executed_tool_output: Option<RemoteToolOutput>,
    pub events: Vec<MobileEvent>,
}

impl AgentLoop {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn run_turn<M, T>(&self, model: &mut M, transport: &mut T, prompt: &str) -> TurnOutcome
    where
        M: ModelClient,
        T: RemoteToolTransport,
    {
        let response = model.complete(prompt);
        self.run_model_tool_turn(response, transport)
    }

    pub fn run_model_tool_turn<T>(&self, response: ModelResponse, transport: &mut T) -> TurnOutcome
    where
        T: RemoteToolTransport,
    {
        let mut outcome = TurnOutcome {
            assistant_text: response.text,
            executed_tool_outputs: Vec::new(),
            pending_approvals: Vec::new(),
            events: Vec::new(),
        };

        for call in response.tool_calls {
            if let Some(request) = self.approval_gate.evaluate(&call) {
                push_event(
                    &mut outcome.events,
                    MobileEventKind::ApprovalRequired,
                    json!({
                        "approval_id": request.approval_id,
                        "call_id": request.call_id,
                        "tool_name": request.tool_name,
                        "risk": {
                            "level": format!("{:?}", request.risk.level),
                            "reason": request.risk.reason,
                        },
                    }),
                );
                outcome.pending_approvals.push(request);
                continue;
            }

            if let Some(output) = execute_tool_call(transport, call, &mut outcome.events) {
                outcome.executed_tool_outputs.push(output);
            }
        }

        outcome
    }

    pub fn continue_approved_tool<T>(
        &self,
        transport: &mut T,
        pending: ApprovalRequest,
        decision: ApprovalDecision,
    ) -> ToolContinuationOutcome
    where
        T: RemoteToolTransport,
    {
        let approval_id = pending.approval_id;
        let call_id = pending.call_id;

        if decision == ApprovalDecision::Denied {
            return ToolContinuationOutcome {
                approval_id,
                call_id,
                decision,
                executed_tool_output: None,
                events: Vec::new(),
            };
        }

        let mut events = Vec::new();
        let executed_tool_output = execute_tool_call(transport, pending.call, &mut events);

        ToolContinuationOutcome {
            approval_id,
            call_id,
            decision,
            executed_tool_output,
            events,
        }
    }
}

fn push_event(events: &mut Vec<MobileEvent>, kind: MobileEventKind, payload: serde_json::Value) {
    events.push(MobileEvent {
        seq: events.len() as u64 + 1,
        kind,
        payload,
    });
}

fn execute_tool_call<T>(
    transport: &mut T,
    call: RemoteToolCall,
    events: &mut Vec<MobileEvent>,
) -> Option<RemoteToolOutput>
where
    T: RemoteToolTransport,
{
    push_event(
        events,
        MobileEventKind::ToolCallStarted,
        json!({
            "call_id": call.call_id,
            "tool_name": call.name.to_string(),
        }),
    );

    match transport.execute(call) {
        Ok(output) => {
            push_event(
                events,
                MobileEventKind::ToolCallCompleted,
                json!({
                    "call_id": output.call_id,
                    "success": output.success,
                }),
            );
            Some(output)
        }
        Err(error) => {
            push_event(
                events,
                MobileEventKind::Error,
                json!({ "message": error.to_string() }),
            );
            None
        }
    }
}
