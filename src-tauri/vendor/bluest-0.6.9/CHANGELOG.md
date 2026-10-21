# Change Log

## 0.6.9

- (MacOS/iOS) Update to `objc2` crates
- (MacOS/iOS) Add preliminary L2CAP support
- (iOS) Fix missing `device_connection_event` notifications

## 0.6.8

- (Windows) Support Windows 10 LTSC 2019
- (Linux) Implement std::fmt::Display for DeviceId
- (MacOS/iOS) Fix dropped notifications when processing is slow

## 0.6.7

- Fix a panic on CoreBluetooth when parent object references become `nil`
- Add `unstable` and `l2cap` features
- (unstable) Add minimal Android platform support
- (unstable) Add support for L2CAP connection-oriented channels (Android only)

## 0.6.6

- Fix compilation on iOS

## 0.6.5

- Fix another segmentation violation on MacOS/iOS introduced in v0.6.1

## 0.6.4

- Fix a segmentation violation on MacOS/iOS introduced in v0.6.1

## 0.6.3

- Add `ServicesChanged::was_invalidated` method to check if a particular service
  was invalidated by a service changed indication

## 0.6.2

- Add `Device::service_changed_indications` to get a stream of service changed
  indications from the device

## 0.6.1

- Fix a bug on MacOS/iOS where the event channels could be closed prematurely

## 0.6.0

- `Characteristic::write_without_response` now returns a `Result<()>`
- Added `Send + Unpin` constraints to returned `impl Stream` values
- Reduced the number and weight of dependencies
- Removed static `bluer` session on Linux. Each `Adapter` now starts its own
  session.

## 0.5.7

- Fix Windows compilation error

## 0.5.6

- Added `Characteristic::max_write_len()` to get the maximum data that can be
  written to the characteristic in a single packet (this is 3 bytes less than
  the negotiated MTU for the connection; this is a method on the
  `Characteristic` instead of the `Device` because Linux only exposes this value
  on characteristics).
- Added limited support for `device_connection_events` on MacOS
- Tightened the behavior of the non-discover counterparts of discovery methods
  (i.e. `Device::services()`, `Service::characteristics()`,
  `Service::included_characteristics()`, `Characteristic::descriptors()`) to
  always perform discovery if discovery has not previously been performed.
- Fix docs.rs example scraping

## 0.5.5

- Fix docs.rs build

## 0.5.4

- Add `Adapter::device_connection_events`

## 0.5.3

- Add support for unpairing devices on Windows/Linux

## 0.5.2

- (MacOS/iOS) Fix FFI memory issues

## 0.5.1

- (Linux) Don't return connected devices from `Adapter::scan`
- (Linux) Skip attempt to pair if the devices is already paired
- (Windows) Let the OS filter the scan results for us

## 0.5.0

- `Device::name` now returns a `Result`
- `Characteristic::properties` is now async
- Added `_async` APIs to all platforms for methods which are sync on some
  platforms and async on others
- Added support for pairing to devices

## 0.4.0

- Breaking change: take `id` by reference in `Adapter::open_device`
- Improve CoreBluetooth error handling on disconnection
- Remove dependency on the Tokio runtime on non-Linux platforms
- Bug fixes

## 0.3.4

- Add `Adapter::discover_devices`
- Add reconnect example

## 0.3.3

- Reduce dependencies

## 0.3.2

- Add CI
- Add scraped examples to docs
- Fix a doctest error

## 0.3.1

- Change `AttError` from an enum to a newtype wrapper around a `u8` with defined
  constants for the known error codes

## 0.3.0

- Add Linux implementation
- Add connected example
- Add `Adapter::connected_devices`
- Add `Adapter::connected_devices_with_services`
- Add re-export of the `Uuid` struct
- Change `Characteristic::properties` to return a `CharacteristicProperties`
  struct
- Change `SmallVec` to `Vec` in all APIs
- Split discovery APIs into separate `discover_x()` and `discover_x_with_uuid()`
  methods.
- Remove `smallvec` and `uuid` re-exports

## 0.2.0

Initial release
