use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type")]
#[serde(rename_all = "lowercase")]
pub enum StorageSettings {
    Native(String),
    ClickHouse {
        /// The ClickHouse server connection url.
        url: String,

        /// The database name.
        db: String,

        /// The username used to connect to the database.
        username: String,

        /// The password used to connect to the database.
        password: String,

        /// The full-text-search index path
        index_path: String,

        /// The maximum memory allowed for the in-memory
        /// buffer before committing.
        memory_budget: u64,

        /// The maximum number of sample allowed for the in-memory
        /// buffer before committing
        sample_budget: u64,
    },
}
