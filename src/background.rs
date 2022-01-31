use std::collections::HashMap;

use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

use tokio_stream::{wrappers::ReceiverStream, StreamExt};

use tokio_util::either::Either;

use crate::{
    errors::{ReceiveError, SendError},
    requests::{Request, RequestType},
    responses::{Response, ResponseType},
    websocket::WebsocketClient,
};

#[derive(Debug)]
pub enum Command {
    Publish {
        topic: String,
        payload: Vec<u8>,
        sender: oneshot::Sender<Result<(), Either<SendError, ReceiveError>>>,
    },
    Subscribe {
        topic: String,
        stream: mpsc::Sender<Result<String, Either<SendError, ReceiveError>>>,
    },

    Signal {
        state: String,
        sender: oneshot::Sender<Result<u64, Either<SendError, ReceiveError>>>,
    },

    Barrier {
        state: String,
        target: u64,
        sender: oneshot::Sender<Result<(), Either<SendError, ReceiveError>>>,
    },
}

pub struct BackgroundTask {
    req_tx: mpsc::Sender<(Request, oneshot::Sender<Result<(), SendError>>)>,
    handle: JoinHandle<()>,
    res_rx: ReceiverStream<Result<Response, ReceiveError>>,

    next_id: u64,

    context: String,

    cmd_rx: ReceiverStream<Command>,

    pending_pub: HashMap<u64, oneshot::Sender<Result<(), Either<SendError, ReceiveError>>>>,
    pending_sub: HashMap<u64, mpsc::Sender<Result<String, Either<SendError, ReceiveError>>>>,
    pending_signal: HashMap<u64, oneshot::Sender<Result<u64, Either<SendError, ReceiveError>>>>,
    pending_barrier: HashMap<u64, oneshot::Sender<Result<(), Either<SendError, ReceiveError>>>>,
}

impl Drop for BackgroundTask {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

impl BackgroundTask {
    pub async fn new(cmd_rx: mpsc::Receiver<Command>) -> Result<Self, Box<dyn std::error::Error>> {
        let cmd_rx = ReceiverStream::new(cmd_rx);

        let context = format!(
            "run:{}:plan:{}:case:{}",
            std::env::var("TEST_RUN").unwrap(),
            std::env::var("TEST_PLAN").unwrap(),
            std::env::var("TEST_CASE").unwrap(),
        );

        let (req_tx, req_rx) = mpsc::channel(10);
        let (res_tx, res_rx) = mpsc::channel(10);

        let mut web = WebsocketClient::new(res_tx, req_rx).await?;

        let handle = tokio::spawn(async move {
            web.run().await;
        });

        let res_rx = ReceiverStream::new(res_rx);

        Ok(Self {
            res_rx,
            next_id: 0,
            context,
            cmd_rx,
            req_tx,
            pending_pub: Default::default(),
            pending_sub: Default::default(),
            pending_signal: Default::default(),
            pending_barrier: Default::default(),
            handle,
        })
    }

    fn contextualize_state(&self, state: String) -> String {
        format!("{}:states:{}", self.context, state)
    }

    fn contextualize_topic(&self, topic: String) -> String {
        format!("{}:topics:{}", self.context, topic)
    }

    fn next_id(&mut self) -> u64 {
        let next_id = self.next_id;
        self.next_id += 1;
        next_id
    }

    pub async fn run(&mut self) {
        loop {
            tokio::select! {
                res = self.res_rx.next() => match res {
                    Some(res) => self.response(res).await,
                    None => return,
                },
                cmd = self.cmd_rx.next() => match cmd {
                    Some(cmd) => self.command(cmd).await,
                    None => return,
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
                let topic = self.contextualize_topic(topic);

                let request = Request {
                    id: id.to_string(),
                    is_cancel: false,
                    request: RequestType::Publish { topic, payload },
                };

                let (tx, rx) = oneshot::channel();

                let msg = (request, tx);

                self.req_tx.send(msg).await.expect("receiver not dropped");

                if let Err(e) = rx.await.expect("sender not dropped") {
                    sender
                        .send(Err(Either::Left(e)))
                        .expect("receiver not dropped");
                } else {
                    self.pending_pub.insert(id, sender);
                }
            }
            Command::Subscribe { topic, stream } => {
                let topic = self.contextualize_topic(topic);

                let request = Request {
                    id: id.to_string(),
                    is_cancel: false,
                    request: RequestType::Subscribe { topic },
                };

                let (tx, rx) = oneshot::channel();

                let msg = (request, tx);

                self.req_tx.send(msg).await.expect("receiver not dropped");

                if let Err(e) = rx.await.expect("sender not dropped") {
                    stream
                        .send(Err(Either::Left(e)))
                        .await
                        .expect("receiver not dropped");
                } else {
                    self.pending_sub.insert(id, stream);
                }
            }
            Command::Signal { state, sender } => {
                let state = self.contextualize_state(state);

                let request = Request {
                    id: id.to_string(),
                    is_cancel: false,
                    request: RequestType::SignalEntry { state },
                };

                let (tx, rx) = oneshot::channel();

                let msg = (request, tx);

                self.req_tx.send(msg).await.expect("receiver not dropped");

                if let Err(e) = rx.await.expect("sender not dropped") {
                    sender
                        .send(Err(Either::Left(e)))
                        .expect("receiver not dropped");
                } else {
                    self.pending_signal.insert(id, sender);
                }
            }
            Command::Barrier {
                state,
                target,
                sender,
            } => {
                let state = self.contextualize_state(state);

                let request = Request {
                    id: id.to_string(),
                    is_cancel: false,
                    request: RequestType::Barrier { state, target },
                };

                let (tx, rx) = oneshot::channel();

                let msg = (request, tx);

                self.req_tx.send(msg).await.expect("receiver not dropped");

                if let Err(e) = rx.await.expect("sender not dropped") {
                    sender
                        .send(Err(Either::Left(e)))
                        .expect("receiver not dropped");
                } else {
                    self.pending_barrier.insert(id, sender);
                }
            }
        }
    }

    async fn response(&mut self, res: Result<Response, ReceiveError>) {
        let res = match res {
            Ok(res) => res,
            Err(e) => {
                eprintln!("{}", e);
                return;
            }
        };

        let Response { id, response } = res;

        let idx = id.parse().unwrap();

        match response {
            ResponseType::SignalEntry { seq } => {
                let sender = match self.pending_signal.remove(&idx) {
                    Some(sender) => sender,
                    None => return,
                };

                let _ = sender.send(Ok(seq));
            }
            ResponseType::Publish { .. } => {
                let sender = match self.pending_pub.remove(&idx) {
                    Some(sender) => sender,
                    None => return,
                };

                let _ = sender.send(Ok(()));
            }
            ResponseType::Subscribe(msg) => {
                let stream = match self.pending_sub.remove(&idx) {
                    Some(sender) => sender,
                    None => return,
                };

                if stream.is_closed() {
                    return;
                }

                stream.send(Ok(msg)).await.unwrap();

                self.pending_sub.insert(idx, stream);
            }
            ResponseType::Error(error) => {
                //TODO
            }
        }
    }
}
