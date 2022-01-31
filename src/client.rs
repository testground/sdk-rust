use std::borrow::Cow;

use tokio::sync::mpsc::{self, channel, Sender};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::Stream;
use tokio_util::either::Either;

use crate::background::{BackgroundTask, Command};
use crate::errors::{ReceiveError, SendError};

/// Basic synchronization client enabling one to send signals, await barriers and subscribe or publish to a topic.
pub struct Client {
    cmd_tx: Sender<Command>,
    handle: JoinHandle<()>,
}

impl Drop for Client {
    fn drop(&mut self) {
        self.handle.abort();
    }
}

impl Client {
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let (cmd_tx, cmd_rx) = channel(10);

        let mut background = BackgroundTask::new(cmd_rx).await?;

        let handle = tokio::spawn(async move {
            background.run().await;
        });

        Ok(Self { cmd_tx, handle })
    }

    pub async fn publish(
        &self,
        topic: impl Into<Cow<'static, str>>,
        payload: Vec<u8>,
    ) -> Result<(), Either<SendError, ReceiveError>> {
        let (sender, receiver) = oneshot::channel();

        let cmd = Command::Publish {
            topic: topic.into().into_owned(),
            payload,
            sender,
        };

        self.cmd_tx.send(cmd).await.expect("receiver not dropped");

        receiver.await.expect("sender not dropped")
    }

    pub async fn subscribe(
        &self,
        topic: impl Into<Cow<'static, str>>,
    ) -> impl Stream<Item = Result<String, Either<SendError, ReceiveError>>> {
        let (stream, out) = mpsc::channel(10);

        let cmd = Command::Subscribe {
            topic: topic.into().into_owned(),
            stream,
        };

        self.cmd_tx.send(cmd).await.expect("receiver not dropped");

        ReceiverStream::new(out)
    }

    pub async fn signal(&mut self, state: String) -> Result<u64, Either<SendError, ReceiveError>> {
        let (sender, receiver) = oneshot::channel();

        let cmd = Command::Signal { state, sender };

        self.cmd_tx.send(cmd).await.expect("receiver not dropped");

        receiver.await.expect("sender not dropped")
    }

    pub async fn wait_for_barrier(
        &mut self,
        state: String,
        target: u64,
    ) -> Result<(), Either<SendError, ReceiveError>> {
        let (sender, receiver) = oneshot::channel();

        let cmd = Command::Barrier {
            state,
            target,
            sender,
        };

        self.cmd_tx.send(cmd).await.expect("receiver not dropped");

        receiver.await.expect("sender not dropped")
    }
}
