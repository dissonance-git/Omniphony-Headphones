# Realtime control, continuity, and latency contract

This document owns realtime correctness for Omniphony.

It does not preserve backend tuning diaries, old state-machine parameter values, implementation inventories, or retired analysis plans. Exact constants and backend implementation live in code/tests. Git owns chronology.

> **Omniphony's audible world must not be defined by a host callback, UI refresh interval, platform buffer size, device name, model-worker completion time, or global channel mode.**

> **Intelligence may run ahead of playback. Playback may never wait for intelligence.**

The canonical time domain for audible state is the audio sample timeline.

## 1. Portable realtime boundary

```text
PLATFORM / CONTROL SIDE
UI and settings
device/session discovery
platform routing
profile/HRTF preparation
optional heavy analysis
file/network I/O
logging
        ↓ bounded validated publication
PORTABLE REALTIME SIDE
logical input streams
sample clocks / generations
current scene / presentation state
bounded trajectories
binaural / room DSP
stereo output samples
```

The realtime engine remains independently useful with no GUI, network, model worker, cache, or platform-specific control surface attached.

## 2. Stream-local format and time

Channel layout and source semantics belong to a logical stream.

Conceptually:

```text
InputStreamState
  stream_id
  sample_rate_hz
  channel_layout / source representation
  stream_generation
  absolute_sample_index
  optional authored object/field state
```

Several streams may coexist with different representations. Starting or changing one stream must not reinterpret unrelated streams.

Each independently meaningful stream has a monotonic sample clock plus a generation that changes only on a real discontinuity such as seek/reset, source replacement, incompatible format restart, or unrecoverable host restart.

A different callback size is not a semantic discontinuity.

## 3. Timed events

Audible control changes have explicit sample-time semantics.

Possible timed events include:

```text
object position / gain / extent
field gain
head pose
bypass
profile switch
room target
stream start / stop / discontinuity
```

An event says when an audible state becomes true, not when a UI or host thread happened to deliver a message.

A target event and the audible trajectory toward the target are separate concepts.

## 4. Callback partition invariance

Equivalent source/control timing with different legal block partitions should produce equivalent intended output apart from declared buffering latency.

```text
same PCM
+ same timed state
+ same semantic timeline
+ different legal callback partitions
→ equivalent intended audible world
```

Exact null is required where the algorithm should mathematically be identical.

This law applies to gain slew, object motion, HRTF motion, bypass, room modulation, transient routing, control interpolation, and analysis-state application.

## 5. Bounded realtime work

The steady-state realtime callback must not perform operations with unbounded or scheduler-dependent latency.

Forbidden:

- filesystem or network I/O;
- device/session enumeration;
- UI calls;
- SOFA/profile parsing;
- ordinary LLM/model/source-separation inference;
- graph construction;
- unbounded allocation/deallocation;
- thread creation;
- blocking logging;
- waiting for worker completion;
- unbounded mutex/RwLock waits;
- unbounded queue growth.

Local analysis is allowed only when its worst-case cost is bounded, measured, and inside the realtime budget.

Preferred lifecycle:

```text
control / preparation thread
→ construct / reserve / prewarm
→ validate
→ publish bounded ready state

realtime thread
→ reuse fixed/bounded storage
```

## 6. Transactional state publication

Construction beginning does not make a candidate authoritative.

```text
request generation N
→ build candidate outside realtime
→ validate
→ publish only if N is still current
→ otherwise discard stale result
```

A validated publication may still require a bounded audible crossfade/ramp.

On preparation failure, keep the last known-good audible state and report the error outside realtime.

## 7. Optional analysis and cache law

Heavy or learned analysis may run before playback, faster than playback, with bounded lookahead, or in another process. Its output is advisory control state.

A realtime control frame must be:

- time-indexed;
- finite;
- bounded in dimension and influence;
- version/provenance checked where needed;
- explicit about confidence/validity;
- smoothly applicable;
- safe to ignore.

If optional analysis is missing, late, stale, invalid, or crashed, normal Omniphony playback continues.

A cache is an acceleration mechanism, not the meaning of the effect. Deleting it may cause recomputation but must not make normal playback unavailable. Stale cache state must be rejected rather than applied to the wrong source/profile/version.

## 8. Stream discontinuity law

A seek, new track, decoder reset, incompatible sample-rate restart, or source-generation change must not inherit old stream-lifetime state accidentally.

Reset as applicable:

- channel/gain ramp history;
- FIR/HRTF history;
- ITD/fractional-delay history;
- crossover/filter state;
- reflection/late-room state;
- analysis-control interpolation;
- source identity/timeline state;
- adaptive latency/control integrators when continuity is no longer valid.

Immutable prepared assets may survive only when still valid.

## 9. Failure containment

### Analyzer/model unavailable
Playback continues with the normal presentation.

### Invalid/non-finite control
Reject it before it reaches output.

### Queue pressure
Use a declared bounded coalescing/drop/backpressure policy appropriate to the queue. Never grow memory without bound or silently discard continuity-critical processed audio.

### Renderer/profile preparation failure
Keep last-known-good state or the normal fallback path.

### Optional mechanism failure
Disable/narrow that layer rather than damage direct music.

### Host/device interruption
Recover or fail to ordinary audio behavior according to the host contract; do not convert an availability event into stale source semantics.

## 10. Latency is a controlled quantity, not a single number

Track latency in separate domains:

```text
host/device buffering
renderer buffering / worker handoff
algorithmic lookahead
FIR/convolution
resampling / clock adaptation
control interpolation
```

A controller should regulate toward a declared target/setpoint with bounded recovery, not chase minimum latency at the cost of instability.

Where producer and consumer clocks differ, the host may use bounded local resampling or another clock-control mechanism. The controller must distinguish:

- actual buffer inventory/control state;
- total user-facing latency estimate;
- low-buffer recovery;
- normal stable servo operation;
- high-buffer recovery.

Implementation-specific thresholds, PI constants, smoothing constants, backend estimates, and state names belong in code/tests unless they are part of a public ABI.

## 11. Recovery law

Hard recovery and fine clock regulation are different jobs.

A valid recovery system must:

- detect dangerously low/high inventory from unsmoothed state quickly enough to protect continuity;
- use smoothing only where it improves stable control rather than hiding hard faults;
- avoid running a fine servo against a deliberately muted/refilling hard-recovery phase when those controls would fight each other;
- return to normal output through a deterministic settling/transition rule;
- make underruns, overruns, drops, mutes, and recovery transitions observable;
- avoid leaking unstable partially recovered audio merely to minimize apparent latency.

Startup may reuse the same recovery semantics when doing so avoids a second hidden state machine.

## 12. No host-shaped sound

Callback cadence, graph quantum, device period, queue chunking, and backend batch size are transport details.

They must not redefine:

- source motion;
- placement;
- scene adaptation;
- HRTF transitions;
- bypass timing;
- musical dynamics;
- room behavior.

A host may introduce declared latency. It may not introduce a different musical world.

## 13. CPU and memory budgets

Average CPU is insufficient. Measure, where relevant:

- median callback/worker time;
- p95/p99;
- maximum observed processing time;
- deadline misses;
- queue occupancy/recovery;
- peak memory;
- allocations after warm-up;
- reset/state-transition cost;
- device/sample-rate change cost.

A feature that occasionally overruns the deadline is not lightweight merely because its average is small.

Memory and CPU should reach bounded steady state during long playback.

## 14. Soak behavior

Always-on validation should cover long playback plus ordinary discontinuities and desktop stress:

```text
hours of playback
track boundaries
seek / pause / resume
sample-rate changes
silence → loud transient
bass-heavy / dense / sparse material
device interruption / restart
optional analysis present / absent / late / invalid
```

Observe glitches/xruns/underruns, recovery events, non-finite samples, clipping/safety intervention, memory growth, and processing maxima.

Completed soak reports do not become permanent documents. Encode regressions/tests where possible; unresolved failures belong in `../ROADMAP.md`.

## 15. Fidelity floor

Realtime stability work may not quietly make the renderer sound worse.

Protect:

- transient timing;
- bass timing/weight;
- center authority;
- timbre;
- dynamics;
- groove/microtiming;
- important stereo relations;
- smooth switching;
- absence of pumping/spatial twitching;
- comfortable spectral balance.

## 16. Acceptance sentence

> **Omniphony is suitable as normal playback infrastructure when it can remain enabled indefinitely, survive ordinary host stress without demanding attention, and turning it off mainly removes the spatial world rather than restoring damaged music.**
