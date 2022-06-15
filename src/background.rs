use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use futures::stream::StreamExt;
use influxdb::{Client, WriteQuery};
use soketto::handshake::ServerResponse;
use tokio::sync::{mpsc, oneshot};
use tokio_util::compat::{Compat, TokioAsyncReadCompatExt};

use crate::events::LogLine;
use crate::{
    errors::Error,
    events::{Event, EventType},
    network_conf::NetworkConfiguration,
    params::RunParameters,
    requests::{PayloadType, Request, RequestType},
    responses::{RawResponse, Response, ResponseType},
};

const WEBSOCKET_RECEIVER: &str = "Websocket Receiver";

#[derive(Debug)]
pub enum Command {
    Publish {
        topic: String,
        message: String,
        sender: oneshot::Sender<Result<u64, Error>>,
    },
    Subscribe {
        topic: String,
        stream: mpsc::Sender<Result<String, Error>>,
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
        stream: mpsc::Sender<Result<String, Error>>,
    },
}

pub struct BackgroundTask {
    websocket_tx: soketto::Sender<Compat<tokio::net::TcpStream>>,
    websocket_rx: futures::stream::BoxStream<'static, Result<Vec<u8>, soketto::connection::Error>>,

    influxdb: influxdb::Client,

    next_id: u64,

    params: RunParameters,

    client_rx: mpsc::Receiver<Command>,

    pending_req: HashMap<u64, PendingRequest>,
}

impl BackgroundTask {
    pub async fn new(
        client_rx: mpsc::Receiver<Command>,
        params: RunParameters,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let (websocket_tx, websocket_rx) = {
            let socket = tokio::net::TcpStream::connect(("testground-sync-service", 5050)).await?;

            let mut client = soketto::handshake::Client::new(socket.compat(), "...", "/");
            match client.handshake().await? {
                ServerResponse::Redirect {
                    status_code,
                    location,
                } => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!(
                            "Remote redirected to {}. Status code {}",
                            location, status_code
                        ),
                    )
                    .into())
                }
                ServerResponse::Rejected { status_code } => {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::ConnectionRefused,
                        format!("Remote refused connection. Status code {}", status_code),
                    )
                    .into())
                }
                _ => {}
            };
            let (tx, rx) = client.into_builder().finish();

            let socket_packets = futures::stream::unfold(rx, move |mut rx| async {
                let mut buf = Vec::new();
                let ret = match rx.receive_data(&mut buf).await {
                    Ok(_) => Ok(buf),
                    Err(err) => Err(err),
                };
                Some((ret, rx))
            });

            (tx, socket_packets.boxed())
        };

        let influxdb = Client::new(params.influxdb_url.clone(), "testground");

        Ok(Self {
            websocket_tx,
            websocket_rx,

            influxdb,
            next_id: 0,
            params,
            client_rx,
            pending_req: Default::default(),
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

    pub async fn run(mut self) {
        loop {
            tokio::select! {
                res = self.websocket_rx.next() => match res {
                    Some(res) => match res {
                        Ok(res) => self.response(serde_json::from_slice::<RawResponse>(&res).expect("Response Deserialization").into()).await,
                        Err(e) => {
                            eprintln!("Web socket Error: {}", e);
                            return;
                        }
                    },
                    None => {
                        eprintln!("Web socket receiver dropped");
                        return;
                    },
                },
                cmd = self.client_rx.recv() => match cmd {
                    Some(cmd) => self.command(cmd).await,
                    None => {
                        log::debug!("Client command sender dropped. Background task shutting down.");
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

                self.publish(id, topic, PayloadType::String(message), sender)
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

                self.publish(id, topic, PayloadType::Event(event.event), sender)
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

                self.publish(id, topic, PayloadType::Event(event.event), sender)
                    .await
            }
            Command::NetworkShaping { config, sender } => {
                if !self.params.test_sidecar {
                    let _ = sender.send(Err(Error::SideCar));
                    return;
                }

                let topic = format!("network:{}", self.params.hostname);

                let topic = self.contextualize_topic(&topic);

                self.publish(id, topic, PayloadType::Config(config), sender)
                    .await
            }
            Command::SignalSuccess { sender } => {
                let event = EventType::Success {
                    group: self.params.test_group_id.clone(),
                };

                let topic = self.contextualize_event();

                self.publish(id, topic, PayloadType::Event(event), sender)
                    .await
            }
            Command::SignalFailure { error, sender } => {
                let event = EventType::Failure {
                    group: self.params.test_group_id.clone(),
                    error,
                };

                let topic = self.contextualize_event();

                self.publish(id, topic, PayloadType::Event(event), sender)
                    .await
            }
            Command::SignalCrash {
                error,
                stacktrace,
                sender,
            } => {
                let event = EventType::Crash {
                    groups: self.params.test_group_id.clone(),
                    error,
                    stacktrace,
                };

                let topic = self.contextualize_event();

                self.publish(id, topic, PayloadType::Event(event), sender)
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
        payload: PayloadType,
        sender: oneshot::Sender<Result<u64, Error>>,
    ) {
        if let PayloadType::Event(ref event) = payload {
            // The Testground daemon determines the success or failure of a test
            // instance by parsing stdout for runtime events.
            println!(
                "{}",
                serde_json::to_string(&LogLine {
                    ts: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_nanos(),
                    event,
                })
                .unwrap(),
            );
        }

        let request = Request {
            id: id.to_string(),
            is_cancel: false,
            request: RequestType::Publish { topic, payload },
        };

        self.send(request).await.expect(WEBSOCKET_RECEIVER);

        self.pending_req
            .insert(id, PendingRequest::PublishOrSignal { sender });
    }

    async fn subscribe(
        &mut self,
        id: u64,
        topic: String,
        stream: mpsc::Sender<Result<String, Error>>,
    ) {
        let request = Request {
            id: id.to_string(),
            is_cancel: false,
            request: RequestType::Subscribe { topic },
        };

        self.send(request).await.expect(WEBSOCKET_RECEIVER);

        self.pending_req
            .insert(id, PendingRequest::Subscribe { stream });
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

        self.send(request).await.expect(WEBSOCKET_RECEIVER);

        self.pending_req
            .insert(id, PendingRequest::PublishOrSignal { sender });
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

        self.send(request).await.expect(WEBSOCKET_RECEIVER);

        self.pending_req
            .insert(id, PendingRequest::Barrier { sender });
    }

    async fn response(&mut self, res: Response) {
        let Response { id, response } = res;

        let idx = id.parse().unwrap();

        let pending_req = match self.pending_req.remove(&idx) {
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
                if stream.send(Ok(msg)).await.is_ok() {
                    self.pending_req
                        .insert(idx, PendingRequest::Subscribe { stream });
                }
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

    async fn send(&mut self, req: Request) -> Result<(), ()> {
        let mut json = serde_json::to_vec(&req).expect("Request Serialization");

        self.websocket_tx.send_binary_mut(&mut json).await.unwrap();

        self.websocket_tx.flush().await.unwrap();

        Ok(())
    }
}
