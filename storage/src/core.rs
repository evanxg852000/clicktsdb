use std::mem::size_of;

use fasthash::xx;
use serde::{Deserialize, Serialize};

pub const SERIES_NAME_LABEL: &'static str = "__name__";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Label {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sample {
    pub timestamp: i64, // timestamp is in ms format
    pub value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeries {
    id: u64,
    name: String,
    labels: Vec<Label>,
    samples: Vec<Sample>,
    #[serde(skip)]
    size_bytes: u64,
}

impl Default for TimeSeries {
    fn default() -> Self {
        Self {
            id: 0,
            name: "".to_string(),
            labels: vec![],
            samples: vec![],
            size_bytes: 0,
        }
    }
}

impl TimeSeries {
    pub fn new(labels: Vec<Label>, samples: Vec<Sample>) -> Self {
        let size_bytes = {
            let mut mem_size = 0;
            mem_size += labels
                .iter()
                .map(|l| l.name.as_bytes().len() + l.value.as_bytes().len())
                .sum::<usize>();
            mem_size += size_of::<Sample>() + samples.len();
            mem_size as u64
        };
        let info = TimeSeriesInfo::new(labels);
        Self {
            id: info.id,
            name: info.name,
            labels: info.labels,
            samples,
            size_bytes,
        }
    }

    pub fn push(&mut self, sample: Sample) {
        self.size_bytes += size_of::<Sample>() as u64;
        self.samples.push(sample);
    }

    pub fn extend(&mut self, samples: Vec<Sample>) {
        self.size_bytes += (size_of::<Sample>() * samples.len()) as u64;
        self.samples.extend(samples.into_iter());
    }

    pub fn get_id(&self) -> u64 {
        self.id
    }

    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_labels(&self) -> &[Label] {
        &self.labels
    }

    pub fn get_samples(&self) -> &[Sample] {
        &self.samples
    }

    pub fn get_size_bytes(&self) -> u64 {
        self.size_bytes
    }

    pub fn info(&self)-> TimeSeriesInfo {
        TimeSeriesInfo{
            id: self.id,
            name: self.name.clone(),
            long_name: "".to_string(),
            labels: self.labels.clone(),
        }
    }

    pub fn into_raw(self) -> (Vec<Label>, Vec<Sample>) {
        (self.labels, self.samples)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeSeriesInfo {
    pub id: u64,
    pub name: String,
    pub long_name: String,
    pub labels: Vec<Label>,
}

impl TimeSeriesInfo {
    pub fn new(labels: Vec<Label>) -> Self {
        let name = labels
            .iter()
            .find(|l| l.name == SERIES_NAME_LABEL)
            .map(|l| l.value.clone())
            .expect(&format!(
                "time series should have a `{}` label.",
                SERIES_NAME_LABEL
            ));

        // sort the labels, append __name__ & hash
        let mut label_set: Vec<String> = labels
            .iter()
            .map(|l| format!("{}_{}", l.name, l.value))
            .collect();
        label_set.sort();

        let long_name = label_set.join("-");

        Self {
            id: xx::hash64(long_name.as_bytes()),
            name,
            long_name,
            labels,
        }
    }
}
