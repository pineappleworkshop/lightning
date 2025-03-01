use affair::{Executor, TokioSpawn};
use anyhow::Result;
use async_trait::async_trait;
use lightning_interfaces::{
    application::{ApplicationInterface, ExecutionEngineSocket},
    common::WithStartAndShutdown,
    config::ConfigConsumer,
};

use crate::{
    config::Config,
    env::{Env, UpdateWorker},
    query_runner::QueryRunner,
};

pub struct Application {
    update_socket: ExecutionEngineSocket,
    query_runner: QueryRunner,
}

#[async_trait]
impl WithStartAndShutdown for Application {
    /// Returns true if this system is running or not.
    fn is_running(&self) -> bool {
        true
    }

    /// Start the system, should not do anything if the system is already
    /// started.
    async fn start(&self) {
        // No op because application is started in the init
    }

    /// Send the shutdown signal to the system.
    async fn shutdown(&self) {}
}

impl ConfigConsumer for Application {
    const KEY: &'static str = "application";

    type Config = Config;
}

#[async_trait]
impl ApplicationInterface for Application {
    /// The type for the sync query executor.
    type SyncExecutor = QueryRunner;

    /// Create a new instance of the application layer using the provided configuration.
    async fn init(config: Self::Config) -> Result<Self> {
        let mut env = Env::new();
        env.genesis(config);
        Ok(Self {
            query_runner: env.query_runner(),
            update_socket: TokioSpawn::spawn(UpdateWorker::new(env)),
        })
    }

    /// Returns a socket that should be used to submit transactions to be executed
    /// by the application layer.
    ///
    /// # Safety
    ///
    /// See the safety document for the [`ExecutionEngineSocket`].
    fn transaction_executor(&self) -> ExecutionEngineSocket {
        self.update_socket.clone()
    }

    /// Returns the instance of a sync query runner which can be used to run queries without
    /// blocking or awaiting. A naive (& blocking) implementation can achieve this by simply
    /// putting the entire application state in an `Arc<RwLock<T>>`, but that is not optimal
    /// and is the reason why we have `Atomo` to allow us to have the same kind of behavior
    /// without slowing down the system.
    fn sync_query(&self) -> Self::SyncExecutor {
        self.query_runner.clone()
    }
}
