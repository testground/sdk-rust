use std::borrow::Cow;

use tokio::{
    sync::{
        mpsc::{self, channel, Sender},
        oneshot,
    },
    task::JoinHandle,
};
use tokio_stream::{wrappers::ReceiverStream, Stream};

use crate::{
    background::{BackgroundTask, Command},
    errors::Error,
    network_conf::NetworkConfiguration,
};

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

    /// ```publish``` publishes an item on the supplied topic.
    ///
    /// Once the item has been published successfully,
    /// returning the sequence number of the new item in the ordered topic,
    /// or an error if one occurred, starting with 1 (for the first item).
    pub async fn publish(
        &self,
        topic: impl Into<Cow<'static, str>>,
        message: impl Into<Cow<'static, str>>,
    ) -> Result<u64, Error> {
        let (sender, receiver) = oneshot::channel();

        let cmd = Command::Publish {
            topic: topic.into().into_owned(),
            message: message.into().into_owned(),
            sender,
        };

        self.cmd_tx.send(cmd).await.expect("receiver not dropped");

        receiver.await.expect("sender not dropped")
    }

    /// ```subscribe``` subscribes to a topic, consuming ordered, elements from index 0.
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

    /// ```signal_and_wait``` composes SignalEntry and Barrier,
    /// signalling entry on the supplied state,
    /// and then awaiting until the required value has been reached.
    pub async fn signal_and_wait(
        &self,
        state: impl Into<Cow<'static, str>>,
        target: u64,
    ) -> Result<u64, Error> {
        let (sender, receiver) = oneshot::channel();

        let state = state.into().into_owned();

        let cmd = Command::SignalEntry {
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

    /// ```signal``` increments the state counter by one,
    /// returning the value of the new value of the counter,
    /// or an error if the operation fails.
    pub async fn signal(&self, state: impl Into<Cow<'static, str>>) -> Result<u64, Error> {
        let (sender, receiver) = oneshot::channel();

        let state = state.into().into_owned();
        let cmd = Command::SignalEntry { state, sender };

        self.cmd_tx.send(cmd).await.expect("receiver not dropped");

        receiver.await.expect("sender not dropped")
    }

    /// ```barrier``` sets a barrier on the supplied ```state``` that fires when it reaches its target value (or higher).
    pub async fn barrier(
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

    /// ```wait_network_initialized``` waits for the sidecar to initialize the network,
    /// if the sidecar is enabled.
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

        // Event
        let (sender, receiver) = oneshot::channel();

        let cmd = Command::WaitNetworkInitializedEnd { sender };

        self.cmd_tx.send(cmd).await.expect("receiver not dropped");

        receiver.await.expect("sender not dropped")?;

        Ok(())
    }

    /// ```configure_network``` asks the sidecar to configure the network.
    pub async fn configure_network(&self, config: NetworkConfiguration) -> Result<(), Error> {
        // Publish
        let (sender, receiver) = oneshot::channel();

        let state = config.callback_state.clone();
        let target = config.callback_target;

        let cmd = Command::NetworkShaping { sender, config };

        self.cmd_tx.send(cmd).await.expect("receiver not dropped");

        receiver.await.expect("sender not dropped")?;

        // Barrier
        let (sender, receiver) = oneshot::channel();

        let cmd = Command::Barrier {
            state,
            target,
            sender,
        };

        self.cmd_tx.send(cmd).await.expect("receiver not dropped");

        receiver.await.expect("sender not dropped")?;

        Ok(())
    }
}
