use std::{
    collections::HashMap,
    panic::{self, AssertUnwindSafe, UnwindSafe},
    sync::Arc,
};

use lsp_server::{ErrorCode, Request, Response, ResponseError};
use lsp_types::{notification::Notification, request::Request as LspRequest};
use parking_lot::Mutex;
use serde::{Serialize, de::DeserializeOwned};
use utils::{cancellation::CancellationToken, thread::ThreadIntent};

use crate::Client;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct RequestPolicy {
    intent: ThreadIntent,
    retry_on_content_change: bool,
}

impl RequestPolicy {
    pub const LATENCY_SENSITIVE: Self =
        Self { intent: ThreadIntent::LatencySensitive, retry_on_content_change: true };
    pub const LATENCY_SENSITIVE_NO_RETRY: Self =
        Self { intent: ThreadIntent::LatencySensitive, retry_on_content_change: false };
    pub const WORKER: Self = Self { intent: ThreadIntent::Worker, retry_on_content_change: true };
    pub const WORKER_NO_RETRY: Self =
        Self { intent: ThreadIntent::Worker, retry_on_content_change: false };
}

#[derive(Debug)]
pub enum HandlerFailure {
    Response(ResponseError),
    Cancelled,
}

pub type AcceptedEffects<E> = Arc<Mutex<Vec<E>>>;

#[derive(Debug)]
pub enum ProtocolTask<E> {
    Response { response: Response, accepted_effects: Vec<E> },
    Retry(Request),
}

pub trait RuntimeState: Sized + 'static {
    type Snapshot: Send + UnwindSafe + 'static;
    type Effect: Send + 'static;

    fn client(&self) -> &Client<Self>;
    fn snapshot(&self, cancellation: CancellationToken) -> Self::Snapshot;
    fn accepted_effects(snapshot: &Self::Snapshot) -> AcceptedEffects<Self::Effect>;
    fn spawn_protocol_task(
        &mut self,
        intent: ThreadIntent,
        task: Box<dyn FnOnce() -> ProtocolTask<Self::Effect> + Send>,
    );
    fn map_handler_error(error: anyhow::Error, cancellation: &CancellationToken) -> HandlerFailure;
}

type RequestHandler<S> = Box<dyn Fn(&mut S, Request) + Send + Sync>;
type NotificationHandler<S> = Box<dyn Fn(&mut S, lsp_server::Notification) + Send + Sync>;

pub struct Router<S> {
    requests: HashMap<&'static str, RequestHandler<S>>,
    notifications: HashMap<&'static str, NotificationHandler<S>>,
}

impl<S> Default for Router<S>
where
    S: RuntimeState,
{
    fn default() -> Self {
        Self::new()
    }
}

impl<S> Router<S>
where
    S: RuntimeState,
{
    pub fn new() -> Self {
        Self { requests: HashMap::new(), notifications: HashMap::new() }
    }

    pub fn request_mut<R>(
        &mut self,
        handler: fn(&mut S, R::Params) -> anyhow::Result<R::Result>,
    ) -> &mut Self
    where
        R: LspRequest + 'static,
        R::Params: DeserializeOwned + UnwindSafe,
        R::Result: Serialize,
    {
        let previous = self.requests.insert(
            R::METHOD,
            Box::new(move |state, request| {
                let id = request.id.clone();
                let cancellation = state.client().request_token(&id);
                let params = match decode_request::<S, R>(state.client(), request.clone()) {
                    Some(params) => params,
                    None => return,
                };
                let result = panic::catch_unwind(AssertUnwindSafe(|| {
                    cancellation.check()?;
                    handler(state, params)
                }));
                let response = encode_thread_result::<S, R>(request, result, &cancellation, false);
                match response {
                    ProtocolTask::Response { response, .. } => {
                        state.client().respond(response);
                    }
                    ProtocolTask::Retry(_) => unreachable!("mutable requests are never retried"),
                }
            }),
        );
        assert!(previous.is_none(), "duplicate request handler for {}", R::METHOD);
        self
    }

    pub fn request<R>(
        &mut self,
        policy: RequestPolicy,
        handler: fn(S::Snapshot, R::Params) -> anyhow::Result<R::Result>,
    ) -> &mut Self
    where
        R: LspRequest + 'static,
        R::Params: DeserializeOwned + Send + UnwindSafe,
        R::Result: Serialize,
    {
        let previous = self.requests.insert(
            R::METHOD,
            Box::new(move |state, request| {
                let id = request.id.clone();
                let cancellation = state.client().request_token(&id);
                let params = match decode_request::<S, R>(state.client(), request.clone()) {
                    Some(params) => params,
                    None => return,
                };
                let snapshot = state.snapshot(cancellation.clone());
                let effects = S::accepted_effects(&snapshot);
                state.spawn_protocol_task(
                    policy.intent,
                    Box::new(move || {
                        let worker_cancellation = cancellation.clone();
                        let result = panic::catch_unwind(move || {
                            worker_cancellation.check()?;
                            let result = handler(snapshot, params)?;
                            worker_cancellation.check()?;
                            Ok(result)
                        });
                        let mut task = encode_thread_result::<S, R>(
                            request,
                            result,
                            &cancellation,
                            policy.retry_on_content_change,
                        );
                        if let ProtocolTask::Response { response, accepted_effects } = &mut task
                            && response.error.is_none()
                        {
                            *accepted_effects = std::mem::take(&mut *effects.lock());
                        }
                        task
                    }),
                );
            }),
        );
        assert!(previous.is_none(), "duplicate request handler for {}", R::METHOD);
        self
    }

    pub fn notification<N>(
        &mut self,
        handler: fn(&mut S, N::Params) -> anyhow::Result<()>,
    ) -> &mut Self
    where
        N: Notification + 'static,
        N::Params: DeserializeOwned + Send + UnwindSafe,
    {
        let previous = self.notifications.insert(
            N::METHOD,
            Box::new(move |state, notification| {
                let params = match notification.extract::<N::Params>(N::METHOD) {
                    Ok(params) => params,
                    Err(error) => {
                        tracing::error!(method = N::METHOD, ?error, "invalid notification");
                        return;
                    }
                };
                let result = panic::catch_unwind(AssertUnwindSafe(|| handler(state, params)));
                match result {
                    Ok(Ok(())) => {}
                    Ok(Err(error)) => {
                        tracing::error!(method = N::METHOD, %error, "notification handler failed");
                    }
                    Err(payload) => {
                        tracing::error!(
                            method = N::METHOD,
                            message = panic_message(&*payload),
                            "notification handler panicked"
                        );
                    }
                }
            }),
        );
        assert!(previous.is_none(), "duplicate notification handler for {}", N::METHOD);
        self
    }

    pub fn dispatch_request(&self, state: &mut S, request: Request) {
        if state.client().shutdown_requested()
            && request.method != lsp_types::request::Shutdown::METHOD
        {
            state.client().respond(Response::new_err(
                request.id,
                ErrorCode::InvalidRequest as i32,
                "shutdown already requested".to_owned(),
            ));
            return;
        }
        let Some(handler) = self.requests.get(request.method.as_str()) else {
            tracing::error!(method = %request.method, "unknown request");
            state.client().respond(Response::new_err(
                request.id,
                ErrorCode::MethodNotFound as i32,
                "unknown request".to_owned(),
            ));
            return;
        };
        handler(state, request);
    }

    pub fn dispatch_notification(&self, state: &mut S, notification: lsp_server::Notification) {
        let Some(handler) = self.notifications.get(notification.method.as_str()) else {
            if !notification.method.starts_with("$/") {
                tracing::error!(method = %notification.method, "unhandled notification");
            }
            return;
        };
        handler(state, notification);
    }
}

fn decode_request<S, R>(client: &Client<S>, request: Request) -> Option<R::Params>
where
    R: LspRequest,
    R::Params: DeserializeOwned,
{
    match serde_json::from_value(request.params) {
        Ok(params) => Some(params),
        Err(error) => {
            tracing::warn!(method = R::METHOD, id = ?request.id, %error, "invalid request params");
            client.respond(Response::new_err(
                request.id,
                ErrorCode::InvalidParams as i32,
                error.to_string(),
            ));
            None
        }
    }
}

fn encode_thread_result<S, R>(
    request: Request,
    result: std::thread::Result<anyhow::Result<R::Result>>,
    cancellation: &CancellationToken,
    retry_on_content_change: bool,
) -> ProtocolTask<S::Effect>
where
    S: RuntimeState,
    R: LspRequest,
    R::Result: Serialize,
{
    let id = request.id.clone();
    let response = match result {
        Ok(Ok(result)) => Response::new_ok(id.clone(), &result),
        Ok(Err(error)) => match S::map_handler_error(error, cancellation) {
            HandlerFailure::Response(error) => {
                Response { id: id.clone(), result: None, error: Some(error) }
            }
            HandlerFailure::Cancelled
                if retry_on_content_change && !cancellation.is_cancelled() =>
            {
                return ProtocolTask::Retry(request);
            }
            HandlerFailure::Cancelled => cancelled_response(id.clone(), cancellation),
        },
        Err(payload) => Response::new_err(
            id.clone(),
            ErrorCode::InternalError as i32,
            format!("request handler panicked: {}", panic_message(&*payload)),
        ),
    };
    ProtocolTask::Response { response, accepted_effects: Vec::new() }
}

fn cancelled_response(id: lsp_server::RequestId, cancellation: &CancellationToken) -> Response {
    let (code, message) = if cancellation.is_cancelled() {
        (lsp_types::error_codes::REQUEST_CANCELLED as i32, "request cancelled")
    } else {
        (ErrorCode::ContentModified as i32, "content modified")
    };
    Response::new_err(id, code, message.to_owned())
}

fn panic_message(payload: &(dyn std::any::Any + Send)) -> &str {
    payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| payload.downcast_ref::<&str>().copied())
        .unwrap_or("unknown panic payload")
}
