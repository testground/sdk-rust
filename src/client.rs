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
    /// The runtime parameters for this test.
    pub run_parameters: RunParameters,
    /// A global sequence number assigned to this test instance by the sync service.
    pub global_seq: u64,
    /// A group-scoped sequence number assigned to this test instance by the sync service.
    pub group_seq: u64,
}

impl Client {
    pub async fn new_and_init() -> Result<Self, Box<dyn std::error::Error>> {
        let run_parameters = RunParameters::try_parse()?;

        let (cmd_tx, cmd_rx) = channel(1);

        let background = BackgroundTask::new(cmd_rx, run_parameters.clone()).await?;
        // `global_seq` and `group_seq` are initialized by 0 at this point since no way to signal to the sync service.
        let mut client = Self {
            cmd_tx,
            run_parameters,
            global_seq: 0,
            group_seq: 0,
        };

        tokio::spawn(background.run());

        client.wait_network_initialized().await?;

        let global_seq_num = client
            // Note that the sdk-go only signals, but not waits.
            .signal_and_wait(
                "initialized_global",
                client.run_parameters.test_instance_count,
            )
            .await?;

        let group_seq_num = client
            // Note that the sdk-go only signals, but not waits.
            .signal_and_wait(
                format!("initialized_group_{}", client.run_parameters.test_group_id),
                client.run_parameters.test_group_instance_count as u64,
            )
            .await?;

        client.record_message(format!(
            "claimed sequence numbers; global={}, group({})={}",
            global_seq_num, client.run_parameters.test_group_id, group_seq_num
        ));

        client.global_seq = global_seq_num;
        client.group_seq = group_seq_num;

        Ok(client)
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
    async fn wait_network_initialized(&self) -> Result<(), Error> {
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
