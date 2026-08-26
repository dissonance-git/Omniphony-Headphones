# Windows audio route

> **Evidence status:** historical Windows transport decision record.
>
> This file does not own the current Windows architecture, current implementation state, or project frontier. The durable Windows product/host boundary is `omniphony-for-windows.md`; unresolved work and gates are owned by `../ROADMAP.md`; executable state is owned by code, tests, and CI. Git history preserves the exact former prototype implementations.

This record keeps the transport constraints and negative/positive evidence learned from the early cable/loopback/ASIO migration period without allowing that scaffolding to compete with the current product contract.

## Durable lessons earned by the transport experiments

### Windows is a host, not the renderer

The portable core should consume source PCM plus source-authority/geometry/timing semantics and produce binaural stereo. Windows-specific device, session, endpoint, clock, capture, installer, and recovery concepts stay at the host boundary.

### Source layout belongs to each logical stream

The experiments established the intended semantic shape:

```text
Stream A { stereo }
Stream B { authored surround }
Stream C { mono }
        ↓
portable Omniphony scene / renderer
        ↓
binaural stereo
```

A richer stream must not force unrelated stereo playback into a global channel mode, and stereo playback must not flatten a richer stream merely because both are active.

### One physical audible path

The strongest transport invariant from the early prototype work was:

```text
source
→ Omniphony
→ physical headphones
```

not:

```text
source ─────────────→ physical headphones
   └→ Omniphony ───→ physical headphones
```

The early listening path produced tinny, hallway-like/echo-like results while incumbent routing was still configured. That observation was not accepted as a renderer-quality verdict because a duplicate delayed physical path had not yet been excluded.

> **No listening comparison is trustworthy until the physical route is proven single.**

### OFF must be route-clean

Bypass is a transport obligation, not merely a UI flag.

A valid OFF path must not leak queued wet tails, stale room state, a second dry forwarding path, or another active physical route. Comparisons should switch near the final output boundary and be latency-aware where required.

### Rich source truth should survive the host boundary

The early route work reinforced that platform layout adaptation must be explicit. A successful compile is not evidence that channel identity survived.

Historical Windows 7.1 interleave was recorded as:

```text
Windows:
L R C LFE Lb Rb Ls Rs

Omniphony bridge:
L R C LFE Ls Rs Lb Rb
```

The general rule survives even if the exact implementation changes:

> **Adapt channel order explicitly at the host boundary; never infer correctness from shape alone.**

## Historical prototype evidence

On 2026-08-10 an early native app prototype carried arbitrary Windows/foobar audio through Omniphony to the real FiiO/headphones.

The then-temporary route was approximately:

```text
Windows / foobar
→ existing Hi-Fi Cable endpoint
→ self-excluding process-loopback capture
→ Omniphony renderer
→ FiiO
→ headphones
```

At that point:

```text
live arbitrary-audio transport = demonstrated
single physical path           = not yet demonstrated
clean bypass                   = not yet demonstrated
fair renderer-quality A/B      = not yet demonstrated
```

That route was intentionally development scaffolding. It is preserved here as evidence of what the experiment established and what it could not establish, not as a product design recommendation.

The incumbent HeSuVi / Hi-Fi Cable / ASIO chain was kept installed during migration so one function could be disabled and replaced at a time. “Installed” and “active” were treated as separate states. That migration discipline remains useful even though the specific chain is historical.

## Historical route candidates

The early investigation considered several Windows host classes:

- an owned virtual render endpoint;
- native system-effect / in-graph integration;
- session-aware host routing;
- hybrids of those approaches.

The decision criteria were more durable than any candidate:

```text
single physical path
source-truth preservation
concurrent-layout correctness
latency
reliability
installability
recovery
clean disable / uninstall
low user ritual
```

Later implementation evidence supersedes the old candidate ranking. Current route decisions therefore belong to `omniphony-for-windows.md`, `../ROADMAP.md`, and executable state.

## Historical binaries

The old transport phase used names including `Omniphony.exe`, `omniphony_worker.exe`, `omniphony_live.exe`, `windows_host.exe`, `realtime_ffi`, and `reference_bridge`.

Those names are retained here only to make old commits and diagnostics searchable. They are not a current implementation inventory. Derive current binaries/components from the repository and build system.

## Retained transport laws

The experiments earned the following durable constraints:

1. Windows is a host, not the portable core.
2. Ordinary stereo must work with an ordinary stereo headphone endpoint.
3. Source layout and authority belong to each logical source/stream, not a global Omniphony mode.
4. Independent source layouts should be able to coexist without corrupting one another.
5. Preserve native rich source truth rather than reconstructing it after flattening.
6. One physical audible path only.
7. OFF/bypass must be route-clean.
8. Platform layout conversion must preserve explicit channel/object identity.
9. Temporary cable/loopback/development transports are evidence scaffolding, not product architecture.
10. Choose the Windows mechanism by demonstrated obligations, not architectural fashion.

Anything beyond these retained lessons is historical context, not current project state.
