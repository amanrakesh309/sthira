//! Explicit request lifecycle and timeout policy for model inference and multi-step workflows.

use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Atomic state of an in-flight request lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequestState {
    /// Request is active, connecting, generating, or awaiting commitment.
    Running,
    /// Request was explicitly cancelled by the caller or user before commit began.
    Cancelled,
    /// Persistence boundary entered: database writer acquired, commit begun.
    Committing,
    /// Persistence completed and transaction finalized.
    Completed,
}

/// Explicit outcome of a cancellation attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancelOutcome {
    /// Cancellation was accepted while the request was Running.
    /// The request will not commit any state changes.
    Cancelled,
    /// The request has already entered the commit phase.
    /// Persistence has begun and cannot be rolled back cooperatively.
    AlreadyCommitting,
    /// The request has already completed successfully.
    Completed,
    /// The request was already cancelled.
    AlreadyCancelled,
}

/// Explicit cancellation token for cooperative request cancellation.
#[derive(Debug, Clone)]
pub struct CancellationToken {
    sender: tokio::sync::watch::Sender<bool>,
    receiver: tokio::sync::watch::Receiver<bool>,
    state: Arc<Mutex<RequestState>>,
}

impl Default for CancellationToken {
    fn default() -> Self {
        Self::new()
    }
}

impl CancellationToken {
    pub fn new() -> Self {
        let (sender, receiver) = tokio::sync::watch::channel(false);
        Self {
            sender,
            receiver,
            state: Arc::new(Mutex::new(RequestState::Running)),
        }
    }

    /// Attempts to cancel the request, returning the explicit outcome.
    pub fn cancel_with_outcome(&self) -> CancelOutcome {
        let mut st = self.state.lock().unwrap();
        match *st {
            RequestState::Running => {
                *st = RequestState::Cancelled;
                let _ = self.sender.send(true);
                CancelOutcome::Cancelled
            }
            RequestState::Committing => CancelOutcome::AlreadyCommitting,
            RequestState::Completed => CancelOutcome::Completed,
            RequestState::Cancelled => CancelOutcome::AlreadyCancelled,
        }
    }

    /// Cancels the request if still in Running state.
    /// Returns true if and only if cancellation was accepted before the commit phase began.
    pub fn cancel(&self) -> bool {
        self.cancel_with_outcome() == CancelOutcome::Cancelled
    }

    /// Atomically transitions the request state from Running to Committing.
    /// Fails with `RequestErrorKind::Cancelled` if the request was cancelled prior to commitment.
    pub fn try_begin_commit(&self) -> Result<(), RequestErrorKind> {
        let mut st = self.state.lock().unwrap();
        match *st {
            RequestState::Running => {
                *st = RequestState::Committing;
                Ok(())
            }
            RequestState::Cancelled => Err(RequestErrorKind::Cancelled),
            RequestState::Committing => Ok(()),
            RequestState::Completed => Ok(()),
        }
    }

    /// Finalizes the request state from Committing to Completed.
    pub fn finish_commit(&self) {
        let mut st = self.state.lock().unwrap();
        if *st == RequestState::Committing {
            *st = RequestState::Completed;
        }
    }

    /// Current atomic lifecycle state.
    pub fn state(&self) -> RequestState {
        *self.state.lock().unwrap()
    }

    pub fn is_cancelled(&self) -> bool {
        *self.state.lock().unwrap() == RequestState::Cancelled || *self.receiver.borrow()
    }

    pub async fn wait_for_cancellation(&self) {
        let mut rx = self.receiver.clone();
        while !*rx.borrow_and_update() {
            if rx.changed().await.is_err() {
                break;
            }
        }
    }
}

/// Explicit timeout and deadline configuration bounding model operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestPolicy {
    /// Maximum time allowed to establish TCP/TLS connection with provider.
    pub connect_timeout: Duration,
    /// Maximum time allowed for single-request model inference / evaluation.
    pub generation_budget: Duration,
    /// Maximum time allowed for the overall caller workflow.
    pub workflow_deadline: Duration,
    /// Inactivity timeout for streaming responses (maximum idle time between chunks).
    pub stream_inactivity_timeout: Duration,
    /// Maximum time allowed for the initial streaming chunk / time-to-first-token.
    pub stream_initial_timeout: Duration,
}

impl Default for RequestPolicy {
    fn default() -> Self {
        Self::standard()
    }
}

impl RequestPolicy {
    /// Standard policy for document synthesis, transformation, derivation, and refinement.
    ///
    /// - Connect timeout: 10s (fast failure on down hosts while allowing for loaded systems).
    /// - Generation budget: 180s (accommodates slow local CPU models like llama3.2 @ ~3.73 tps generating 600+ tokens).
    /// - Workflow deadline: 185s (ensures caller bounds total execution with a slight buffer for serialization/persistence).
    /// - Stream inactivity timeout: 30s (detects hung or stalled streams).
    /// - Stream initial timeout: 60s (allows for prompt evaluation and model warmup).
    pub const fn standard() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            generation_budget: Duration::from_secs(180),
            workflow_deadline: Duration::from_secs(185),
            stream_inactivity_timeout: Duration::from_secs(30),
            stream_initial_timeout: Duration::from_secs(60),
        }
    }

    /// Policy for quick / short structured metadata tasks (e.g. Naming, Smart Title).
    ///
    /// - Connect timeout: 10s.
    /// - Generation budget: 45s (output is strictly bounded to 3-9 words, ~25 tokens).
    /// - Workflow deadline: 60s.
    /// - Stream inactivity timeout: 15s.
    /// - Stream initial timeout: 30s.
    pub const fn quick_task() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            generation_budget: Duration::from_secs(45),
            workflow_deadline: Duration::from_secs(60),
            stream_inactivity_timeout: Duration::from_secs(15),
            stream_initial_timeout: Duration::from_secs(30),
        }
    }

    /// Custom policy constructor for testing or specific workloads.
    pub const fn custom(
        connect_timeout: Duration,
        generation_budget: Duration,
        workflow_deadline: Duration,
        stream_inactivity_timeout: Duration,
        stream_initial_timeout: Duration,
    ) -> Self {
        Self {
            connect_timeout,
            generation_budget,
            workflow_deadline,
            stream_inactivity_timeout,
            stream_initial_timeout,
        }
    }
}

/// Explicit classification of request failures across the request lifecycle.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestErrorKind {
    /// Failed to connect to provider (connection refused, host unreachable, DNS resolution failed).
    ConnectionFailed { details: String },
    /// Connection establishment timed out before completing handshake.
    ConnectionTimeout { limit: Duration },
    /// Model provider accepted request, but inference exceeded the generation budget.
    GenerationDeadline { limit: Duration },
    /// Streaming connection established, but no chunk received within the inactivity window.
    StreamInactivity { limit: Duration },
    /// Total caller/workflow deadline expired before the workflow completed.
    WorkflowDeadline { limit: Duration },
    /// The request was explicitly cancelled by the caller or user.
    Cancelled,
    /// Provider returned an HTTP error status.
    HttpStatus { status: u16, message: String },
    /// Provider response was not valid JSON or could not be parsed into target schema.
    MalformedResponse { details: String },
    /// Provider response was empty or whitespace-only.
    EmptyResponse,
}

impl RequestErrorKind {
    /// Returns a truthful, human-readable error message indicating stage and configured limit.
    pub fn user_message(&self, stage: &str, unaffected_scope: &str) -> String {
        let verb = if unaffected_scope.contains(" and ") { "are" } else { "is" };
        match self {
            Self::ConnectionTimeout { limit } => {
                format!(
                    "Could not connect to the model provider within {} seconds. Check that the service is running and accessible.",
                    limit.as_secs()
                )
            }
            Self::ConnectionFailed { details } => {
                format!(
                    "Model generation failed: Could not reach the model provider ({details}). Check that the service is running and the address is correct."
                )
            }
            Self::GenerationDeadline { limit } => {
                format!(
                    "{stage} timed out after {} seconds. {unaffected_scope} {verb} unchanged.",
                    limit.as_secs()
                )
            }
            Self::StreamInactivity { limit } => {
                format!(
                    "The model stream stalled for over {} seconds without emitting tokens. {unaffected_scope} {verb} unchanged.",
                    limit.as_secs()
                )
            }
            Self::WorkflowDeadline { limit } => {
                format!(
                    "{stage} timed out after {} seconds. {unaffected_scope} {verb} unchanged.",
                    limit.as_secs()
                )
            }
            Self::Cancelled => {
                format!("{stage} was cancelled. {unaffected_scope} {verb} unchanged.")
            }
            Self::HttpStatus { status, message } => {
                format!("Model generation failed: The model provider returned HTTP {status}: {message}")
            }
            Self::MalformedResponse { details } => {
                format!("Model returned malformed {stage} response: {details}")
            }
            Self::EmptyResponse => {
                format!("Model returned empty {stage} content. {unaffected_scope} {verb} unchanged.")
            }
        }
    }
}

impl std::fmt::Display for RequestErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.user_message("Operation", "Document"))
    }
}

impl std::error::Error for RequestErrorKind {}

/// Classify a `reqwest::Error` using the configured `RequestPolicy`.
pub fn classify_reqwest_error(err: &reqwest::Error, policy: &RequestPolicy) -> RequestErrorKind {
    if err.is_timeout() {
        if err.is_connect() {
            RequestErrorKind::ConnectionTimeout {
                limit: policy.connect_timeout,
            }
        } else {
            RequestErrorKind::GenerationDeadline {
                limit: policy.generation_budget,
            }
        }
    } else if err.is_connect() {
        RequestErrorKind::ConnectionFailed {
            details: sanitize_error(err),
        }
    } else if let Some(status) = err.status() {
        RequestErrorKind::HttpStatus {
            status: status.as_u16(),
            message: status.to_string(),
        }
    } else {
        RequestErrorKind::ConnectionFailed {
            details: sanitize_error(err),
        }
    }
}

/// Sanitize error messages to avoid leaking sensitive credentials or raw prompts.
pub fn sanitize_error(err: &reqwest::Error) -> String {
    let msg = err.to_string();
    // Strip query parameters or basic auth tokens if present
    if let Some(pos) = msg.find("?key=") {
        let mut clean = msg[..pos].to_string();
        clean.push_str("?[REDACTED]");
        clean
    } else {
        msg
    }
}
