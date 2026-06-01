# lau-tensor-midi

**Tensor field MIDI engine for reactive improvisation.**

Rooted in the Chinese mathematical tradition — I Ching hexagrams, Taoist conservation of chi, the Book of Changes as a tensor field of possibility. Every conversation is music. Every agent has a rhythm.

---

## What This Does

`lau-tensor-midi` models a multi-agent conversation as a live musical score. Each agent has a rhythmic personality (cadence), and their interactions — excitement, pushback, questions, silence — are represented as **nudges** that perturb a shared **conversation tensor**. The system decides when each agent "speaks" (plays notes), what notes they play, and how the tempo flows — all governed by a conservation-of-energy invariant inspired by Taoist physics.

Think of it as a jazz rhythm section for AI agents: nobody calls the tune, the music emerges.

---

## Key Idea

A **conversation tensor** `T ∈ ℝ^{B × A × N}` encodes the full musical state:
- **B** = bars (time)
- **A** = agents (who's playing)
- **N** = note dimensions (pitch, velocity, duration, onset, channel)

Energy flows through this tensor like chi through a body. The total energy is budgeted and conserved — no agent can play more than the field allows. BPM adapts to energy (flow follows energy, *wu-wei*). Entropy measures how evenly distributed the musical ideas are across time.

---

## Install

Add to your `Cargo.toml`:

```toml
[dependencies]
lau-tensor-midi = "0.1.0"
```

Or use cargo:

```bash
cargo add lau-tensor-midi
```

Requires Rust 2021 edition. Only external dependency: `serde` (with `derive`).

---

## Quick Start

```rust
use lau_tensor_midi::*;

// Create the engine with 4 agents, 8 bars, and an energy budget of 500
let mut engine = ReactiveImprovEngine::new(default_cadences(), 8, 500.0);

// Send a nudge: the Architect is excited and the Implementer feels it
engine.nudge(Nudge::new(
    NudgeType::Excitement(0.7),
    "Architect",
    Some("Implementer".into()),
    0.9,
    0,
));

// Compute and commit each agent's musical contribution
for agent in &["Architect", "Implementer", "Critic", "Historian"] {
    engine.compute_draft(agent);
    engine.commit_draft(agent);
}

// Advance the simulation
for _ in 0..960 {
    engine.tick();
}

// See the state of the conversation
println!("{}", engine.summary());
```

Output looks like:

```
🎵 Conversation Field: 4 agents, 32 notes
⚡ Energy: 8.432 / budget 500.000
🥁 BPM: 82.4 (flowing)
🔥 Entropy: 1.832 bits
💫 Nudges: 1 active
📝 Drafts: 0 pending notes
📍 Tick: 960
```

---

## API Reference

### Core Types

| Type | Description |
|------|-------------|
| `TensorMidiId` | Opaque newtype identifier for tensor entities. Hashable, comparable, serializable. |
| `NoteVector` | A single note event: pitch, velocity, duration, onset, channel. Has an `energy()` method. |
| `AgentCadence` | The rhythmic fingerprint of an agent: BPM, swing, articulation, energy baseline, variation. |
| `ConversationTensor` | The tensor field: bars × agents × notes. Tracks energy, entropy, BPM. |
| `Nudge` / `NudgeType` | Reactive signals that perturb the field (Excitement, Pushback, Question, TopicShift, Silence). |
| `ReactiveImprovEngine` | The full system: tensor + nudges + draft management + conservation budget. |

### Key Methods

#### `NoteVector`
```rust
let note = NoteVector::new(60, 100, 480, 0, 0); // C4, velocity 100, quarter note, tick 0, channel 0
let energy = note.energy(); // velocity/127 * duration/480
let rest = NoteVector::silence(100, 480, 2); // zero-energy rest
```

#### `AgentCadence`
```rust
let cadence = AgentCadence::new("my-agent", 120.0);
// .tick_interval()      → ms per tick
// .swing_offset(beat)   → timing offset for swing feel
// .next_onset(tick, energy) → when agent should speak next
// .should_speak(now, last_spoke) → boolean decision
```

#### `ConversationTensor`
```rust
let tensor = ConversationTensor::new(agents, 8);
// .energy_at(tick)       → total energy at a point in time
// .agent_energy(id, bar) → per-agent energy in a bar
// .bar_energy(bar)       → total energy in a bar
// .tensor_entropy()      → Shannon entropy of note distribution
// .adapt_bpm(energy)     → Taoist BPM flow
// .is_conserved(tol)     → conservation invariant check
```

#### `ReactiveImprovEngine`
```rust
let mut engine = ReactiveImprovEngine::new(agents, 8, 500.0);
engine.nudge(/* ... */);
engine.compute_draft("Architect");   // generates notes
engine.commit_draft("Architect");    // commits if budget allows
engine.tick();                        // advance simulation
engine.summary();                     // human-readable state
```

### Pre-built Cadences

| Function | Agent | BPM | Swing | Articulation | Energy | Variation |
|----------|-------|-----|-------|-------------|--------|-----------|
| `architect_cadence()` | Architect | 80 | 0.3 | 0.7 (legato) | 0.6 | 0.4 |
| `implementer_cadence()` | Implementer | 120 | 0.1 | 0.3 (staccato) | 0.8 | 0.2 |
| `critic_cadence()` | Critic | 90 | 0.5 | 0.5 | 0.4 | 0.6 |
| `historian_cadence()` | Historian | 70 | 0.7 | 0.9 (very legato) | 0.3 | 0.8 |

`default_cadences()` returns all four.

---

## How It Works

### The Pipeline

```
Nudge → energy_delta → adapt_bpm → compute_draft → commit_draft (if budget allows) → tick
```

1. **Nudges** arrive from agents or the system, each carrying an energy delta (positive or negative).
2. **BPM adapts** via a sigmoid mapping: low energy → 60 BPM, high energy → 120 BPM. The transition is smoothed (80/20 blend) to avoid jarring tempo jumps.
3. **Draft computation**: an agent's energy (baseline + nudge modifiers) determines how many notes they produce (1–16). Pitches are drawn from a **pentatonic scale** rooted at C4 — the five-note Chinese scale (C D E G A). Swing offsets are applied to off-beat notes.
4. **Commit gate**: a draft is only committed if the total energy stays within the conservation budget. If not, the draft is rejected.
5. **Tick**: old nudges are cleaned up (older than 4 bars), BPM continues to flow.

### The Pentatonic Scale

Notes are generated from the five-note Chinese pentatonic scale: **C, D, E, G, A** (intervals 0, 2, 4, 7, 9). Octave shifts happen when the scale wraps around, keeping pitches in MIDI range [0, 127].

### Deterministic "Randomness"

The system uses no external RNG. "Randomness" for variation and should_speak decisions comes from deterministic hash functions (Knuth multiplicative hash: `tick × 2654435761`). This means the same conversation state always produces the same output — reproducible improvisation.

---

## The Math

### Note Energy

$$E(n) = \frac{v}{127} \cdot \frac{d}{480}$$

Where `v` = velocity [0, 127] and `d` = duration in ticks (480 = one quarter note).

### BPM Adaptation (Sigmoid Flow)

$$\text{target} = 60 + 60 \cdot \min\!\left(\frac{e}{e + 1},\ 1\right)$$

$$\text{bpm}_{t+1} = 0.8 \cdot \text{bpm}_t + 0.2 \cdot \text{target}$$

The exponential moving average ensures smooth tempo evolution — the system never forces, it flows (*wu-wei*).

### Shannon Entropy

$$H = -\sum_{b=1}^{B} p_b \log_2 p_b, \quad p_b = \frac{n_b}{N}$$

Where $n_b$ = number of notes in bar $b$, $N$ = total notes. Maximum entropy (notes uniformly distributed) = $\log_2 B$ bits.

### Swing Offset

Off-beat positions get delayed:

$$\Delta t_{\text{swing}} = s \cdot \frac{T}{3}$$

Where $s$ = swing amount [0, 1] and $T$ = ticks per beat. At full swing (1.0), off-beats become triplets.

### Conservation Invariant

$$\sum_{n \in \text{notes}} E(n) \leq B_{\text{budget}}$$

The total energy of all committed notes must not exceed the conservation budget. Drafts that would violate this are rejected.

### Next Onset (When to Speak)

$$t_{\text{next}} = t_{\text{current}} + \left\lfloor 4T \cdot (1 - 0.6e) \cdot j \right\rfloor$$

Where $e$ = energy, $T$ = ticks per beat, and $j$ = variation jitter (deterministic hash).

---

## Test Coverage

**71 tests** covering all types and their interactions:

- `TensorMidiId`: construction, equality, hashing, cloning, display, serde
- `NoteVector`: energy calculations, silence, serde
- `AgentCadence`: tick intervals, swing offsets, onset timing, speak decisions, BPM clamping
- `ConversationTensor`: energy at tick/bar, entropy (empty, uniform, concentrated), conservation, BPM adaptation, serde
- `Nudge`: energy deltas for all types, cadence effects, strength clamping
- `ReactiveImprovEngine`: full pipeline (nudge → draft → commit → tick), budget enforcement, topic shifts, agent should-speak logic
- Pre-built cadences: value verification

Run tests:

```bash
cargo test
```

---

## License

MIT
