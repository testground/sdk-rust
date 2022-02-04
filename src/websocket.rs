use soketto::handshake::{Client, ServerResponse};

use tokio::{
    net::TcpStream,
    sync::{mpsc, oneshot},
};

use tokio_stream::StreamExt;
use tokio_util::compat::{Compat, TokioAsyncReadCompatExt};

use crate::{
    errors::Error,
    requests::Request,
    responses::{RawResponse, Response},
};

pub struct WebsocketClient<'a> {
    client: Client<'a, Compat<TcpStream>>,

    sender: mpsc::Sender<Result<Response, Error>>,
    receiver: mpsc::Receiver<(Request, oneshot::Sender<Result<(), Error>>)>,
}

impl<'a> WebsocketClient<'a> {
    pub async fn new(
        sender: mpsc::Sender<Result<Response, Error>>,
        receiver: mpsc::Receiver<(Request, oneshot::Sender<Result<(), Error>>)>,
    ) -> Result<WebsocketClient<'a>, Box<dyn std::error::Error>> {
        let socket = tokio::net::TcpStream::connect(("testground-sync-service", 5050)).await?;

        let mut client = Client::new(socket.compat(), "...", "/");

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
        }

        Ok(Self {
            client,
            sender,
            receiver,
        })
    }
    pub async fn run(mut self) {
        let (mut tx, rx) = self.client.into_builder().finish();

        let socket_packets = futures_util::stream::unfold(rx, move |mut rx| async {
            let mut buf = Vec::new();
            let ret = match rx.receive_data(&mut buf).await {
                Ok(ty) => Ok((ty, buf)),
                Err(err) => Err(err),
            };
            Some((ret, rx))
        });

        futures::pin_mut!(socket_packets);

        loop {
            tokio::select! {
                res = socket_packets.next() => {
                    let res = match res {
                        Some(res) => res,
                        None => {
                            eprintln!("Web socket response receiver dropped");
                            return;
                        },
                    };

                    if let Err(e) = res {
                        self.sender
                            .send(Err(e.into()))
                            .await
                            .expect("receiver not dropped");
                        continue;
                    }

                    let (_, buf) = res.unwrap();

                    let msg = serde_json::from_slice::<RawResponse>(&buf).expect("Response Deserialization");

                    self.sender
                        .send(Ok(msg.into()))
                        .await
                        .expect("receiver not dropped");
                },
                req = self.receiver.recv() => {
                    let (req, sender) = match req {
                        Some(req) => req,
                        None => {
                            eprintln!("Web socket request sender dropped");
                            return;
                        },
                    };

                    let mut json = serde_json::to_vec(&req).expect("Request Serialization");

                    if let Err(e) = tx.send_binary_mut(&mut json).await {
                        sender.send(Err(e.into())).expect("receiver not dropped");
                        continue;
                    }

                    if let Err(e) = tx.flush().await {
                        sender.send(Err(e.into())).expect("receiver not dropped");
                        continue;
                    }

                    sender.send(Ok(())).expect("receiver not dropped");
                },
            }
        }
    }
}
