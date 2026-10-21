[![crates.io][crates-badge]][crates-url] [![docs.rs][docs-badge]][docs-url]
[![Build Status][actions-badge]][actions-url]

[crates-badge]: https://img.shields.io/crates/v/bluest
[crates-url]: https://crates.io/crates/bluest
[docs-badge]: https://docs.rs/bluest/badge.svg
[docs-url]: https://docs.rs/bluest
[actions-badge]: https://github.com/alexmoon/bluest/workflows/CI/badge.svg
[actions-url]: https://github.com/alexmoon/bluest/actions?query=workflow%3ACI+branch%3Amain

# Bluest — Cross-platform Bluetooth LE crate for Rust

<!-- cargo-rdme start -->

Bluest is a cross-platform [Bluetooth Low Energy] (BLE) library for [Rust]. It
currently supports Windows (version 10 and later), MacOS/iOS, and Linux. Android
support is planned.

The goal of Bluest is to create a _thin_ abstraction on top of the
platform-specific Bluetooth APIs in order to provide safe, cross-platform access
to Bluetooth LE devices. The crate currently supports the GAP Central and GATT
Client roles. Peripheral and Server roles are not supported.

[Rust]: https://www.rust-lang.org/
[Bluetooth Low Energy]: https://www.bluetooth.com/specifications/specs/

## Usage

```rust
let adapter = Adapter::default().await.ok_or("Bluetooth adapter not found")?;
adapter.wait_available().await?;

println!("starting scan");
let mut scan = adapter.scan(&[]).await?;
println!("scan started");
while let Some(discovered_device) = scan.next().await {
   println!(
       "{}{}: {:?}",
       discovered_device.device.name().as_deref().unwrap_or("(unknown)"),
       discovered_device
           .rssi
           .map(|x| format!(" ({}dBm)", x))
           .unwrap_or_default(),
       discovered_device.adv_data.services
   );
}
```

## Overview

The primary functions provided by Bluest are:

- Device discovery:
  - [Scanning][Adapter::scan] for devices and receiving advertisements
  - Finding [connected devices][Adapter::connected_devices]
  - [Opening][Adapter::open_device] previously found devices
  - [Connecting][Adapter::connect_device] to discovered devices
  - [Pairing][Device::pair] with devices
- Accessing remote GATT services:
  - Discovering device [services][Device::discover_services]
  - Discovering service [characteristics][Service::discover_characteristics]
  - Discovering characteristic
    [descriptors][Characteristic::discover_descriptors]
  - [Read][Characteristic::read], [write][Characteristic::write] (including
    [write without response][Characteristic::write_without_response]), and
    [notify/indicate][Characteristic::notify] operations on remote
    characteristics
  - [Read][Descriptor::read] and [write][Descriptor::write] operations on
    characteristic descriptors

## Asynchronous runtimes

On non-linux platforms, Bluest should work with any asynchronous runtime. On
linux the underlying `bluer` crate requires the Tokio runtime and Bluest makes
use of Tokio's `block_in_place` API (which requires Tokio's multi-threaded
runtime) to make a few methods synchronous. Linux-only asynchronous versions of
those methods are also provided, which should be preferred in platform-specific
code.

## Platform specifics

Because Bluest aims to provide a thin abstraction over the platform-specific
APIs, the available APIs represent the lowest common denominator of APIs among
the supported platforms. For example, CoreBluetooth never exposes the Bluetooth
address of devices to applications, therefore there is no method on `Device` for
retrieving an address or even any Bluetooth address struct in the crate.

Most Bluest APIs should behave consistently across all supported platforms.
Those APIs with significant differences in behavior are summarized in the table
below.

| Method                                                           | MacOS/iOS | Windows | Linux |
| ---------------------------------------------------------------- | :-------: | :-----: | :---: |
| [`Adapter::connect_device`][Adapter::connect_device]             |    ✅     |   ✨    |  ✅   |
| [`Adapter::disconnect_device`][Adapter::disconnect_device]       |    ✅     |   ✨    |  ✅   |
| [`Device::name`][Device::name]                                   |    ✅     |   ✅    |  ⌛️   |
| [`Device::is_paired`][Device::is_paired]                         |    ❌     |   ✅    |  ✅   |
| [`Device::pair`][Device::pair]                                   |    ✨     |   ✅    |  ✅   |
| [`Device::pair_with_agent`][Device::pair_with_agent]             |    ✨     |   ✅    |  ✅   |
| [`Device::unpair`][Device::unpair]                               |    ❌     |   ✅    |  ✅   |
| [`Device::rssi`][Device::rssi]                                   |    ✅     |   ❌    |  ❌   |
| [`Service::uuid`][Service::uuid]                                 |    ✅     |   ✅    |  ⌛️   |
| [`Service::is_primary`][Service::is_primary]                     |    ✅     |   ❌    |  ✅   |
| [`Characteristic::uuid`][Characteristic::uuid]                   |    ✅     |   ✅    |  ⌛️   |
| [`Characteristic::max_write_len`][Characteristic::max_write_len] |    ✅     |   ✅    |  ⌛️   |
| [`Descriptor::uuid`][Descriptor::uuid]                           |    ✅     |   ✅    |  ⌛️   |

✅ = supported\
✨ = managed automatically by the OS, this method is a no-op\
⌛️ = the underlying API is async so this method uses Tokio's `block_in_place`
API internally\
❌ = returns a [`NotSupported`][error::ErrorKind::NotSupported] error

Also, the errors returned by APIs in a given situation may not be consistent
from platform to platform. For example, Linux's bluez API does not return the
underlying Bluetooth protocol error in a useful way, whereas the other platforms
do. Where it is possible to return a meaningful error, Bluest will attempt to do
so. In other cases, Bluest may return an error with a [`kind`][Error::kind] of
[`Other`][error::ErrorKind::Other] and you would need to look at the
platform-specific [`source`][std::error::Error::source] of the error for more
information.

## Feature flags

The `serde` feature is available to enable serializing/deserializing device
identifiers.

## Examples

Examples demonstrating basic usage are available in the [examples folder].

[examples folder]: https://github.com/alexmoon/bluest/tree/master/bluest/examples

<!-- cargo-rdme end -->

Refer to the [API documentation] for more details.

[API documentation]: https://docs.rs/bluest
[Adapter::scan]: https://docs.rs/bluest/latest/bluest/struct.Adapter.html#method.scan
[Adapter::connected_devices]: https://docs.rs/bluest/latest/bluest/struct.Adapter.html#method.connected_devices
[Adapter::open_device]: https://docs.rs/bluest/latest/bluest/struct.Adapter.html#method.open_device
[Adapter::connect_device]: https://docs.rs/bluest/latest/bluest/struct.Adapter.html#method.connect_device
[Adapter::disconnect_device]: https://docs.rs/bluest/latest/bluest/struct.Adapter.html#method.disconnect_device
[Device::name]: https://docs.rs/bluest/latest/bluest/struct.Device.html#method.name
[Device::is_connected]: https://docs.rs/bluest/latest/bluest/struct.Device.html#method.is_connected
[Device::is_paired]: https://docs.rs/bluest/latest/bluest/struct.Device.html#method.is_paired
[Device::pair]: https://docs.rs/bluest/latest/bluest/struct.Device.html#method.pair
[Device::pair_with_agent]: https://docs.rs/bluest/latest/bluest/struct.Device.html#method.pair_with_agent
[Device::unpair]: https://docs.rs/bluest/latest/bluest/struct.Device.html#method.unpair
[Device::discover_services]: https://docs.rs/bluest/latest/bluest/struct.Device.html#method.discover_services
[Device::rssi]: https://docs.rs/bluest/latest/bluest/struct.Device.html#method.rssi
[Service::uuid]: https://docs.rs/bluest/latest/bluest/struct.Service.html#method.uuid
[Service::is_primary]: https://docs.rs/bluest/latest/bluest/struct.Service.html#method.is_primary
[Service::discover_characteristics]: https://docs.rs/bluest/latest/bluest/struct.Service.html#method.discover_characteristics
[Characteristic::uuid]: https://docs.rs/bluest/latest/bluest/struct.Characteristic.html#method.uuid
[Characteristic::properties]: https://docs.rs/bluest/latest/bluest/struct.Characteristic.html#method.properties
[Characteristic::discover_descriptors]: https://docs.rs/bluest/latest/bluest/struct.Characteristic.html#method.discover_descriptors
[Characteristic::read]: https://docs.rs/bluest/latest/bluest/struct.Characteristic.html#method.read
[Characteristic::write]: https://docs.rs/bluest/latest/bluest/struct.Characteristic.html#method.write
[Characteristic::write_without_response]: https://docs.rs/bluest/latest/bluest/struct.Characteristic.html#method.write_without_response
[Characteristic::max_write_len]: https://docs.rs/bluest/latest/bluest/struct.Characteristic.html#method.max_write_len
[Characteristic::notify]: https://docs.rs/bluest/latest/bluest/struct.Characteristic.html#method.notify
[Descriptor::uuid]: https://docs.rs/bluest/latest/bluest/struct.Descriptor.html#method.uuid
[Descriptor::read]: https://docs.rs/bluest/latest/bluest/struct.Descriptor.html#method.read
[Descriptor::write]: https://docs.rs/bluest/latest/bluest/struct.Descriptor.html#method.write
[Error::kind]: https://docs.rs/bluest/latest/bluest/error/struct.Error.html#method.kind
[error::ErrorKind::NotSupported]: https://docs.rs/bluest/latest/bluest/error/enum.ErrorKind.html#variant.NotSupported
[error::ErrorKind::Other]: https://docs.rs/bluest/latest/bluest/error/enum.ErrorKind.html#variant.Other
[std::error::Error::source]: https://doc.rust-lang.org/stable/std/error/trait.Error.html#method.source
