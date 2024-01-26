use std::{path::Path, time::{Duration, Instant}};

use clickhouse::{Client, Row};
use derivative::Derivative;
use fts::{query::Query, Config, Document, Index, IndexReader, IndexWriter};
use hashbrown::HashMap;
use serde::{Deserialize, Serialize};
use tokio::{
    sync::mpsc::{self, Sender},
    task::JoinHandle,
};

use crate::{
    error::{StorageError, StorageResult}, Label, Sample, TimeSeries
};

const DDL_SQL: &'static str = r#"
CREATE TABLE IF NOT EXISTS samples (
    series_id UInt64, 
    timestamp Int64 Codec(DoubleDelta, LZ4), 
    value Float64 Codec(Gorilla, LZ4)
)
ENGINE = MergeTree
PARTITION BY toStartOfWeek(toDateTime64(timestamp, 3))
ORDER BY (series_id, timestamp);
"#;

const SELECT_SQL: &'static str = r#"
SELECT * FROM samples 
WHERE series_id IN (?) AND timestamp >= ? AND timestamp < ?"#; 

const DELETE_SQL: &'static str = r#"
ALTER TABLE samples DELETE 
    WHERE toStartOfWeek(toDateTime64(timestamp, 3)) < toStartOfWeek(toDateTime64(?, 3)))
    AND timestamp >= ? AND timestamp < ?;
"#;

const SAMPLES_TABLE_NAME: &'static str = "samples";

#[derive(Clone)]
pub struct ClickHouseClient {
    client: Client,
}

impl ClickHouseClient {
    pub fn new(url: &str, db: &str, user: &str, password: &str) -> Self {
        Self {
            client: Client::default()
                .with_url(url)
                .with_database(db)
                .with_user(user)
                .with_password(password),
        }
    }

    pub async fn migrate(&self) -> StorageResult<()> {
        self.client
            .query(DDL_SQL)
            .execute()
            .await
            .map_err(StorageError::from)
    }

    pub async fn insert(&self, time_series: Vec<TimeSeries>) -> StorageResult<()> {
        let mut batch = self.client.insert(SAMPLES_TABLE_NAME)?;
        for series in time_series {
            let series_id = series.get_id();
            for sample in series.get_samples() {
                let row = SampleRow {
                    series_id,
                    timestamp: sample.timestamp,
                    value: sample.value,
                };
                // Not a big fan of await in loops. Internally most fustures
                // would return immediately when buffer is not full but still.
                batch.write(&row).await?;
            }
        }
        batch.end().await?;
        Ok(())
    }

    pub async fn select(
        &self,
        series_ids: Vec<u64>,
        start_timestamp: i64,
        end_timestamp: i64,
    ) -> StorageResult<Vec<SampleRow>> {
        let mut cursor = self
            .client
            .query(SELECT_SQL)
            .bind(series_ids)
            .bind(start_timestamp)
            .bind(end_timestamp)
            .fetch::<SampleRow>()?;
        let mut samples = vec![];
        while let Some(sample) = cursor.next().await? {
            samples.push(sample);
        }
        Ok(samples)
    }

    /// Remove samples older than specified timestamp
    /// Internally, this will discard all CH parts older than the
    /// the specified timestamp converted into part naming scheme (weekly).
    pub async fn truncate(&self, timestamp: i64) -> StorageResult<()> {
        self.client
            .query(DELETE_SQL)
            .bind(timestamp)
            .execute()
            .await?;
        Ok(())
    }
}

#[derive(Debug, Clone, Row, Serialize, Deserialize)]
pub struct SampleRow {
    series_id: u64,
    timestamp: i64,
    value: f64,
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct ClickHouseStorageInner {
    memory: HashMap<u64, TimeSeries>,
    #[derivative(Debug = "ignore")]
    client: ClickHouseClient,
    // indexer:
    sample_size: u64,
}

#[derive(Derivative)]
#[derivative(Debug)]
pub struct ClickHouseStorage {
    #[derivative(Debug = "ignore")]
    client: ClickHouseClient,
    sender: Sender<Vec<TimeSeries>>,
    #[derivative(Debug = "ignore")]
    index: Index,
    handle: JoinHandle<StorageResult<()>>,
    memory_budget: u64, // Max allowed memory consumption by in-memory buffer before commit.
    sample_budget: u64, // Max allowed number of sample in buffer before commit.
}

impl ClickHouseStorage {
    pub fn new(
        url: &str,
        db: &str,
        username: &str,
        password: &str,
        index_path: &str,
        memory_budget: u64,
        sample_budget: u64,
    ) -> StorageResult<Self> {
        let client = ClickHouseClient::new(url, db, username, password);
        let click_house_client = client.clone();

        let index = Index::open(Config::new(&Path::new(index_path)))?;
        let index_writer = index.writer();

        let (sender, mut receiver) = mpsc::channel(50);
        let task = tokio::spawn(async move {
            let mut memory_usage = 0u64;
            let mut sample_count = 0u64;
            let mut memory_buffer: HashMap<u64, TimeSeries> = HashMap::new();
            let mut interval = tokio::time::interval(Duration::from_secs(2));
            let result = click_house_client.migrate().await;
            if let Err(err) = result {
                println!("Clickhouse migration error {:?}", err);
            };
            
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        handle_commit_ticker(
                            &index_writer,
                            &click_house_client,
                            &mut memory_buffer,
                            &mut memory_usage,
                            &mut sample_count,
                            memory_budget, sample_budget
                        ).await?;
                    }
                    message = receiver.recv() => {
                        match message {
                            Some(time_series) => handle_received_series(&index_writer, &mut memory_buffer, &mut memory_usage, &mut sample_count, time_series),
                            _ => break  // sender has been dropped
                        }
                    }
                };
            }
            StorageResult::Ok(())
        });

        Ok(Self {
            client,
            sender,
            index,
            handle: task,
            memory_budget,
            sample_budget,
        })
    }

    pub async fn write(&self, series: Vec<TimeSeries>) -> StorageResult<()> {
        // put in a fts index,
        // buffer until full or commit time elapsed
        // commit it by storing inside clickhouse
        self.sender
            .send(series)
            .await
            .map_err(|_| StorageError::Other("tokio send error".to_string()))
    }

    pub async fn read(
        &self,
        query: Query,
        start_timestamp: i64,
        end_timestamp: i64,
    ) -> StorageResult<Vec<TimeSeries>> {
        let now = Instant::now();
        let index_reader = self.index.reader();
        let series_ids = index_reader.query(query)?;
        //TODO: improve series grouping (maybe do it in clickhouse)
        let mut timeseries_map: HashMap<u64, TimeSeries> = self.fetch_docs(&index_reader, &series_ids)?;
        let rows = self.client
            .clone()
            .select(series_ids, start_timestamp, end_timestamp)
            .await?;
        let num_rows = rows.len();
        for row in rows {
            if let Some(entry) = timeseries_map.get_mut(&row.series_id) {
                entry.push(Sample{timestamp: row.timestamp, value: row.value});
            }
        }

        let timeseries = timeseries_map.into_iter()
            .map(|(_, v)| v)
            .collect();

        let elapsed = now.elapsed();
        println!("Selected `{}` samples in `{:.2?}`.", num_rows, elapsed);
        Ok(timeseries)
    }

    pub async fn truncate(&self, timestamp: i64) -> StorageResult<()> {
        self.client.clone().truncate(timestamp).await
    }

    fn fetch_docs(&self,index_reader: &IndexReader, series_ids: &[u64]) -> StorageResult<HashMap<u64, TimeSeries>> {
        let mut map = HashMap::with_capacity(series_ids.len());
        for id in series_ids {
            //TODO: check cache
            let doc_data = index_reader.fetch_doc(*id).unwrap();
            let labels: Vec<Label> = serde_json::from_slice(&doc_data).unwrap();
            map.insert(*id, TimeSeries::new(labels, vec![]));
        }
        Ok(map)
    }
}

async fn handle_commit_ticker(
    index_writer: &IndexWriter,
    click_house_client: &ClickHouseClient,
    memory_buffer: &mut HashMap<u64, TimeSeries>,
    memory_usage: &mut u64,
    sample_count: &mut u64,
    memory_budget: u64,
    sample_budget: u64,
) -> StorageResult<()> {
    if *memory_usage >= memory_budget || *sample_count >= sample_budget {
        // commit
        let committing_buffer =
            std::mem::replace(&mut *memory_buffer, HashMap::<u64, TimeSeries>::new());
        let series = committing_buffer
            .into_iter()
            .map(|(_, v)| v)
            .collect::<Vec<_>>();
        let result = click_house_client.insert(series).await;
        if let Err(err) = result {
            println!("ClickHouse insertion error {:?}", err);
        };

        if let Err(err) = index_writer.commit(true) {
            println!("Fts index commit error {:?}", err);
        }
        *memory_usage = 0;
        *sample_count = 0;

        println!("ClickTSDB buffer committed!")
    }
    Ok(())
}

fn handle_received_series(
    index_writer: &IndexWriter,
    memory_buffer: &mut HashMap<u64, TimeSeries>,
    memory_usage: &mut u64,
    sample_count: &mut u64,
    time_series: Vec<TimeSeries>,
) {
    for series in time_series {
        *memory_usage += series.get_size_bytes();
        *sample_count += series.get_samples().len() as u64;

        if let Some(entry) = memory_buffer.get_mut(&series.get_id()) {
            let (_, samples) = series.into_raw();
            entry.extend(samples);
        } else {
            let labels = series.get_labels().iter().cloned().collect::<Vec<_>>();
            let series_id = series.get_id();
            let doc_content = serde_json::to_string(&labels).unwrap();

            let terms = labels
                .iter()
                .map(|l| format!("{}:{}", l.name, l.value))
                .collect::<Vec<_>>();
            index_writer.insert_doc(Document::new(series_id, doc_content.as_str(), &terms));

            memory_buffer.insert(series_id, series);
        }
    }
}
