use std::{collections::HashMap, sync::Arc, time::Duration};

use tokio::sync::Semaphore;

use crate::{
    dal::Store,
    jobs::{
        art_process::ArtProcessJobHandler,
        cue_split::CueSplitJobHandler,
        fingerprint::FingerprintJobHandler,
        freedb_lookup::FreedBLookupJobHandler,
        maintenance::MaintenanceJobHandler,
        mb_lookup::MbLookupJobHandler,
        normalize::NormalizeJobHandler,
        organize::OrganizeJobHandler,
        process_staged::ProcessStagedJobHandler,
        scan::ScanJobHandler,
        transcode::TranscodeJobHandler,
        virtual_sync::VirtualSyncJobHandler,
        JobHandler,
    },
    services::{freedb::FreedBService, musicbrainz::MusicBrainzService},
};

const DEFAULT_SCAN_CONCURRENCY: usize = 2;
const DEFAULT_OTHER_CONCURRENCY: usize = 4;
const POLL_INTERVAL_SECS: u64 = 5;

pub struct Scheduler {
    db: Arc<dyn Store>,
    handlers: HashMap<&'static str, Arc<dyn JobHandler>>,
    semaphores: HashMap<&'static str, Arc<Semaphore>>,
}

impl Scheduler {
    pub fn new(db: Arc<dyn Store>, mb_service: Arc<MusicBrainzService>, freedb_service: Arc<FreedBService>) -> Self {
        let mut handlers: HashMap<&'static str, Arc<dyn JobHandler>> = HashMap::new();
        handlers.insert("scan", Arc::new(ScanJobHandler));
        handlers.insert("fingerprint", Arc::new(FingerprintJobHandler));
        handlers.insert("organize", Arc::new(OrganizeJobHandler));
        handlers.insert("mb_lookup", Arc::new(MbLookupJobHandler::new(mb_service)));
        handlers.insert("freedb_lookup", Arc::new(FreedBLookupJobHandler::new(freedb_service)));
        handlers.insert("cue_split", Arc::new(CueSplitJobHandler::new(db.clone())));
        handlers.insert("transcode", Arc::new(TranscodeJobHandler::new(db.clone())));
        handlers.insert("art_process", Arc::new(ArtProcessJobHandler::new(db.clone())));
        handlers.insert("normalize", Arc::new(NormalizeJobHandler::new(db.clone())));
        handlers.insert("process_staged", Arc::new(ProcessStagedJobHandler::new(db.clone())));
        handlers.insert("virtual_sync", Arc::new(VirtualSyncJobHandler::new(db.clone())));
        handlers.insert("maintenance", Arc::new(MaintenanceJobHandler));

        let mut semaphores: HashMap<&'static str, Arc<Semaphore>> = HashMap::new();
        semaphores.insert("scan",          Arc::new(Semaphore::new(DEFAULT_SCAN_CONCURRENCY)));
        semaphores.insert("fingerprint",   Arc::new(Semaphore::new(DEFAULT_OTHER_CONCURRENCY)));
        semaphores.insert("mb_lookup",     Arc::new(Semaphore::new(DEFAULT_OTHER_CONCURRENCY)));
        semaphores.insert("freedb_lookup", Arc::new(Semaphore::new(DEFAULT_OTHER_CONCURRENCY)));
        semaphores.insert("transcode",     Arc::new(Semaphore::new(2)));
        semaphores.insert("art_process",   Arc::new(Semaphore::new(DEFAULT_OTHER_CONCURRENCY)));
        semaphores.insert("organize",      Arc::new(Semaphore::new(DEFAULT_OTHER_CONCURRENCY)));
        semaphores.insert("cue_split",     Arc::new(Semaphore::new(2)));
        semaphores.insert("normalize",       Arc::new(Semaphore::new(2)));
        semaphores.insert("process_staged", Arc::new(Semaphore::new(2)));
        semaphores.insert("virtual_sync",   Arc::new(Semaphore::new(1)));
        semaphores.insert("maintenance",     Arc::new(Semaphore::new(1)));

        Self { db, handlers, semaphores }
    }

    /// Run the scheduler poll loop indefinitely. Call via `tokio::spawn`.
    pub async fn run(self: Arc<Self>) {
        let job_types: Vec<&str> = self.handlers.keys().copied().collect();

        loop {
            let result = self.db.claim_next_job(&job_types).await;

            match result {
                Err(e) => {
                    tracing::error!(error = %e, "error claiming job");
                    tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECS)).await;
                }
                Ok(None) => {
                    tokio::time::sleep(Duration::from_secs(POLL_INTERVAL_SECS)).await;
                }
                Ok(Some(job)) => {
                    let Some(handler) = self.handlers.get(job.job_type.as_str()).cloned() else {
                        tracing::warn!(job_type = %job.job_type, "no handler for job type");
                        let _ = self.db.fail_job(job.id, "no handler registered").await;
                        continue;
                    };

                    let semaphore = self
                        .semaphores
                        .get(job.job_type.as_str())
                        .cloned()
                        .unwrap_or_else(|| Arc::new(Semaphore::new(1)));

                    let db = self.db.clone();

                    tokio::spawn(async move {
                        let _permit = semaphore.acquire().await
                            .expect("job semaphore unexpectedly closed");

                        tracing::info!(job_id = job.id, job_type = %job.job_type, "running job");

                        match handler.run(db.clone(), job.payload.clone()).await {
                            Ok(result) => {
                                if let Err(e) = db.complete_job(job.id, result).await {
                                    tracing::error!(job_id = job.id, error = %e, "failed to mark job complete");
                                }
                            }
                            Err(e) => {
                                tracing::warn!(job_id = job.id, error = %e, "job failed");
                                if let Err(db_err) = db.fail_job(job.id, &e.to_string()).await {
                                    tracing::error!(job_id = job.id, error = %db_err, "failed to mark job failed");
                                }
                            }
                        }
                    });
                }
            }
        }
    }
}
