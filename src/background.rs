use std::collections::HashMap;

use structopt::StructOpt;
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

use tokio_stream::{wrappers::ReceiverStream, StreamExt};

use crate::{
    errors::Error,
    events::Event,
    params::RunParameters,
    requests::{Request, RequestType},
    responses::{Response, ResponseType},
    websocket::WebsocketClient,
};

#[derive(Debug)]
pub enum Command {
    Publish {
        topic: String,
        payload: Vec<u8>,
        sender: oneshot::Sender<Result<(), Error>>,
    },
    Subscribe {
        topic: String,
        stream: mpsc::Sender<Result<String, Error>>,
    },

    Signal {
        state: String,
        sender: oneshot::Sender<Result<u64, Error>>,
    },

    Barrier {
        state: String,
        target: u64,
        sender: oneshot::Sender<Result<(), Error>>,
    },

    WaitNetworkInitializedStart {
        sender: oneshot::Sender<Result<(), Error>>,
    },

    WaitNetworkInitializedBarrier {
        sender: oneshot::Sender<Result<(), Error>>,
    },

    WaitNetworkInitializedEnd {
        sender: oneshot::Sender<Result<(), Error>>,
    },
}

pub struct BackgroundTask {
    websocket_tx: mpsc::Sender<(Request, oneshot::Sender<Result<(), Error>>)>,
    handle: JoinHandle<()>,
    websocket_rx: ReceiverStream<Result<Response, Error>>,

    next_id: u64,

    params: RunParameters,

    client_rx: ReceiverStream<Command>,

    pending_general: HashMap<u64, oneshot::Sender<Result<(), Error>>>,
    pending_sub: HashMap<u64, mpsc::Sender<Result<String, Error>>>,
    pending_signal: HashMap<u64, oneshot::Sender<Result<u64, Error>>>,
}

impl Drop for BackgroundTask {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

impl BackgroundTask {
    pub async fn new(
        client_rx: mpsc::Receiver<Command>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let client_rx = ReceiverStream::new(client_rx);

        let params = RunParameters::from_args();

        let (websocket_tx, req_rx) = mpsc::channel(10);
        let (res_tx, websocket_rx) = mpsc::channel(10);

        let web = WebsocketClient::new(res_tx, req_rx).await?;

        let handle = tokio::spawn(async move {
            web.run().await;
        });

        let websocket_rx = ReceiverStream::new(websocket_rx);

        Ok(Self {
            websocket_rx,
            next_id: 0,
            params,
            client_rx,
            websocket_tx,
            pending_general: Default::default(),
            pending_sub: Default::default(),
            pending_signal: Default::default(),
            handle,
        })
    }

    fn contextualize_state(&self, state: &str) -> String {
        format!(
            "run:{}:plan:{}:case:{}:states:{}",
            self.params.test_run, self.params.test_plan, self.params.test_case, state
        )
    }

    fn contextualize_topic(&self, topic: &str) -> String {
        format!(
            "run:{}:plan:{}:case:{}:topics:{}",
            self.params.test_run, self.params.test_plan, self.params.test_case, topic
        )
    }

    fn contextualize_event(&self) -> String {
        format!(
            "run:{}:plan:{}:case:{}:run_events",
            self.params.test_run, self.params.test_plan, self.params.test_case
        )
    }

    fn next_id(&mut self) -> u64 {
        let next_id = self.next_id;
        self.next_id += 1;
        next_id
    }

    pub async fn run(&mut self) {
        loop {
            tokio::select! {
                res = self.websocket_rx.next() => match res {
                    Some(res) => self.response(res).await,
                    None => {
                        eprintln!("Web socket receiver dropped");
                        return;
                    },
                },
                cmd = self.client_rx.next() => match cmd {
                    Some(cmd) => self.command(cmd).await,
                    None => {
                        eprintln!("Client command receiver dropped");
                        return;
                    },
                },
            }
        }
    }

    async fn command(&mut self, cmd: Command) {
        let id = self.next_id();

        match cmd {
            Command::Publish {
                topic,
                payload,
                sender,
            } => {
                let topic = self.contextualize_topic(&topic);

                let request = Request {
                    id: id.to_string(),
                    is_cancel: false,
                    request: RequestType::Publish { topic, payload },
                };

                let (tx, websocket_rx) = oneshot::channel();

                let msg = (request, tx);

                self.websocket_tx
                    .send(msg)
                    .await
                    .expect("receiver not dropped");

                if let Err(e) = websocket_rx.await.expect("sender not dropped") {
                    sender.send(Err(e)).expect("receiver not dropped");
                } else {
                    self.pending_general.insert(id, sender);
                }
            }
            Command::Subscribe { topic, stream } => {
                let topic = self.contextualize_topic(&topic);

                let request = Request {
                    id: id.to_string(),
                    is_cancel: false,
                    request: RequestType::Subscribe { topic },
                };

                let (tx, websocket_rx) = oneshot::channel();

                let msg = (request, tx);

                self.websocket_tx
                    .send(msg)
                    .await
                    .expect("receiver not dropped");

                if let Err(e) = websocket_rx.await.expect("sender not dropped") {
                    stream.send(Err(e)).await.expect("receiver not dropped");
                } else {
                    self.pending_sub.insert(id, stream);
                }
            }
            Command::Signal { state, sender } => {
                let state = self.contextualize_state(&state);

                let request = Request {
                    id: id.to_string(),
                    is_cancel: false,
                    request: RequestType::SignalEntry { state },
                };

                let (tx, websocket_rx) = oneshot::channel();

                let msg = (request, tx);

                self.websocket_tx
                    .send(msg)
                    .await
                    .expect("receiver not dropped");

                if let Err(e) = websocket_rx.await.expect("sender not dropped") {
                    sender.send(Err(e)).expect("receiver not dropped");
                } else {
                    self.pending_signal.insert(id, sender);
                }
            }
            Command::Barrier {
                state,
                target,
                sender,
            } => {
                let state = self.contextualize_state(&state);

                let request = Request {
                    id: id.to_string(),
                    is_cancel: false,
                    request: RequestType::Barrier { state, target },
                };

                let (tx, websocket_rx) = oneshot::channel();

                let msg = (request, tx);

                self.websocket_tx
                    .send(msg)
                    .await
                    .expect("receiver not dropped");

                if let Err(e) = websocket_rx.await.expect("sender not dropped") {
                    sender.send(Err(e)).expect("receiver not dropped");
                } else {
                    self.pending_general.insert(id, sender);
                }
            }
            Command::WaitNetworkInitializedStart { sender } => {
                let event = Event::StageStart {
                    name: "network-initialized".to_owned(),
                    group: self.params.test_group_id.clone(),
                };

                let topic = self.contextualize_event();
                let payload = serde_json::to_vec(&event).expect("Serializable Event");

                let request = Request {
                    id: id.to_string(),
                    is_cancel: false,
                    request: RequestType::Publish { topic, payload },
                };

                let (tx, websocket_rx) = oneshot::channel();

                let msg = (request, tx);

                self.websocket_tx
                    .send(msg)
                    .await
                    .expect("receiver not dropped");

                if let Err(e) = websocket_rx.await.expect("sender not dropped") {
                    sender.send(Err(e)).expect("receiver not dropped");
                } else {
                    self.pending_general.insert(id, sender);
                }
            }
            Command::WaitNetworkInitializedBarrier { sender } => {
                if !self.params.test_sidecar {
                    sender.send(Ok(())).expect("receiver not dropped");
                    return;
                }

                let state = self.contextualize_state("network-initialized");

                let request = Request {
                    id: id.to_string(),
                    is_cancel: false,
                    request: RequestType::Barrier {
                        state,
                        target: self.params.test_instance_count,
                    },
                };

                let (tx, websocket_rx) = oneshot::channel();

                let msg = (request, tx);

                self.websocket_tx
                    .send(msg)
                    .await
                    .expect("receiver not dropped");

                if let Err(e) = websocket_rx.await.expect("sender not dropped") {
                    sender.send(Err(e)).expect("receiver not dropped");
                } else {
                    self.pending_general.insert(id, sender);
                }
            }
            Command::WaitNetworkInitializedEnd { sender } => {
                let event = Event::StageEnd {
                    name: "network-initialized".to_owned(),
                    group: self.params.test_group_id.clone(),
                };

                let topic = self.contextualize_event();
                let payload = serde_json::to_vec(&event).expect("Serializable Event");

                let request = Request {
                    id: id.to_string(),
                    is_cancel: false,
                    request: RequestType::Publish { topic, payload },
                };

                let (tx, websocket_rx) = oneshot::channel();

                let msg = (request, tx);

                self.websocket_tx
                    .send(msg)
                    .await
                    .expect("receiver not dropped");

                if let Err(e) = websocket_rx.await.expect("sender not dropped") {
                    sender.send(Err(e)).expect("receiver not dropped");
                } else {
                    self.pending_general.insert(id, sender);
                }
            }
        }
    }

    async fn response(&mut self, res: Result<Response, Error>) {
        let res = match res {
            Ok(res) => res,
            Err(e) => {
                eprintln!("{:?}", e);
                return;
            }
        };

        let Response {
            id,
            error,
            response,
        } = res;

        let idx = id.parse().unwrap();

        if let Some(response) = response {
            match response {
                ResponseType::SignalEntry { seq } => {
                    let sender = match self.pending_signal.remove(&idx) {
                        Some(sender) => sender,
                        None => {
                            eprintln!("Signal response {} not found", idx);
                            return;
                        }
                    };

                    sender.send(Ok(seq)).expect("receiver not dropped");
                }
                ResponseType::Publish { .. } => {
                    let sender = match self.pending_general.remove(&idx) {
                        Some(sender) => sender,
                        None => {
                            eprintln!("Publish response {} not found", idx);
                            return;
                        }
                    };

                    sender.send(Ok(())).expect("receiver not dropped");
                }
                ResponseType::Subscribe(msg) => {
                    let stream = match self.pending_sub.remove(&idx) {
                        Some(sender) => sender,
                        None => return,
                    };

                    if stream.is_closed() {
                        return;
                    }

                    stream.send(Ok(msg)).await.expect("stream not dropped");

                    self.pending_sub.insert(idx, stream);
                }
            }
        }

        if let Some(error) = error {
            if let Some(sender) = self.pending_general.remove(&idx) {
                sender
                    .send(Err(Error::SyncService(error)))
                    .expect("receiver not dropped");
                return;
            }

            if let Some(sender) = self.pending_signal.remove(&idx) {
                sender
                    .send(Err(Error::SyncService(error)))
                    .expect("receiver not dropped");
                return;
            }

            if let Some(sender) = self.pending_sub.remove(&idx) {
                sender
                    .send(Err(Error::SyncService(error)))
                    .await
                    .expect("receiver not dropped");
                return;
            }
        }
    }
}
