use std::collections::HashMap;

use influxdb::{Client, WriteQuery};
use structopt::StructOpt;
use tokio::{
    sync::{mpsc, oneshot},
    task::JoinHandle,
};

use tokio_stream::{wrappers::UnboundedReceiverStream, StreamExt};

use crate::{
    errors::Error,
    events::{Event, EventType},
    network_conf::NetworkConfiguration,
    params::RunParameters,
    requests::{PlayloadType, Request, RequestType},
    responses::{Response, ResponseType},
    websocket::WebsocketClient,
};

const WEBSOCKET_RECEIVER: &str = "Websocket Receiver";
const WEBSOCKET_SENDER: &str = "Websocket Sender";

#[derive(Debug)]
pub enum Command {
    Publish {
        topic: String,
        message: String,
        sender: oneshot::Sender<Result<u64, Error>>,
    },
    Subscribe {
        topic: String,
        stream: mpsc::UnboundedSender<Result<String, Error>>,
    },

    SignalEntry {
        state: String,
        sender: oneshot::Sender<Result<u64, Error>>,
    },

    Barrier {
        state: String,
        target: u64,
        sender: oneshot::Sender<Result<(), Error>>,
    },

    WaitNetworkInitializedStart {
        sender: oneshot::Sender<Result<u64, Error>>,
    },

    WaitNetworkInitializedBarrier {
        sender: oneshot::Sender<Result<(), Error>>,
    },

    WaitNetworkInitializedEnd {
        sender: oneshot::Sender<Result<u64, Error>>,
    },

    NetworkShaping {
        config: NetworkConfiguration,
        sender: oneshot::Sender<Result<u64, Error>>,
    },

    SignalSuccess {
        sender: oneshot::Sender<Result<u64, Error>>,
    },

    SignalFailure {
        error: String,
        sender: oneshot::Sender<Result<u64, Error>>,
    },

    SignalCrash {
        error: String,
        stacktrace: String,
        sender: oneshot::Sender<Result<u64, Error>>,
    },

    Metric {
        write_query: WriteQuery,
        sender: oneshot::Sender<Result<(), Error>>,
    },
}

#[derive(Debug)]
enum PendingRequest {
    PublishOrSignal {
        sender: oneshot::Sender<Result<u64, Error>>,
    },
    Barrier {
        sender: oneshot::Sender<Result<(), Error>>,
    },
    Subscribe {
        stream: mpsc::UnboundedSender<Result<String, Error>>,
    },
}

pub struct BackgroundTask {
    websocket_tx: mpsc::UnboundedSender<(Request, oneshot::Sender<Result<(), Error>>)>,
    handle: JoinHandle<()>,
    websocket_rx: UnboundedReceiverStream<Result<Response, Error>>,

    influxdb: influxdb::Client,

    next_id: u64,

    params: RunParameters,

    client_rx: UnboundedReceiverStream<Command>,

    pending_cmd: HashMap<u64, PendingRequest>,
}

impl Drop for BackgroundTask {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

impl BackgroundTask {
    pub async fn new(
        client_rx: mpsc::UnboundedReceiver<Command>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let client_rx = UnboundedReceiverStream::new(client_rx);

        let params = RunParameters::from_args();

        let (websocket_tx, req_rx) = mpsc::unbounded_channel();
        let (res_tx, websocket_rx) = mpsc::unbounded_channel();

        let web = WebsocketClient::new(res_tx, req_rx).await?;

        let handle = tokio::spawn(async move {
            web.run().await;
        });

        let websocket_rx = UnboundedReceiverStream::new(websocket_rx);

        let influxdb = Client::new(params.influxdb_url.clone(), "testground");

        Ok(Self {
            websocket_tx,
            websocket_rx,
            handle,
            influxdb,
            next_id: 0,
            params,
            client_rx,
            pending_cmd: Default::default(),
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
                message,
                sender,
            } => {
                let topic = self.contextualize_topic(&topic);

                self.publish(id, topic, PlayloadType::String(message), sender)
                    .await
            }
            Command::Subscribe { topic, stream } => {
                let topic = self.contextualize_topic(&topic);

                self.subscribe(id, topic, stream).await
            }
            Command::SignalEntry { state, sender } => {
                let state = self.contextualize_state(&state);

                self.signal(id, state, sender).await
            }
            Command::Barrier {
                state,
                mut target,
                sender,
            } => {
                let state = self.contextualize_state(&state);

                if target == 0 {
                    target = self.params.test_instance_count;
                }

                self.barrier(id, state, target, sender).await
            }
            Command::WaitNetworkInitializedStart { sender } => {
                let event = Event {
                    event: EventType::StageStart {
                        name: "network-initialized".to_owned(),
                        group: self.params.test_group_id.clone(),
                    },
                };

                let topic = self.contextualize_event();

                self.publish(id, topic, PlayloadType::Event(event), sender)
                    .await
            }
            Command::WaitNetworkInitializedBarrier { sender } => {
                if !self.params.test_sidecar {
                    let _ = sender.send(Err(Error::SideCar));
                    return;
                }

                let state = self.contextualize_state("network-initialized");
                let target = self.params.test_instance_count;

                self.barrier(id, state, target, sender).await;
            }
            Command::WaitNetworkInitializedEnd { sender } => {
                let event = Event {
                    event: EventType::StageEnd {
                        name: "network-initialized".to_owned(),
                        group: self.params.test_group_id.clone(),
                    },
                };

                let topic = self.contextualize_event();

                self.publish(id, topic, PlayloadType::Event(event), sender)
                    .await
            }
            Command::NetworkShaping { config, sender } => {
                if !self.params.test_sidecar {
                    let _ = sender.send(Err(Error::SideCar));
                    return;
                }

                let topic = format!("network:{}", self.params.hostname);

                let topic = self.contextualize_topic(&topic);

                self.publish(id, topic, PlayloadType::Config(config), sender)
                    .await
            }
            Command::SignalSuccess { sender } => {
                let event = Event {
                    event: EventType::Success {
                        groups: self.params.test_group_id.clone(),
                    },
                };

                let topic = self.contextualize_event();

                self.publish(id, topic, PlayloadType::Event(event), sender)
                    .await
            }
            Command::SignalFailure { error, sender } => {
                let event = Event {
                    event: EventType::Failure {
                        groups: self.params.test_group_id.clone(),
                        error,
                    },
                };

                let topic = self.contextualize_event();

                self.publish(id, topic, PlayloadType::Event(event), sender)
                    .await
            }
            Command::SignalCrash {
                error,
                stacktrace,
                sender,
            } => {
                let event = Event {
                    event: EventType::Crash {
                        groups: self.params.test_group_id.clone(),
                        error,
                        stacktrace,
                    },
                };

                let topic = self.contextualize_event();

                self.publish(id, topic, PlayloadType::Event(event), sender)
                    .await
            }
            Command::Metric {
                write_query,
                sender,
            } => {
                //TODO add global tag to the query before processing

                match self.influxdb.query(write_query).await {
                    Ok(_) => {
                        let _ = sender.send(Ok(()));
                    }
                    Err(e) => {
                        let _ = sender.send(Err(e.into()));
                    }
                }
            }
        }
    }

    async fn publish(
        &mut self,
        id: u64,
        topic: String,
        payload: PlayloadType,
        sender: oneshot::Sender<Result<u64, Error>>,
    ) {
        let request = Request {
            id: id.to_string(),
            is_cancel: false,
            request: RequestType::Publish { topic, payload },
        };

        let (tx, websocket_rx) = oneshot::channel();

        let msg = (request, tx);

        self.websocket_tx.send(msg).expect(WEBSOCKET_RECEIVER);

        if let Err(e) = websocket_rx.await.expect(WEBSOCKET_SENDER) {
            let _ = sender.send(Err(e));
        } else {
            self.pending_cmd
                .insert(id, PendingRequest::PublishOrSignal { sender });
        }
    }

    async fn subscribe(
        &mut self,
        id: u64,
        topic: String,
        stream: mpsc::UnboundedSender<Result<String, Error>>,
    ) {
        let request = Request {
            id: id.to_string(),
            is_cancel: false,
            request: RequestType::Subscribe { topic },
        };

        let (tx, websocket_rx) = oneshot::channel();

        let msg = (request, tx);

        self.websocket_tx.send(msg).expect(WEBSOCKET_RECEIVER);

        if let Err(e) = websocket_rx.await.expect(WEBSOCKET_SENDER) {
            let _ = stream.send(Err(e));
        } else {
            self.pending_cmd
                .insert(id, PendingRequest::Subscribe { stream });
        }
    }

    async fn signal(
        &mut self,
        id: u64,
        state: String,
        sender: oneshot::Sender<Result<u64, Error>>,
    ) {
        let request = Request {
            id: id.to_string(),
            is_cancel: false,
            request: RequestType::SignalEntry { state },
        };

        let (tx, websocket_rx) = oneshot::channel();

        let msg = (request, tx);

        self.websocket_tx.send(msg).expect(WEBSOCKET_RECEIVER);

        if let Err(e) = websocket_rx.await.expect(WEBSOCKET_SENDER) {
            let _ = sender.send(Err(e));
        } else {
            self.pending_cmd
                .insert(id, PendingRequest::PublishOrSignal { sender });
        }
    }

    async fn barrier(
        &mut self,
        id: u64,
        state: String,
        target: u64,
        sender: oneshot::Sender<Result<(), Error>>,
    ) {
        let request = Request {
            id: id.to_string(),
            is_cancel: false,
            request: RequestType::Barrier { state, target },
        };

        let (tx, websocket_rx) = oneshot::channel();

        let msg = (request, tx);

        self.websocket_tx.send(msg).expect(WEBSOCKET_RECEIVER);

        if let Err(e) = websocket_rx.await.expect(WEBSOCKET_SENDER) {
            let _ = sender.send(Err(e));
        } else {
            self.pending_cmd
                .insert(id, PendingRequest::Barrier { sender });
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

        let Response { id, response } = res;

        let idx = id.parse().unwrap();

        let pending_req = match self.pending_cmd.remove(&idx) {
            Some(req) => req,
            None => return,
        };

        match (pending_req, response) {
            (PendingRequest::Barrier { sender }, ResponseType::Error(error)) => {
                let _ = sender.send(Err(Error::SyncService(error)));
            }
            (PendingRequest::PublishOrSignal { sender }, ResponseType::Error(error)) => {
                let _ = sender.send(Err(Error::SyncService(error)));
            }
            (PendingRequest::Subscribe { stream }, ResponseType::Error(error)) => {
                let _ = stream.send(Err(Error::SyncService(error)));
            }
            (PendingRequest::Subscribe { stream }, ResponseType::Subscribe(msg)) => {
                if let Err(_) = stream.send(Ok(msg)) {
                    return;
                }

                self.pending_cmd
                    .insert(idx, PendingRequest::Subscribe { stream });
            }
            (PendingRequest::PublishOrSignal { sender }, ResponseType::SignalEntry { seq }) => {
                let _ = sender.send(Ok(seq));
            }
            (PendingRequest::PublishOrSignal { sender }, ResponseType::Publish { seq }) => {
                let _ = sender.send(Ok(seq));
            }
            (PendingRequest::Barrier { sender }, ResponseType::Barrier) => {
                let _ = sender.send(Ok(()));
            }
            (req, res) => {
                panic!("No match Request: {:?} Response: {:?}", req, res);
            }
        }
    }
}
