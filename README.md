<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# restic-to-influxdb

[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-informational?style=flat-square)](COPYRIGHT.md)

Feed `restic` backup data into InfluxDB and/or a Prometheus node-exporter textfile.

# Usage

```
./restic backup ... --json | ./restic-to-influxdb --user ... --password ... --database ... --host http://localhost:8086
```

For Prometheus, point the converter at a directory mounted into node-exporter's
textfile collector. The file is replaced atomically so Prometheus never sees a
partially written scrape:

```
./restic backup ... --json | ./restic-to-influxdb --prometheus-file /var/lib/node-exporter/textfile/restic_backup.prom
```

```
Usage: restic-to-influxdb [OPTIONS]

Options:
      --dry-run              Enable dry-run mode: don't write to influxdb
  -v, --verbose              Enable verbose mode
  -i, --interval <INTERVAL>  Status interval [default: 10]
  -u, --user <USER>          InfluxDB user
  -p, --password <PASSWORD>  InfluxDB password
  -d, --database <DATABASE>  InfluxDB database
      --host <HOST>          InfluxDB host [default: http://localhost:8086]
      --prometheus-file <PROMETHEUS_FILE>
                             Atomically write Prometheus textfile-collector metrics here
  -h, --help                 Print help
  -V, --version              Print version
```

# Development

```
cat ./sample-lines.txt | cargo run -- --dry-run
```

# License

&copy; 2024 Paul Adenot \<paul@paul.cx\>.

This project is licensed under either of

- [Apache License, Version 2.0](https://www.apache.org/licenses/LICENSE-2.0) ([`LICENSE-APACHE`](LICENSE-APACHE))
- [MIT license](https://opensource.org/licenses/MIT) ([`LICENSE-MIT`](LICENSE-MIT))

at your option.

The [SPDX](https://spdx.dev) license identifier for this project is `MIT OR Apache-2.0`.
