use futures_channel::mpsc::{UnboundedReceiver as Receiver, UnboundedSender as Sender};
use search_provider::{ResultID, ResultMeta, SearchProvider as SP, SearchProviderImpl};

use super::RUNTIME;
use crate::config;

pub struct SearchProvider {
    sender: Sender<SearchProviderAction>,
}

pub enum SearchProviderAction {
    LaunchSearch(Vec<String>, u32),
    ActivateResult(ResultID),
    InitialResultSet(Vec<String>, futures_channel::oneshot::Sender<Vec<ResultID>>),
    ResultMetas(
        Vec<ResultID>,
        futures_channel::oneshot::Sender<Vec<ResultMeta>>,
    ),
}

impl SearchProvider {
    pub fn new() -> (Self, Receiver<SearchProviderAction>) {
        let (sender, receiver) = futures_channel::mpsc::unbounded();
        (Self { sender }, receiver)
    }
}

impl SearchProviderImpl for SearchProvider {
    fn activate_result(&self, identifier: ResultID, _terms: &[String], _timestamp: u32) {
        let _ = self
            .sender
            .unbounded_send(SearchProviderAction::ActivateResult(identifier));
    }

    fn launch_search(&self, terms: &[String], timestamp: u32) {
        let _ = self
            .sender
            .unbounded_send(SearchProviderAction::LaunchSearch(
                terms.to_owned(),
                timestamp,
            ));
    }

    fn initial_result_set(&self, terms: &[String]) -> Vec<ResultID> {
        let (sender, receiver) = futures_channel::oneshot::channel();
        let _ = self
            .sender
            .unbounded_send(SearchProviderAction::InitialResultSet(
                terms.to_owned(),
                sender,
            ));
        let fut = async { receiver.await.unwrap() };
        futures_executor::block_on(fut)
    }

    fn result_metas(&self, identifiers: &[ResultID]) -> Vec<ResultMeta> {
        let (sender, receiver) = futures_channel::oneshot::channel();
        let _ = self
            .sender
            .unbounded_send(SearchProviderAction::ResultMetas(
                identifiers.to_owned(),
                sender,
            ));
        let fut = async { receiver.await.unwrap() };
        futures_executor::block_on(fut)
    }
}

pub async fn start() -> anyhow::Result<Receiver<SearchProviderAction>> {
    let (search_provider, receiver) = SearchProvider::new();
    let path = config::OBJECT_PATH.to_owned();
    let name = format!("{}.SearchProvider", config::APP_ID);
    RUNTIME.spawn(async move {
        match SP::new(search_provider, name, path).await {
            Ok(_) => {
                tracing::info!("Search provider started");
            }
            Err(err) => {
                tracing::error!("Failed to start search provider {err}");
            }
        }
    });

    Ok(receiver)
}
