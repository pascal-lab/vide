use std::{collections::HashMap, sync::Arc, time::Instant};

use lsp_server::{Connection, Message, ReqQueue, Request, RequestId, Response};
use lsp_types::{notification::Notification, request::Request as LspRequest};
use parking_lot::Mutex;
use utils::cancellation::CancellationToken;

type ResponseHandler<S> = Box<dyn FnOnce(&mut S, Response) + Send + 'static>;

struct ClientState<S> {
    sender: crossbeam_channel::Sender<Message>,
    requests: Mutex<ReqQueue<(String, Instant), ResponseHandler<S>>>,
    lifecycle_cancel: CancellationToken,
    request_cancellations: Mutex<HashMap<RequestId, CancellationToken>>,
    shutdown_requested: Mutex<bool>,
}

pub struct Client<S> {
    state: Arc<ClientState<S>>,
}

impl<S> Clone for Client<S> {
    fn clone(&self) -> Self {
        Self { state: Arc::clone(&self.state) }
    }
}

impl<S> Client<S> {
    pub fn new(sender: crossbeam_channel::Sender<Message>) -> Self {
        Self {
            state: Arc::new(ClientState {
                sender,
                requests: Mutex::new(ReqQueue::default()),
                lifecycle_cancel: CancellationToken::new(),
                request_cancellations: Mutex::new(HashMap::new()),
                shutdown_requested: Mutex::new(false),
            }),
        }
    }

    pub fn send(&self, message: Message) {
        if self.state.sender.send(message).is_err() {
            tracing::debug!("LSP message dropped because client connection is closed");
        }
    }

    pub fn notify<N>(&self, params: N::Params)
    where
        N: Notification,
    {
        self.send(lsp_server::Notification::new(N::METHOD.to_owned(), params).into());
    }

    pub fn request<R>(
        &self,
        params: R::Params,
        handler: impl FnOnce(&mut S, Response) + Send + 'static,
    ) where
        R: LspRequest,
    {
        let request = self.state.requests.lock().outgoing.register(
            R::METHOD.to_owned(),
            params,
            Box::new(handler),
        );
        self.send(request.into());
    }

    pub fn request_ignore<R>(&self, params: R::Params)
    where
        R: LspRequest,
    {
        self.request::<R>(params, |_, _| {});
    }

    pub fn complete_outgoing(&self, state: &mut S, response: Response) {
        let Some(handler) = self.state.requests.lock().outgoing.complete(response.id.clone())
        else {
            tracing::error!(?response, "received response for unknown request");
            return;
        };
        handler(state, response);
    }

    pub fn register_incoming(&self, received_at: Instant, request: &Request) {
        self.state
            .requests
            .lock()
            .incoming
            .register(request.id.clone(), (request.method.clone(), received_at));
        let cancellation = self.state.lifecycle_cancel.child_token();
        self.state.request_cancellations.lock().insert(request.id.clone(), cancellation);
    }

    pub fn request_token(&self, id: &RequestId) -> CancellationToken {
        self.state
            .request_cancellations
            .lock()
            .get(id)
            .cloned()
            .unwrap_or_else(|| self.state.lifecycle_cancel.child_token())
    }

    pub fn task_token(&self) -> CancellationToken {
        self.state.lifecycle_cancel.child_token()
    }

    pub fn is_completed(&self, request: &Request) -> bool {
        self.state.requests.lock().incoming.is_completed(&request.id)
    }

    pub fn respond(&self, response: Response) -> bool {
        let completed = self.state.requests.lock().incoming.complete(&response.id);
        let Some((method, started_at)) = completed else {
            return false;
        };
        self.state.request_cancellations.lock().remove(&response.id);
        let duration = started_at.elapsed();
        tracing::debug!(method, id = ?response.id, ?duration, "handled request");
        self.send(response.into());
        true
    }

    pub fn cancel(&self, id: RequestId) {
        if let Some(token) = self.state.request_cancellations.lock().remove(&id) {
            token.cancel();
        }
        if let Some(response) = self.state.requests.lock().incoming.cancel(id) {
            self.send(response.into());
        }
    }

    pub fn cancel_all(&self) {
        self.state.lifecycle_cancel.cancel();
        self.state.request_cancellations.lock().clear();
    }

    pub fn request_shutdown(&self) {
        *self.state.shutdown_requested.lock() = true;
        self.cancel_all();
    }

    pub fn shutdown_requested(&self) -> bool {
        *self.state.shutdown_requested.lock()
    }
}

pub fn memory_transport<S>() -> (Client<S>, crossbeam_channel::Receiver<Message>, Connection) {
    let (server, peer) = Connection::memory();
    (Client::new(server.sender), server.receiver, peer)
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use lsp_server::{Message, Request, Response};
    use lsp_types::{ProgressToken, WorkDoneProgressCreateParams, request::WorkDoneProgressCreate};

    use super::memory_transport;

    #[derive(Default)]
    struct State {
        callback_called: bool,
    }

    #[test]
    fn memory_transport_completes_outgoing_requests() {
        let (client, _inbox, peer) = memory_transport::<State>();
        let mut state = State::default();
        client.request::<WorkDoneProgressCreate>(
            WorkDoneProgressCreateParams { token: ProgressToken::String("test".to_owned()) },
            |state, response| {
                assert!(response.error.is_none());
                state.callback_called = true;
            },
        );

        let Message::Request(request) = peer.receiver.recv().unwrap() else {
            panic!("expected outgoing request");
        };
        client.complete_outgoing(&mut state, Response::new_ok(request.id, &()));

        assert!(state.callback_called);
    }

    #[test]
    fn memory_transport_cancels_incoming_requests() {
        let (client, inbox, peer) = memory_transport::<State>();
        let request = Request::new(7.into(), "test/request".to_owned(), ());
        peer.sender.send(request.clone().into()).unwrap();
        let Message::Request(received) = inbox.recv().unwrap() else {
            panic!("expected incoming request");
        };
        client.register_incoming(Instant::now(), &received);
        let cancellation = client.request_token(&received.id);

        client.cancel(received.id);

        assert!(cancellation.is_cancelled());
        let Message::Response(response) = peer.receiver.recv().unwrap() else {
            panic!("expected cancellation response");
        };
        assert!(response.error.is_some());
    }
}
