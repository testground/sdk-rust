use soketto::handshake::{Client, ServerResponse};

use tokio::sync::{mpsc, oneshot};

use tokio_util::compat::{Compat, TokioAsyncReadCompatExt};

use crate::{
    errors::{ReceiveError, SendError},
    requests::Request,
    responses::Response,
};

pub struct WebsocketClient {
    rx: soketto::connection::Receiver<Compat<tokio::net::TcpStream>>,
    tx: soketto::connection::Sender<Compat<tokio::net::TcpStream>>,

    sender: mpsc::Sender<Result<Response, ReceiveError>>,
    receiver: mpsc::Receiver<(Request, oneshot::Sender<Result<(), SendError>>)>,
}

impl WebsocketClient {
    pub async fn new(
        sender: mpsc::Sender<Result<Response, ReceiveError>>,
        receiver: mpsc::Receiver<(Request, oneshot::Sender<Result<(), SendError>>)>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let socket = tokio::net::TcpStream::connect(("testground-sync-service", 5050)).await?;

        let mut client = Client::new(socket.compat(), "...", "/");

        let (tx, rx) = match client.handshake().await? {
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
            rx,
            tx,
            sender,
            receiver,
        })
    }

    pub async fn run(&mut self) {
        let mut message = Vec::new();

        loop {
            message.clear();

            tokio::select! {
                res = self.rx.receive_data(&mut message) => {
                    if let Err(e) = res {
                        self.sender
                            .send(Err(e.into()))
                            .await
                            .expect("receiver not dropped");
                        continue;
                    }

                    match serde_json::from_slice(&message) {
                        Ok(msg) => {
                            self.sender
                                .send(Ok(msg))
                                .await
                                .expect("receiver not dropped");
                        }
                        Err(e) => {
                            self.sender
                                .send(Err(e.into()))
                                .await
                                .expect("receiver not dropped");
                        }
                    }
                },
                req = self.receiver.recv() => {
                    let (req, tx) = match req {
                        Some(req) => req,
                        None => continue,
                    };

                    let res = self.send_request(req).await;

                    tx.send(res).expect("receiver not dropped");
                },
            }
        }
    }

    async fn send_request(&mut self, req: Request) -> Result<(), SendError> {
        let json = serde_json::to_string(&req)?;

        self.tx.send_text(json).await?;

        self.tx.flush().await?;

        Ok(())
    }
}
