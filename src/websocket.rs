use soketto::{
    handshake::{Client, ServerResponse},
    Data,
};

use tokio::{
    net::TcpStream,
    sync::{mpsc, oneshot},
};

use tokio_stream::StreamExt;
use tokio_util::compat::{Compat, TokioAsyncReadCompatExt};

use crate::{errors::Error, requests::Request, responses::Response};

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
                    let res: Result<(Data, Vec<u8>), soketto::connection::Error> = res.unwrap();//infinite stream

                    if let Err(e) = res {
                        self.sender
                            .send(Err(e.into()))
                            .await
                            .expect("receiver not dropped");
                        continue;
                    }

                    let (_, buf) = res.unwrap();

                    #[cfg(debug_assertions)]
                    {
                        let string_res = String::from_utf8_lossy(&buf);

                        println!("Raw response: {}", string_res);
                    }

                    match serde_json::from_slice(&buf) {
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
                    let (req, sender) = match req {
                        Some(req) => req,
                        None => {
                            eprintln!("Web socket request sender dropped");
                            return;
                        },
                    };

                    let json = match serde_json::to_string(&req) {
                        Ok(j) => j,
                        Err(e) => {
                            sender.send(Err(e.into())).expect("receiver not dropped");
                            continue;
                        }
                    };

                    if let Err(e) = tx.send_text(json).await {
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
