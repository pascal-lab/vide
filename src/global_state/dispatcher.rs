use std::{
    fmt,
    panic::{self, UnwindSafe},
    thread,
};

use ide::Cancelled;
use lsp_server::{ExtractError, Request, Response};
use serde::{Serialize, de::DeserializeOwned};
use utils::{json::from_json, thread::ThreadIntent};

use super::{main_loop::Task, snapshot::GlobalStateSnapshot};
use crate::{global_state::GlobalState, lsp_ext::lsp_error::LspError};

pub(crate) struct ReqDispatcher<'a> {
    pub(crate) req: Option<Request>,
    pub(crate) global_state: &'a mut GlobalState,
}

type FnReqSynMut<R> = fn(
    &mut GlobalState,
    <R as lsp_types::request::Request>::Params,
) -> anyhow::Result<<R as lsp_types::request::Request>::Result>;
type FnReqSynSnap<R> = fn(
    GlobalStateSnapshot,
    <R as lsp_types::request::Request>::Params,
) -> anyhow::Result<<R as lsp_types::request::Request>::Result>;
type FnReqErrHandler = fn(Request) -> Task;

impl ReqDispatcher<'_> {
    fn parse<R>(&mut self) -> Option<(Request, R::Params, String)>
    where
        R: lsp_types::request::Request,
        R::Params: DeserializeOwned + fmt::Debug,
    {
        let req = match &self.req {
            Some(req) if req.method == R::METHOD => self.req.take()?,
            _ => return None,
        };

        match from_json(R::METHOD, &req.params) {
            Ok(params) => {
                let panic_context = format!("request: {} {params:#?}", R::METHOD);
                Some((req, params, panic_context))
            }
            Err(err) => {
                let err_code = lsp_server::ErrorCode::InvalidParams as i32;
                let response = Response::new_err(req.id, err_code, err.to_string());
                self.global_state.respond(response);
                None
            }
        }
    }

    pub(crate) fn on_sync_mut<R>(&mut self, f: FnReqSynMut<R>) -> &mut Self
    where
        R: lsp_types::request::Request,
        R::Params: DeserializeOwned + UnwindSafe + fmt::Debug,
        R::Result: Serialize,
    {
        let Some((req, params, panic_context)) = self.parse::<R>() else {
            return self;
        };

        let result = {
            let _pctx = utils::panic_context::enter(panic_context);
            f(self.global_state, params)
        };

        if let Ok(response) = result_to_response::<R>(req.id, result) {
            self.global_state.respond(response);
        }

        self
    }

    pub(crate) fn on_sync<R>(&mut self, f: FnReqSynSnap<R>) -> &mut Self
    where
        R: lsp_types::request::Request,
        R::Params: DeserializeOwned + UnwindSafe + fmt::Debug,
        R::Result: Serialize,
    {
        let Some((req, params, panic_context)) = self.parse::<R>() else {
            return self;
        };

        let global_state_snapshot = self.global_state.make_snapshot();

        let result = panic::catch_unwind(move || {
            let _pctx = utils::panic_context::enter(panic_context);
            f(global_state_snapshot, params)
        });

        if let Ok(response) = thread_result_to_response::<R>(req.id, result) {
            self.global_state.respond(response);
        }

        self
    }

    pub(crate) fn on_no_retry<R>(&mut self, f: FnReqSynSnap<R>) -> &mut Self
    where
        R: lsp_types::request::Request + 'static,
        R::Params: DeserializeOwned + UnwindSafe + Send + fmt::Debug,
        R::Result: Serialize,
    {
        self.on_with_intent_and_err_handler::<R>(ThreadIntent::Worker, f, |req| {
            Task::Response(Response::new_err(
                req.id,
                lsp_server::ErrorCode::ContentModified as i32,
                "content modified".to_string(),
            ))
        })
    }

    pub(crate) fn on_fmt_thread<R>(
        &mut self,
        f: fn(GlobalStateSnapshot, R::Params) -> anyhow::Result<R::Result>,
    ) -> &mut Self
    where
        R: lsp_types::request::Request + 'static,
        R::Params: DeserializeOwned + panic::UnwindSafe + Send + fmt::Debug,
        R::Result: Serialize,
    {
        self.on_with_intent_and_err_handler::<R>(ThreadIntent::LatencySensitive, f, |req| {
            Task::Response(Response::new_err(
                req.id,
                lsp_server::ErrorCode::InternalError as i32,
                "internal error".to_string(),
            ))
        })
    }

    fn on_with_intent_and_err_handler<R>(
        &mut self,
        intent: ThreadIntent,
        f: FnReqSynSnap<R>,
        err_handler: FnReqErrHandler,
    ) -> &mut Self
    where
        R: lsp_types::request::Request + 'static,
        R::Params: DeserializeOwned + UnwindSafe + Send + fmt::Debug,
        R::Result: Serialize,
    {
        let Some((req, params, panic_context)) = self.parse::<R>() else {
            return self;
        };

        let world = self.global_state.make_snapshot();

        self.global_state.task_pool.handle.spawn_and_send(intent, move || {
            let result = panic::catch_unwind(move || {
                let _pctx = utils::panic_context::enter(panic_context);
                f(world, params)
            });
            match thread_result_to_response::<R>(req.id.clone(), result) {
                Ok(response) => Task::Response(response),
                Err(_) => err_handler(req),
            }
        });

        self
    }

    pub(crate) fn on<R>(&mut self, f: FnReqSynSnap<R>) -> &mut Self
    where
        R: lsp_types::request::Request + 'static,
        R::Params: DeserializeOwned + UnwindSafe + Send + fmt::Debug,
        R::Result: Serialize,
    {
        self.on_with_intent_and_err_handler::<R>(ThreadIntent::Worker, f, Task::Retry)
    }

    pub(crate) fn on_latency_sensitive<R>(&mut self, f: FnReqSynSnap<R>) -> &mut Self
    where
        R: lsp_types::request::Request + 'static,
        R::Params: DeserializeOwned + UnwindSafe + Send + fmt::Debug,
        R::Result: Serialize,
    {
        self.on_with_intent_and_err_handler::<R>(ThreadIntent::LatencySensitive, f, |req| {
            Task::Retry(req)
        })
    }

    pub(crate) fn finish(&mut self) {
        if let Some(req) = self.req.take() {
            tracing::error!("unknown request: {:?}", req);
            let response = Response::new_err(
                req.id,
                lsp_server::ErrorCode::MethodNotFound as i32,
                "unknown request".to_string(),
            );
            self.global_state.respond(response);
        }
    }
}

// Analysis error code
fn result_to_response<R>(
    id: lsp_server::RequestId,
    result: anyhow::Result<R::Result>,
) -> Result<Response, Cancelled>
where
    R: lsp_types::request::Request,
    R::Params: DeserializeOwned,
    R::Result: Serialize,
{
    if let Ok(res) = result {
        return Ok(Response::new_ok(id, &res));
    }

    let e = result.err().unwrap().downcast::<LspError>();
    if let Ok(lsp_error) = e {
        return Ok(Response::new_err(id, lsp_error.code, lsp_error.message));
    }

    match e.err().unwrap().downcast::<Cancelled>() {
        Ok(cancelled) => Err(cancelled),
        Err(e) => {
            Ok(Response::new_err(id, lsp_server::ErrorCode::InternalError as i32, e.to_string()))
        }
    }
}

// Analysis error code for threads
fn thread_result_to_response<R>(
    id: lsp_server::RequestId,
    result: thread::Result<anyhow::Result<R::Result>>,
) -> Result<Response, Cancelled>
where
    R: lsp_types::request::Request,
    R::Params: DeserializeOwned,
    R::Result: Serialize,
{
    match result {
        Ok(result) => result_to_response::<R>(id, result),
        Err(panic) => {
            let panic_message = panic
                .downcast_ref::<String>()
                .map(String::as_str)
                .or_else(|| panic.downcast_ref::<&str>().copied());

            let mut message = "request handler panicked".to_string();
            if let Some(panic_message) = panic_message {
                message.push_str(": ");
                message.push_str(panic_message)
            };

            Ok(Response::new_err(id, lsp_server::ErrorCode::InternalError as i32, message))
        }
    }
}

pub(crate) struct NotifDispatcher<'a> {
    pub(crate) notif: Option<lsp_server::Notification>,
    pub(crate) global_state: &'a mut GlobalState,
}

type FnNotifSynMut<N> = fn(
    &mut GlobalState,
    <N as lsp_types::notification::Notification>::Params,
) -> anyhow::Result<()>;

impl NotifDispatcher<'_> {
    pub(crate) fn on_sync_mut<N>(&mut self, f: FnNotifSynMut<N>) -> &mut Self
    where
        N: lsp_types::notification::Notification,
        N::Params: DeserializeOwned + Send,
    {
        let notif = match &self.notif {
            Some(notif) if notif.method == N::METHOD => self.notif.take().unwrap(),
            _ => return self,
        };

        let params = match notif.extract::<N::Params>(N::METHOD) {
            Ok(it) => it,
            Err(ExtractError::JsonError { method, error }) => {
                tracing::error!("invalid notification params for {method}: {error}");
                return self;
            }
            Err(ExtractError::MethodMismatch(notif)) => {
                self.notif = Some(notif);
                return self;
            }
        };

        let _pctx = utils::panic_context::enter(format!("\nnotification: {}", N::METHOD));
        if let Err(error) = f(self.global_state, params) {
            tracing::error!("notification handler failed for {}: {error:#}", N::METHOD);
        }

        self
    }

    pub(crate) fn finish(&self) {
        if self.notif.is_some() {
            tracing::error!("Unhandled notification: {:?}", &self.notif);
        }
    }
}
