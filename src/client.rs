use std::borrow::Cow;

use crate::{
    background::{BackgroundTask, Command},
    errors::Error,
    events::{Event, EventType},
    network_conf::NetworkConfiguration,
    RunParameters,
};

use clap::Parser;

use influxdb::WriteQuery;

use tokio::sync::{
    mpsc::{self, channel, Sender},
    oneshot,
};
use tokio_stream::{wrappers::ReceiverStream, Stream};

const BACKGROUND_RECEIVER: &str = "Background Receiver";
const BACKGROUND_SENDER: &str = "Background Sender";

/// Basic synchronization client enabling one to send signals, await barriers and subscribe or publish to a topic.
#[derive(Clone)]
pub struct Client {
    cmd_tx: Sender<Command>,
}

impl Client {
    pub async fn new() -> Result<(Self, RunParameters), Box<dyn std::error::Error>> {
        let params = RunParameters::try_parse()?;

        let (cmd_tx, cmd_rx) = channel(1);

        let background = BackgroundTask::new(cmd_rx, params.clone()).await?;

        tokio::spawn(background.run());

        Ok((Self { cmd_tx }, params))
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

        self.cmd_tx.send(cmd).await.expect(BACKGROUND_RECEIVER);

        receiver.await.expect(BACKGROUND_SENDER)
    }

    /// ```subscribe``` subscribes to a topic, consuming ordered, elements from index 0.
    pub async fn subscribe(
        &self,
        topic: impl Into<Cow<'static, str>>,
    ) -> impl Stream<Item = Result<String, Error>> {
        let (stream, out) = mpsc::channel(1);

        let cmd = Command::Subscribe {
            topic: topic.into().into_owned(),
            stream,
        };

        self.cmd_tx.send(cmd).await.expect(BACKGROUND_RECEIVER);

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
        let state = state.into().into_owned();

        let res = self.signal(state.clone()).await?;

        self.barrier(state, target).await?;

        Ok(res)
    }

    /// ```signal``` increments the state counter by one,
    /// returning the value of the new value of the counter,
    /// or an error if the operation fails.
    pub async fn signal(&self, state: impl Into<Cow<'static, str>>) -> Result<u64, Error> {
        let (sender, receiver) = oneshot::channel();

        let state = state.into().into_owned();
        let cmd = Command::SignalEntry { state, sender };

        self.cmd_tx.send(cmd).await.expect(BACKGROUND_RECEIVER);

        receiver.await.expect(BACKGROUND_SENDER)
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

        self.cmd_tx.send(cmd).await.expect(BACKGROUND_RECEIVER);

        receiver.await.expect(BACKGROUND_SENDER)
    }

    /// ```wait_network_initialized``` waits for the sidecar to initialize the network,
    /// if the sidecar is enabled.
    pub async fn wait_network_initialized(&self) -> Result<(), Error> {
        // Event
        let (sender, receiver) = oneshot::channel();

        let cmd = Command::WaitNetworkInitializedStart { sender };

        self.cmd_tx.send(cmd).await.expect(BACKGROUND_RECEIVER);

        receiver.await.expect(BACKGROUND_SENDER)?;

        // Barrier
        let (sender, receiver) = oneshot::channel();

        let cmd = Command::WaitNetworkInitializedBarrier { sender };

        self.cmd_tx.send(cmd).await.expect(BACKGROUND_RECEIVER);

        receiver.await.expect(BACKGROUND_SENDER)?;

        // Event
        let (sender, receiver) = oneshot::channel();

        let cmd = Command::WaitNetworkInitializedEnd { sender };

        self.cmd_tx.send(cmd).await.expect(BACKGROUND_RECEIVER);

        receiver.await.expect(BACKGROUND_SENDER)?;

        Ok(())
    }

    /// ```configure_network``` asks the sidecar to configure the network.
    pub async fn configure_network(&self, config: NetworkConfiguration) -> Result<(), Error> {
        // Publish
        let (sender, receiver) = oneshot::channel();

        let state = config.callback_state.clone();
        let target = if let Some(callback_target) = config.callback_target {
            callback_target
        } else {
            0
        };

        let cmd = Command::NetworkShaping { sender, config };

        self.cmd_tx.send(cmd).await.expect(BACKGROUND_RECEIVER);

        receiver.await.expect(BACKGROUND_SENDER)?;

        self.barrier(state, target).await?;

        Ok(())
    }

    pub fn record_message(&self, message: impl Into<Cow<'static, str>>) {
        let message = message.into().into_owned();

        let event = Event {
            event: EventType::Message { message },
        };

        //TODO implement logger similar to go-sdk

        let json_event = serde_json::to_string(&event).expect("Event Serialization");

        println!("{}", json_event);
    }

    pub async fn record_success(self) -> Result<(), Error> {
        let (sender, receiver) = oneshot::channel();

        let cmd = Command::SignalSuccess { sender };

        self.cmd_tx.send(cmd).await.expect(BACKGROUND_RECEIVER);

        receiver.await.expect(BACKGROUND_SENDER)?;

        Ok(())
    }

    pub async fn record_failure(self, error: impl Into<Cow<'static, str>>) -> Result<(), Error> {
        let error = error.into().into_owned();

        let (sender, receiver) = oneshot::channel();

        let cmd = Command::SignalFailure { error, sender };

        self.cmd_tx.send(cmd).await.expect(BACKGROUND_RECEIVER);

        receiver.await.expect(BACKGROUND_SENDER)?;

        Ok(())
    }

    pub async fn record_crash(
        self,
        error: impl Into<Cow<'static, str>>,
        stacktrace: impl Into<Cow<'static, str>>,
    ) -> Result<(), Error> {
        let error = error.into().into_owned();
        let stacktrace = stacktrace.into().into_owned();

        let (sender, receiver) = oneshot::channel();

        let cmd = Command::SignalCrash {
            error,
            stacktrace,
            sender,
        };

        self.cmd_tx.send(cmd).await.expect(BACKGROUND_RECEIVER);

        receiver.await.expect(BACKGROUND_SENDER)?;

        Ok(())
    }

    pub async fn record_metric(&self, write_query: WriteQuery) -> Result<(), Error> {
        let (sender, receiver) = oneshot::channel();

        let cmd = Command::Metric {
            write_query,
            sender,
        };

        self.cmd_tx.send(cmd).await.expect(BACKGROUND_RECEIVER);

        receiver.await.expect(BACKGROUND_SENDER)?;

        Ok(())
    }
}
