use anyhow::{Context, Result};
use std::{path::PathBuf, sync::Arc};
use tokio::sync::{mpsc, Mutex};

#[derive(Debug, Clone)]
pub struct DownloadTask {
    pub url: String,
    pub destination: PathBuf,
}
#[derive(Debug, Clone)]
pub enum DownloadEvent {
    Started(PathBuf),
    Progress { path: PathBuf, received: u64, total: Option<u64> },
    Finished(PathBuf),
    Failed { path: PathBuf, error: String },
}
#[derive(Clone)]
pub struct DownloadQueue {
    tx: mpsc::Sender<DownloadTask>,
}
impl DownloadQueue {
    pub fn start(concurrency: usize, events: mpsc::UnboundedSender<DownloadEvent>) -> Self {
        let (tx, rx) = mpsc::channel::<DownloadTask>(128);
        let shared = Arc::new(Mutex::new(rx));
        let workers = concurrency.max(1).min(16);
        for _ in 0..workers {
            let shared = shared.clone();
            let events = events.clone();
            tokio::spawn(async move {
                loop {
                    let task = {
                        let mut guard = shared.lock().await;
                        guard.recv().await
                    };
                    let Some(task) = task else { break };
                    let _ = events.send(DownloadEvent::Started(task.destination.clone()));
                    let result = download_one(&task, &events).await;
                    match result {
                        Ok(()) => { let _ = events.send(DownloadEvent::Finished(task.destination)); }
                        Err(e) => { let _ = events.send(DownloadEvent::Failed { path: task.destination, error: format!("{e:#}") }); }
                    }
                }
            });
        }
        Self { tx }
    }
    pub async fn enqueue(&self, task: DownloadTask) -> Result<()> {
        self.tx.send(task).await.context("Черга завантажень закрита")
    }
}
async fn download_one(task: &DownloadTask, events: &mpsc::UnboundedSender<DownloadEvent>) -> Result<()> {
    if let Some(parent) = task.destination.parent() { tokio::fs::create_dir_all(parent).await?; }
    let client = reqwest::Client::builder().user_agent("PLauncher/1.4.0").build()?;
    let mut response = client.get(&task.url).send().await?.error_for_status()?;
    let total = response.content_length();
    let tmp = task.destination.with_extension("plauncher-download");
    let mut file = tokio::fs::File::create(&tmp).await?;
    let mut received = 0u64;
    while let Some(chunk) = response.chunk().await? {
        tokio::io::AsyncWriteExt::write_all(&mut file, &chunk).await?;
        received += chunk.len() as u64;
        let _ = events.send(DownloadEvent::Progress { path: task.destination.clone(), received, total });
    }
    tokio::io::AsyncWriteExt::flush(&mut file).await?;
    drop(file);
    if tokio::fs::try_exists(&task.destination).await.unwrap_or(false) {
        tokio::fs::remove_file(&task.destination).await?;
    }
    tokio::fs::rename(&tmp, &task.destination).await?;
    Ok(())
}
