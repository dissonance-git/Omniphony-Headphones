# Omniphony OSC control/state contract

This document owns the **wire-level semantic rules** for Omniphony OSC control and state publication.

It does **not** manually inventory every address. The machine-readable canonical address vocabulary is `runtime_control::osc_contract` in `omniphony-renderer/runtime_control/src/osc_contract.rs`, including `ALL_CONTROL` and `ALL_STATE`. Exact addresses, tunables, enums, and schemas must be derived from that executable owner rather than copied into a second hand-maintained table.

## 1. Directions

```text
/omniphony/control/...
client → engine
request an action or state change

/omniphony/state/...
engine → subscribed clients
publish current state / deltas / diagnostics
```

Controls request state. State messages report authoritative engine state. A client must not assume a sent control succeeded until the resulting state/acknowledgement semantics say so.

## 2. Address ownership

Every fixed OSC address has one named constant in `runtime_control::osc_contract`.

Rules:

- dispatchers/producers reference the constant rather than duplicating string literals;
- `ALL_CONTROL` and `ALL_STATE` are exhaustive machine-readable inventories;
- tests enforce address namespace and duplicate-address invariants;
- client documentation or UI inventories should be generated/queried from the executable schema where practical;
- this document changes only when protocol semantics change, not whenever an address is added.

## 3. Argument conventions

Unless a narrower executable schema says otherwise:

- booleans may be accepted from the supported OSC scalar forms and normalized by the engine;
- enums are validated strings; invalid values are rejected/ignored rather than becoming partial state;
- realtime ordered controls carry sequence/generation information when stale UDP delivery would be unsafe;
- larger structured payloads use a declared serialized schema rather than ad-hoc positional growth;
- non-finite numeric input is invalid for audible state;
- bounds are validated before publication into realtime state.

The executable option/schema registry is authoritative for exact types, ranges, enum values, defaults, and current addresses.

## 4. UDP and ordering law

OSC transport is UDP. Delivery may be late, duplicated, or out of order.

Therefore controls whose ordering matters must carry enough sequence/generation identity for the engine to reject stale state.

```text
newer authoritative control
→ accepted

older delayed control
→ rejected
```

Do not rely on packet arrival order as semantic time.

Audible application time follows `realtime-control-contract.md`, not UDP receive time.

## 5. Snapshot and delta law

A client may receive:

```text
full snapshot
+ later deltas
```

A snapshot establishes a coherent baseline. Deltas amend that baseline only when their generation/version semantics are compatible.

Late-attaching clients must be able to reconstruct current state without relying on messages that happened before they subscribed.

Sparse declarations may be cached by the engine, but cache lifetime must follow the owning generation/reset semantics.

## 6. Transactional configuration

Large configuration updates should use stage/validate/apply semantics when partial mutation could create an invalid audible state.

```text
candidate config
→ parse / validate outside realtime
→ publish atomically if still current
→ state publication confirms result
```

A failed candidate leaves last-known-good state active.

## 7. Realtime controls

Realtime controls such as gain, head pose, bypass, and other audible targets publish bounded target state. They do not perform graph construction or heavy work in the audio callback.

The control path may request a target immediately; the renderer owns the sample-time trajectory required to reach it safely.

Callback/block boundaries must not become audible automation boundaries.

## 8. Head tracking

Head-tracking feeds may use configurable OSC addresses/formats, but their semantic output is listener pose.

Requirements:

- explicit coordinate convention;
- finite normalized pose;
- recenter semantics;
- bounded smoothing/transition;
- stale/dropout handling;
- source authority unchanged by listener motion.

A configurable sensor-feed address is configuration, not part of the fixed Omniphony control-address inventory unless explicitly declared there.

## 9. Latency and recovery controls

OSC may expose latency targets, adaptive clock-control settings, and recovery diagnostics. Those controls configure/observe the realtime system; they do not define its underlying correctness law.

`realtime-control-contract.md` owns:

- target-latency semantics;
- stable vs hard-recovery separation;
- bounded clock adaptation;
- underrun/overrun observability;
- sample-time invariance.

Exact tunables and backend-specific controls are derived from the executable schema/code.

## 10. Diagnostics and lifecycle

Diagnostic/metering publication must be rate-bounded and must not make realtime audio wait for OSC clients.

A client disappearing must not stop rendering.

Engine shutdown/yield semantics must be explicit enough that one process cannot accidentally evict an unrelated authoritative renderer instance.

## 11. Changing the protocol

For a protocol change:

1. change the canonical constant/schema in code;
2. change dispatcher/producer behavior using that canonical owner;
3. add/update executable tests;
4. update this document only if the wire semantics changed;
5. do not paste a new full address table here.

> **The protocol vocabulary is executable data. This document owns only the semantics that make that vocabulary safe and coherent.**
