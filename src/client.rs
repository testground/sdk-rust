use std::borrow::Cow;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;

use crate::{
    background::{BackgroundTask, Command},
    errors::Error,
    events::{Event, EventType},
    network_conf::NetworkConfiguration,
    RunParameters,
};

use clap::Parser;

use influxdb::WriteQuery;

use crate::events::LogLine;
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
    run_parameters: RunParameters,
    /// A global sequence number assigned to this test instance by the sync service.
    global_seq: u64,
    /// A group-scoped sequence number assigned to this test instance by the sync service.
    group_seq: u64,
    /// A path to `run.out`.
    run_out: Option<PathBuf>,
}

impl Client {
    pub async fn new_and_init() -> Result<Self, Box<dyn std::error::Error>> {
        let run_parameters: RunParameters = RunParameters::try_parse()?;

        let (cmd_tx, cmd_rx) = channel(1);

        let background = BackgroundTask::new(cmd_rx, run_parameters.clone()).await?;

        let run_out = run_parameters
            .test_outputs_path
            .to_str()
            .map(|path_str| {
                if path_str.is_empty() {
                    None
                } else {
                    let mut path = PathBuf::from(path_str);
                    path.push("run.out");
                    Some(path)
                }
            })
            .unwrap_or(None);

        // `global_seq` and `group_seq` are initialized by 0 at this point since no way to signal to the sync service.
        let mut client = Self {
            cmd_tx,
            run_parameters,
            global_seq: 0,
            group_seq: 0,
            run_out,
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
                client.run_parameters.test_group_instance_count,
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
        message: impl Into<Cow<'static, serde_json::Value>>,
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

    /// ```subscribe``` subscribes to a topic, consuming ordered, elements from
    /// index 0.
    ///
    /// Note that once the capacity of the returned [`Stream`] is reached, the
    /// background task blocks and thus all work related to the [`Client`] will
    /// pause until elements from the [`Stream`] are consumed and thus capacity
    /// is freed. Callers of [`Client::subscribe`] should either set a high
    /// capacity, continuously read from the returned [`Stream`] or drop it.
    ///
    /// ```no_run
    /// # use testground::client::Client;
    /// # let client: Client = todo!();
    /// client.subscribe("my_topic", u16::MAX.into());
    /// ```
    pub async fn subscribe(
        &self,
        topic: impl Into<Cow<'static, str>>,
        capacity: usize,
    ) -> impl Stream<Item = Result<serde_json::Value, Error>> {
        let (stream, out) = mpsc::channel(capacity);

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

        self.write(&event.event);
    }

    pub async fn record_success(self) -> Result<(), Error> {
        let (sender, receiver) = oneshot::channel();

        let cmd = Command::SignalSuccess { sender };

        self.cmd_tx.send(cmd).await.expect(BACKGROUND_RECEIVER);

        receiver.await.expect(BACKGROUND_SENDER)?;

        self.write(&EventType::Success {
            group: self.run_parameters.test_group_id.clone(),
        });

        Ok(())
    }

    pub async fn record_failure(self, error: impl Into<Cow<'static, str>>) -> Result<(), Error> {
        let error = error.into().into_owned();

        let (sender, receiver) = oneshot::channel();

        let cmd = Command::SignalFailure {
            error: error.clone(),
            sender,
        };

        self.cmd_tx.send(cmd).await.expect(BACKGROUND_RECEIVER);

        receiver.await.expect(BACKGROUND_SENDER)?;

        self.write(&EventType::Failure {
            group: self.run_parameters.test_group_id.clone(),
            error,
        });

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
            error: error.clone(),
            stacktrace: stacktrace.clone(),
            sender,
        };

        self.cmd_tx.send(cmd).await.expect(BACKGROUND_RECEIVER);

        receiver.await.expect(BACKGROUND_SENDER)?;

        self.write(&EventType::Crash {
            groups: self.run_parameters.test_group_id.clone(),
            error,
            stacktrace,
        });

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

    /// Returns runtime parameters for this test.
    pub fn run_parameters(&self) -> RunParameters {
        self.run_parameters.clone()
    }

    /// Returns a global sequence number assigned to this test instance.
    pub fn global_seq(&self) -> u64 {
        self.global_seq
    }

    /// Returns a group-scoped sequence number assigned to this test instance.
    pub fn group_seq(&self) -> u64 {
        self.group_seq
    }

    /// Writes an event to `run.out`.
    fn write(&self, event_type: &EventType) {
        if let Some(path) = self.run_out.as_ref() {
            let mut file = match File::options().create(true).append(true).open(path) {
                Ok(file) => file,
                Err(e) => {
                    eprintln!("Failed to open `run.out`: {}", e);
                    return;
                }
            };

            if let Err(e) = writeln!(
                file,
                "{}",
                serde_json::to_string(&LogLine::new(event_type)).expect("Event Serialization")
            ) {
                eprintln!("Failed to write a log to `run.out`: {}", e);
            }
        }
    }
}
