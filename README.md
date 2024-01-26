# clicktsdb

A time-series database backed by ClickHouse that can serve as prometheus remote storage.

I built this as part of my projects at [recurse center W2'24](https://www.recurse.com/) not fully complete but it's in a state I feel like I will not learn new things unless I implement new features or decide to make it production grade (make it highly performant). I believe I can do this later after the recurse batch, so let me move on to something more challenging. 

Features:

- Prometheus remote storage
- Basic promQL API 
- prometheus-matchers
- Data Ingestion using influxDB line protocol
- Include a purposefully built full-text library
