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

    sender: mpsc::UnboundedSender<Result<Response, Error>>,
    receiver: mpsc::UnboundedReceiver<(Request, oneshot::Sender<Result<(), Error>>)>,
}

impl<'a> WebsocketClient<'a> {
    pub async fn new(
        sender: mpsc::UnboundedSender<Result<Response, Error>>,
        receiver: mpsc::UnboundedReceiver<(Request, oneshot::Sender<Result<(), Error>>)>,
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
                    let res = res.expect("Websocket Receiver");

                    let (_, buf) =  match res {
                        Ok(res) => res,
                        Err(e) => {
                            self.sender.send(Err(e.into())).expect("Receiver");
                            continue;
                        }
                    };

                    let msg = serde_json::from_slice::<RawResponse>(&buf).expect("Response Deserialization");

                    self.sender.send(Ok(msg.into())).expect("Receiver");

                },
                req = self.receiver.recv() => {
                    let (req, sender) = req.expect("Websocket Request Sender");

                    let mut json = serde_json::to_vec(&req).expect("Request Serialization");

                    if let Err(e) = tx.send_binary_mut(&mut json).await {
                        let _ = sender.send(Err(e.into()));
                        continue;
                    }

                    if let Err(e) = tx.flush().await {
                        let _ = sender.send(Err(e.into()));
                        continue;
                    }

                    let _ = sender.send(Ok(()));
                },
            }
        }
    }
}
