use std::sync::Arc;

use futures::StreamExt;
use futures_util::stream::FuturesOrdered;
use lightning_interfaces::PubSub;
use mysten_metrics::{metered_channel, RegistryService};
use narwhal_config::{Committee, Parameters, WorkerCache};
use narwhal_consensus::{
    bullshark::Bullshark,
    consensus::ConsensusRound,
    metrics::{ChannelMetrics, ConsensusMetrics},
    Consensus,
};
use narwhal_executor::ExecutionState;
use narwhal_node::NodeStorage;
use narwhal_primary::PrimaryChannelMetrics;
use narwhal_types::{
    Certificate, CertificateAPI, CommittedSubDag, ConditionalBroadcastReceiver, ConsensusOutput,
    HeaderAPI, PreSubscribedBroadcastSender,
};
use prometheus::IntGauge;
use tokio::{sync::watch, task::JoinHandle};

use super::pool::BatchPool;
use crate::{consensus::PubSubMsg, execution::Execution};

pub struct EdgeConsensus {
    handles: Vec<JoinHandle<()>>,
    tx_shutdown: PreSubscribedBroadcastSender,
}

impl EdgeConsensus {
    const CHANNEL_CAPACITY: usize = 1000;
    const CONSENSUS_SCHEDULE_CHANGE_SUB_DAGS: u64 = 300;

    pub fn spawn<P: PubSub<PubSubMsg> + 'static>(
        pub_sub: P,
        parameters: Parameters,
        store: &NodeStorage,
        committee: Committee,
        worker_cache: WorkerCache,
        execution: Arc<Execution<P>>,
        registry_service: RegistryService,
    ) -> Self {
        // Collect the handle to each tokio::spawn that happens.
        let mut handles = Vec::with_capacity(3);

        // Some metric stuff. Here we create a new empty registry for metrics since we don't
        // care about them at the moment.
        let registry = registry_service.default_registry();
        let consensus_metrics = Arc::new(ConsensusMetrics::new(&registry));
        let channel_metrics = ChannelMetrics::new(&registry);

        // Create the shutdown channel. Narwhal uses an interesting pre-subscribed broadcast impl.
        // Which only allows creation of a fixed number of subscribers.
        let mut tx_shutdown = PreSubscribedBroadcastSender::new(3);
        let mut shutdown_receivers = tx_shutdown.subscribe_n(3);

        let (tx_consensus_round_updates, _rx_consensus_round_updates) =
            watch::channel(ConsensusRound::new(0, 0));

        let (tx_sequence, rx_sequence) =
            metered_channel::channel(Self::CHANNEL_CAPACITY, &channel_metrics.tx_sequence);

        let new_certificates_counter = IntGauge::new(
            PrimaryChannelMetrics::NAME_NEW_CERTS,
            PrimaryChannelMetrics::DESC_NEW_CERTS,
        )
        .unwrap();
        let (tx_new_certificates, rx_new_certificates) =
            metered_channel::channel(Self::CHANNEL_CAPACITY, &new_certificates_counter);

        let committed_certificates_counter = IntGauge::new(
            PrimaryChannelMetrics::NAME_COMMITTED_CERTS,
            PrimaryChannelMetrics::DESC_COMMITTED_CERTS,
        )
        .unwrap();

        let (tx_committed_certificates, mut rx_committed_certificates) =
            metered_channel::channel(20, &committed_certificates_counter);
        // todo(dalton): we dont need the other end of this reciever so no op it so channel doesnt
        // get full
        tokio::spawn(async move { while rx_committed_certificates.recv().await.is_some() {} });
        let ordering_engine = Bullshark::new(
            committee.clone(),
            store.consensus_store.clone(),
            consensus_metrics.clone(),
            Self::CONSENSUS_SCHEDULE_CHANGE_SUB_DAGS,
        );

        let consensus_handles = Consensus::spawn(
            committee.clone(),
            parameters.gc_depth,
            store.consensus_store.clone(),
            store.certificate_store.clone(),
            shutdown_receivers.pop().unwrap(),
            rx_new_certificates,
            tx_committed_certificates,
            tx_consensus_round_updates,
            tx_sequence,
            ordering_engine,
            consensus_metrics,
        );

        let pool = BatchPool::new(store.batch_store.clone());

        // Get a sub dag generated by consensus and produce [`ConsensusOutput`].
        let consensus_output_producer_handles = tokio::spawn(consensus_output_producer_worker(
            shutdown_receivers.pop().unwrap(),
            rx_sequence,
            pool.clone(),
            execution,
        ));

        // Spawn the event loop that listens for new messages from the pubsub and passes processes
        // them.
        let message_receiver_handles = tokio::spawn(message_receiver_worker(
            committee,
            worker_cache,
            pub_sub,
            shutdown_receivers.pop().unwrap(),
            tx_new_certificates,
            pool,
        ));

        handles.push(consensus_handles);
        handles.push(message_receiver_handles);
        handles.push(consensus_output_producer_handles);

        Self {
            handles,
            tx_shutdown,
        }
    }

    /// Consume this executor and shutdown all of the workers and processes.
    pub async fn shutdown(self) {
        // Send the shutdown signal.
        self.tx_shutdown.send().expect("Failed to send shutdown.");

        // Gracefully wait for all the subtasks to finish and return.
        for handle in self.handles {
            let _ = handle.await;
        }
    }
}

/// Creates and event loop which consumes messages from pubsub and sends them to the
/// right destination.
async fn message_receiver_worker<P: PubSub<PubSubMsg>>(
    committee: Committee,
    worker_cache: WorkerCache,
    mut pub_sub: P,
    mut rx_shutdown: ConditionalBroadcastReceiver,
    tx_new_certificates: metered_channel::Sender<Certificate>,
    pool: BatchPool,
) {
    let handle = |msg: PubSubMsg| async {
        match msg {
            PubSubMsg::Batch(batch) => {
                // TODO(qti3e): The gossip recv should return the originator of the message
                // so we can verify that it is a committee member here.

                // Store the batch. This will wake the interested getters up.
                pool.store(batch);
            },
            PubSubMsg::Certificate(certificate)
                if certificate.verify(&committee, &worker_cache).is_ok() =>
            {
                tx_new_certificates
                    .send(certificate)
                    .await
                    .expect("Failed to send new certificated through the channel.");
            },
            _ => {},
        }
    };

    loop {
        tokio::select! {
            _ = rx_shutdown.receiver.recv() => {
                return;
            },
            Some(msg) = pub_sub.recv() => {
                handle(msg).await;
            }
        }
    }
}

/// The task worker which consumes [`CommittedSubDag`] produced by consensus and feeds them to the
/// output producer.
async fn consensus_output_producer_worker<P: PubSub<PubSubMsg> + 'static>(
    mut rx_shutdown: ConditionalBroadcastReceiver,
    mut rx_sequence: metered_channel::Receiver<CommittedSubDag>,
    pool: BatchPool,
    execution: Arc<Execution<P>>,
) {
    // This queue gets a Future by a push API and will give the resolved futures out
    // in the same order as the insertion happened.
    let mut waiting = FuturesOrdered::new();

    loop {
        tokio::select! {
            _ = rx_shutdown.receiver.recv() => {
                return;
            },
            Some(committed_sub_dag) = rx_sequence.recv() => {
                waiting.push_back(fetch(pool.clone(), committed_sub_dag));
            }
            Some(output) = waiting.next() => {
                execution.handle_consensus_output(output).await;
            }
        }
    }
}

async fn fetch(pool: BatchPool, sub_dag: CommittedSubDag) -> ConsensusOutput {
    // todo(dalton): Lets look into futures::join_all() and make sure nested join_all() isnt crazy
    // memory overhead
    //
    // we are using join_all to put all the requests into the pool at once, and that way if we
    // resort to asking peers for these batches we know all of the ones we need
    let batches = futures::future::join_all(sub_dag.certificates.iter().map(|cert| async {
        // for every certificate in the committed sub dag, try to grab every batch in it
        let cert_batches = futures::future::join_all(
            cert.header()
                .payload()
                .keys()
                .map(|digest| pool.get(*digest)),
        )
        .await;
        (cert.clone(), cert_batches)
    }))
    .await;

    ConsensusOutput {
        sub_dag: Arc::new(sub_dag),
        batches,
    }
}
