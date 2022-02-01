use std::borrow::Cow;

use tokio::sync::mpsc::{self, channel, Sender};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::Stream;

use crate::background::{BackgroundTask, Command};
use crate::errors::Error;
use crate::events::Event;

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
    ) -> Result<(), Error> {
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
    ) -> impl Stream<Item = Result<String, Error>> {
        let (stream, out) = mpsc::channel(10);

        let cmd = Command::Subscribe {
            topic: topic.into().into_owned(),
            stream,
        };

        self.cmd_tx.send(cmd).await.expect("receiver not dropped");

        ReceiverStream::new(out)
    }

    pub async fn signal_and_wait(
        &self,
        state: impl Into<Cow<'static, str>>,
        target: u64,
    ) -> Result<u64, Error> {
        let (sender, receiver) = oneshot::channel();

        let state = state.into().into_owned();

        let cmd = Command::Signal {
            state: state.clone(),
            sender,
        };

        self.cmd_tx.send(cmd).await.expect("receiver not dropped");

        let res = receiver.await.expect("sender not dropped")?;

        let (sender, receiver) = oneshot::channel();

        let cmd = Command::Barrier {
            state,
            target,
            sender,
        };

        self.cmd_tx.send(cmd).await.expect("receiver not dropped");

        receiver.await.expect("sender not dropped")?;

        Ok(res)
    }

    pub async fn signal(&self, state: impl Into<Cow<'static, str>>) -> Result<u64, Error> {
        let (sender, receiver) = oneshot::channel();

        let state = state.into().into_owned();
        let cmd = Command::Signal { state, sender };

        self.cmd_tx.send(cmd).await.expect("receiver not dropped");

        receiver.await.expect("sender not dropped")
    }

    pub async fn wait_for_barrier(
        &self,
        state: impl Into<Cow<'static, str>>,
        target: u64,
    ) -> Result<(), Error> {
        let (sender, receiver) = oneshot::channel();

        let state = state.into().into_owned();
        let cmd = Command::Barrier {
            state,
            target,
            sender,
        };

        self.cmd_tx.send(cmd).await.expect("receiver not dropped");

        receiver.await.expect("sender not dropped")
    }

    pub async fn wait_network_initialized(&self) -> Result<(), Error> {
        // Event
        let (sender, receiver) = oneshot::channel();

        let cmd = Command::WaitNetworkInitializedStart { sender };

        self.cmd_tx.send(cmd).await.expect("receiver not dropped");

        receiver.await.expect("sender not dropped")?;

        // Barrier
        let (sender, receiver) = oneshot::channel();

        let cmd = Command::WaitNetworkInitializedBarrier { sender };

        self.cmd_tx.send(cmd).await.expect("receiver not dropped");

        receiver.await.expect("sender not dropped")?;

        // Message
        println!(
            "{:?}",
            Event::Message {
                message: "network initialisation successful".to_owned(),
            }
        );

        // Event
        let (sender, receiver) = oneshot::channel();

        let cmd = Command::WaitNetworkInitializedEnd { sender };

        self.cmd_tx.send(cmd).await.expect("receiver not dropped");

        receiver.await.expect("sender not dropped")
    }
}
