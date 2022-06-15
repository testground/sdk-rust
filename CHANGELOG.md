# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.3.0] unreleased
### Added

- Add `RunParameters::data_network_ip` for ease of finding the IP within the data network assigned to the instance. See [PR 22].

### Change
- Change `RunParameters::test_instance_params` from `String` to `HashMap` which contains key-value pairs from parsing the parameter string. See [PR 19].

[PR 19]: https://github.com/testground/sdk-rust/pull/19
[PR 22]: https://github.com/testground/sdk-rust/pull/22

## [0.2.0]
### Added
- Add  PubSub, Network Shaping & Metrics. See [PR 6].

### Change
- Take ownership of `Client` when signaling success / failure. See [PR 7].

### Fixed
- Make events payload compatible with the go-sdk. See [PR 14]

[PR 6]: https://github.com/testground/sdk-rust/pull/6
[PR 7]: https://github.com/testground/sdk-rust/pull/7
[PR 14]: https://github.com/testground/sdk-rust/pull/14

## [0.1.1]
### Added
- Add `Client::publish_success` to signal instance success to daemon and sync service. See [PR 5].

[PR 5]: https://github.com/testground/sdk-rust/pull/5

## [0.1.0] - 2022-01-24
### Added
- Add initial scaffolding with basic synchronization client. See [PR 1].

[PR 1]: https://github.com/testground/sdk-rust/pull/1
