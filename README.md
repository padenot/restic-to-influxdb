<!-- SPDX-License-Identifier: MIT OR Apache-2.0 -->

# restic-to-influxdb

[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-informational?style=flat-square)](COPYRIGHT.md)

Feed `restic` backup data into `influxdb`.

# Usage

```
./restic backup ... --json | ./restic-to-influxdb --user ... --password ... --database ... --host http://localhost:8086
```

```
Usage: restic-to-influxdb [OPTIONS] --user <USER> --password <PASSWORD> --database <DATABASE>

Options:
      --dry-run              Enable dry-run mode: don't write to influxdb
  -v, --verbose              Enable verbose mode
  -i, --interval <INTERVAL>  Status interval [default: 10]
  -u, --user <USER>          InfluxDB user
  -p, --password <PASSWORD>  InfluxDB password
  -d, --database <DATABASE>  InfluxDB database
      --host <HOST>          InfluxDB host [default: http://localhost:8086]
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
