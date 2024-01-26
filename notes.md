


fts: https://www.youtube.com/watch?v=NIL677uIaOs


https://github.com/mindis/prom2click
https://github.com/jiacai2050/prom-remote-api
https://github.com/jamessanford/remote-tsdb-clickhouse

https://www.youtube.com/watch?v=p9qjb_yoBro

https://altinity.com/wp-content/uploads/2021/11/How-ClickHouse-Inspired-Us-to-Build-a-High-Performance-Time-Series-Database.pdf

https://www.youtube.com/watch?v=EfIlRXVyfZM

Prometheus-Internals:
- https://uzxmx.github.io/prometheus-tsdb-internals.html
- https://ganeshvernekar.com/blog/prometheus-tsdb-the-head-block


Also extend to AWS-S3 filtering

```SQL
CREATE TABLE metrics.samples
(
    date Date DEFAULT toDate(0),
    name String,
    tags Array(String),
    value Float64 CODEC(Gorilla, LZ4),
    timestamp DateTime CODEC(DoubleDelta, LZ4),   
)
ENGINE = MergeTree
ORDER BY (date, name, tags, timestamp)
SETTINGS index_granularity = 8192
```


## Schema

```SQL
CREATE TABLE IF NOT EXISTS metric_to_series (
    metric_name_with_labels String, 
    series_id Uint64
)
ENGINE = MergeTree
ORDER BY (metric_name_with_labels, labels)

CREATE TABLE IF NOT EXISTS series_to_metric (
    series_id Uint64, 
    metric_name_with_labels String
) 
ENGINE = MergeTree
ORDER BY (series_id)

CREATE TABLE IF NOT EXISTS label_to_series (
    label_name_value LowCardinality(String), 
    series_id Uint64
) 
ENGINE = MergeTree
ORDER BY (label_name_value, series_id)

CREATE TABLE IF NOT EXISTS samples (
    series_id Uint64, 
    timestamp Int64 Codec(DoubleDelta, LZ4), 
    value Float64 Codec(Gorilla, LZ4)
) 
ENGINE = MergeTree
ORDER BY (series_id, timestamp)
```

FST
AXUM

cpu,host=A,region=west usage_system=64.2 1590488773254420000

https://medium.com/@cuteberry.madhu/python-script-to-push-sample-data-to-influxdb-87935c9bbd2



* -> match_all
term:* term_match_all
term:~regex
term:!~regex
term:%contains
term:2%levenstein
term:?startwith
term:equal

AND, OR
