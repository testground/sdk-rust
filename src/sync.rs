use serde::{Deserialize, Serialize};
use soketto::connection::{Receiver, Sender};
use soketto::handshake::{self, ServerResponse};
use thiserror::Error;

/// Basic synchronization client enabling one to send signals and await barriers.
pub struct Client {
    next_id: u64,
    sender: Sender<async_std::net::TcpStream>,
    receiver: Receiver<async_std::net::TcpStream>,
}

impl Client {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let socket = async_std::net::TcpStream::connect(("testground-sync-service", 5050)).await?;

        let mut client = handshake::Client::new(socket, "...", "/");

        let (sender, receiver) = match client.handshake().await? {
            ServerResponse::Accepted { .. } => client.into_builder().finish(),
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
        };

        Ok(Self {
            sender,
            receiver,
            next_id: 0,
        })
    }

    pub async fn signal(&mut self, state: String) -> Result<u64, SignalError> {
        let id = self.next_id().to_string();

        let contextualized_state = contextualize_state(state);
        let request = Request {
            id: id.clone(),
            signal_entry: Some(SignalEntryRequest {
                state: contextualized_state,
            }),
            barrier: None,
            publish: None,
        };

        self.send(request).await?;
        let resp = self.receive().await?;
        if resp.id != id {
            return Err(SignalError::UnexpectedId(resp.id));
        }
        if !resp.error.is_empty() {
            return Err(SignalError::Remote(resp.error));
        }
        resp.signal_entry
            .ok_or(SignalError::ExpectedSignalEntryInResponse)
            .map(|resp| resp.seq)
    }

    pub async fn wait_for_barrier(
        &mut self,
        state: String,
        target: u64,
    ) -> Result<(), BarrierError> {
        let id = self.next_id().to_string();

        let contextualized_state = contextualize_state(state);
        let request = Request {
            id: id.clone(),
            signal_entry: None,
            publish: None,
            barrier: Some(BarrierRequest {
                state: contextualized_state,
                target,
            }),
        };

        self.send(request).await?;
        let resp = self.receive().await?;
        if resp.id != id {
            return Err(BarrierError::UnexpectedId(resp.id));
        }
        if !resp.error.is_empty() {
            return Err(BarrierError::Remote(resp.error));
        }
        Ok(())
    }

    pub async fn publish_success(&mut self) -> Result<u64, PublishSuccessError> {
        let id = self.next_id().to_string();

        let request = Request {
            id: id.clone(),
            signal_entry: None,
            barrier: None,
            publish: Some(PublishRequest {
                topic: topic(),
                payload: Event {
                    success_event: SuccessEvent {
                        group: std::env::var("TEST_GROUP_ID").unwrap(),
                    },
                },
            }),
        };

        self.send(request).await?;
        let resp = self.receive().await?;
        if resp.id != id {
            return Err(PublishSuccessError::UnexpectedId(resp.id));
        }
        if !resp.error.is_empty() {
            return Err(PublishSuccessError::Remote(resp.error));
        }
        resp.publish
            .ok_or(PublishSuccessError::ExpectedPublishInResponse)
            .map(|resp| resp.seq)
    }

    fn next_id(&mut self) -> u64 {
        let next_id = self.next_id;
        self.next_id += 1;
        next_id
    }

    async fn send(&mut self, req: Request) -> Result<(), SendError> {
        self.sender.send_text(serde_json::to_string(&req)?).await?;
        self.sender.flush().await?;
        Ok(())
    }

    async fn receive(&mut self) -> Result<Response, ReceiveError> {
        let mut data = Vec::new();
        self.receiver.receive_data(&mut data).await?;
        let resp = serde_json::from_str(&String::from_utf8(data)?)?;
        Ok(resp)
    }
}

fn context_from_env() -> String {
    format!(
        "run:{}:plan:{}:case:{}",
        std::env::var("TEST_RUN").unwrap(),
        std::env::var("TEST_PLAN").unwrap(),
        std::env::var("TEST_CASE").unwrap(),
    )
}

fn contextualize_state(state: String) -> String {
    format!("{}:states:{}", context_from_env(), state,)
}

fn topic() -> String {
    format!("{}:run_events", context_from_env(),)
}

#[derive(Error, Debug)]
pub enum SignalError {
    #[error("Remote returned error {0}.")]
    Remote(String),
    #[error("Expected remote to return signal entry in response.")]
    ExpectedSignalEntryInResponse,
    #[error("Remote returned response with unexpected ID {0}.")]
    UnexpectedId(String),
    #[error("Error sending request: {0}")]
    Send(#[from] SendError),
    #[error("Error receiving response: {0}")]
    Receive(#[from] ReceiveError),
}

#[derive(Error, Debug)]
pub enum BarrierError {
    #[error("Remote returned error {0}.")]
    Remote(String),
    #[error("Remote returned response with unexpected ID {0}.")]
    UnexpectedId(String),
    #[error("Error sending request {0}")]
    Send(#[from] SendError),
    #[error("Error receiving response: {0}")]
    Receive(#[from] ReceiveError),
}

#[derive(Error, Debug)]
pub enum PublishSuccessError {
    #[error("Remote returned error {0}.")]
    Remote(String),
    #[error("Remote returned response with unexpected ID {0}.")]
    UnexpectedId(String),
    #[error("Expected remote to return signal entry in response.")]
    ExpectedPublishInResponse,
    #[error("Error sending request {0}")]
    Send(#[from] SendError),
    #[error("Error receiving response: {0}")]
    Receive(#[from] ReceiveError),
}

#[derive(Error, Debug)]
pub enum SendError {
    #[error("Soketto: {0}")]
    Soketto(#[from] soketto::connection::Error),
    #[error("Serde: {0}")]
    Serde(#[from] serde_json::error::Error),
}

#[derive(Error, Debug)]
pub enum ReceiveError {
    #[error("Soketto: {0}")]
    Soketto(#[from] soketto::connection::Error),
    #[error("Serde: {0}")]
    Serde(#[from] serde_json::error::Error),
    #[error("{0}")]
    FromUtf8(#[from] std::string::FromUtf8Error),
}

#[derive(Serialize)]
struct Request {
    id: String,
    signal_entry: Option<SignalEntryRequest>,
    barrier: Option<BarrierRequest>,
    publish: Option<PublishRequest>,
}

#[derive(Serialize)]
struct SignalEntryRequest {
    state: String,
}

#[derive(Serialize)]
struct BarrierRequest {
    state: String,
    target: u64,
}

#[derive(Serialize)]
struct PublishRequest {
    topic: String,
    payload: Event,
}

#[derive(Serialize)]
struct Event {
    success_event: SuccessEvent,
}

#[derive(Serialize)]
struct SuccessEvent {
    group: String,
}

#[derive(Deserialize, Debug)]
struct Response {
    id: String,
    signal_entry: Option<SignalEntryResponse>,
    error: String,
    publish: Option<PublishResponse>,
}

#[derive(Deserialize, Debug)]
struct SignalEntryResponse {
    seq: u64,
}

#[derive(Deserialize, Debug)]
struct PublishResponse {
    seq: u64,
}
