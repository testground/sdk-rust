# Changelog
All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.4.0] - unreleased
### Added
- Add `global_seq` and `group_seq` to `Client`, sequence numbers assigned to test instance by the sync service. See [PR 29]

### Change
- Move `RunParameters` to a field on `Client`. See [PR 29].

- Don't wait for network when no sidecar. See [PR 27].

- Make `RunParameters::test_group_instance_count` a `u64` to be consistent with
  `RunParameters::test_instance_count`. See [PR 26].

- Replace `Client::new` with `Client::new_and_init`, waiting for the network to
  initialize, claiming global and group sequence numbers, as well as waiting for
  other instances to do the same. Also makes `Client::wait_network_initialized`
  private, as it is included in `Client::new_and_init` now. See [PR 25].

[PR 26]: https://github.com/testground/sdk-rust/pull/26
[PR 26]: https://github.com/testground/sdk-rust/pull/25
[PR 27]: https://github.com/testground/sdk-rust/pull/27
[PR 29]: https://github.com/testground/sdk-rust/pull/29

## [0.3.0]
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
