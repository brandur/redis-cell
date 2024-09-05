# Changelog

## 0.4.0 - 2024-09-05
* [#76](https://github.com/brandur/redis-cell/pull/76) Upgrade dependencies
* [#77](https://github.com/brandur/redis-cell/pull/77) Include `Cargo.lock`

## 0.3.1 - 2023-06-02
* [#66](https://github.com/brandur/redis-cell/pull/66) Publish to ARM64 for Linux

## 0.3.0 - 2021-03-05
* [#53](https://github.com/brandur/redis-cell/pull/53) Replicate `CL.THROTTLE` invocations to replicas/AOF

## 0.2.5 - 2020-04-05
* [#42](https://github.com/brandur/redis-cell/pull/42) Explicitly don't support zero rates

## 0.2.4 - 2019-03-24
* [#31](https://github.com/brandur/redis-cell/pull/31) Add a replication invocation to allow Redis Cluster compatibility.
* [#32](https://github.com/brandur/redis-cell/pull/32) Minor documentation fixups.

## 0.2.3 - 2019-03-18
* [#29](https://github.com/brandur/redis-cell/pull/29) Fix `firstkey`/`lastkey`/`keystep` arguments for command creation which allows redis-cell to work correctly with Redis Cluster. [#29]

## 0.2.2 - 2019-02-27
* [#13](https://github.com/brandur/redis-cell/pull/13) Fix bug where limiting was calculated incorrectly for rates faster than 1 second (i.e., N operations per second).
* [#14](https://github.com/brandur/redis-cell/pull/14) Fix bug where the case of a value being expired was not checked in `compare_and_swap_with_ttl`. This problem also manifested for fast rates smaller than 1 second.

## 0.2.1 - 2017-05-29
* Minor build fixes.

## 0.2.0 - 2016-11-08
* The project has been renamed from "redis-throttle" to "redis-cell" to avoid naming contention with the multitude of projects that are already named for the former.

## 0.1.0 - 2016-11-06
* Initial release.

<!--
# vim: set tw=0:
-->
