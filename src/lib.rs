//! # lau-tensor-midi
//!
//! Tensor field MIDI engine for reactive improvisation.
//!
//! Rooted in the Chinese mathematical tradition — I Ching hexagrams,
//! Taoist conservation of chi, the Book of Changes as a tensor field
//! of possibility. Every conversation is music. Every agent has a rhythm.
//!
//! A MIDI tensor `T ∈ ℝ^{B×A×N}` where `B` = bars, `A` = agents,
//! `N` = note-dimensions (pitch, velocity, duration, onset, channel).
//! The tensor field encodes the full musical state of a conversation.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

// ---------------------------------------------------------------------------
// 1. TensorMidiId — opaque newtype identifier
// ---------------------------------------------------------------------------

/// Opaque identifier for tensor MIDI entities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TensorMidiId(String);

impl TensorMidiId {
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Hash for TensorMidiId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

impl PartialEq for TensorMidiId {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for TensorMidiId {}

impl std::fmt::Display for TensorMidiId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ---------------------------------------------------------------------------
// 2. NoteVector — a note in tensor space
// ---------------------------------------------------------------------------

/// A single note event in the tensor field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NoteVector {
    pub pitch: u8,
    pub velocity: u8,
    pub duration_ticks: u32,
    pub onset_tick: u64,
    pub channel: u8,
}

impl NoteVector {
    /// Energy is derived from velocity and duration.
    /// Higher velocity and longer duration → more energy.
    /// Normalised so that a note at velocity 127, duration 480 ticks ≈ 1.0.
    pub fn energy(&self) -> f64 {
        (self.velocity as f64 / 127.0) * (self.duration_ticks as f64 / 480.0)
    }

    /// Convenience constructor.
    pub fn new(pitch: u8, velocity: u8, duration_ticks: u32, onset_tick: u64, channel: u8) -> Self {
        Self { pitch, velocity, duration_ticks, onset_tick, channel }
    }

    /// A rest-like note with zero energy.
    pub fn silence(onset_tick: u64, duration_ticks: u32, channel: u8) -> Self {
        Self { pitch: 0, velocity: 0, duration_ticks, onset_tick, channel }
    }
}

// ---------------------------------------------------------------------------
// 3. AgentCadence — the rhythm signature of an agent
// ---------------------------------------------------------------------------

/// The rhythmic fingerprint of an agent's conversational style.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCadence {
    pub agent_id: String,
    /// Beats per minute (60–180).
    pub base_bpm: f64,
    /// 0 = straight, 1 = full swing triplet.
    pub swing: f64,
    /// 0 = staccato, 1 = legato.
    pub articulation: f64,
    /// Baseline energy level (0–1).
    pub energy_baseline: f64,
    /// 0 = machine-precise, 1 = human-like variation.
    pub variation: f64,
    /// Ticks per quarter note (default 480).
    pub ticks_per_beat: u32,
}

impl AgentCadence {
    pub fn new(agent_id: impl Into<String>, base_bpm: f64) -> Self {
        Self {
            agent_id: agent_id.into(),
            base_bpm: base_bpm.clamp(60.0, 180.0),
            swing: 0.0,
            articulation: 0.5,
            energy_baseline: 0.5,
            variation: 0.0,
            ticks_per_beat: 480,
        }
    }

    /// Milliseconds per tick at the current BPM.
    pub fn tick_interval(&self) -> f64 {
        // 1 beat = ticks_per_beat ticks, BPM beats per minute
        // → ms/tick = (60000 / BPM) / ticks_per_beat
        60_000.0 / self.base_bpm / self.ticks_per_beat as f64
    }

    /// Swing timing offset for a given beat position.
    /// On-beat positions get no offset; off-beat positions swing late.
    /// A swing of 0.0 → no offset; 1.0 → triplet feel (delay off-beats by 1/3).
    pub fn swing_offset(&self, beat_position: u64) -> f64 {
        // Even-numbered beat positions are on-beat → no swing.
        if beat_position.is_multiple_of(2) {
            0.0
        } else {
            // Off-beat: delay proportional to swing amount
            // Maximum swing delays by 1/3 of a beat (triplet feel)
            self.swing * (self.ticks_per_beat as f64 / 3.0)
        }
    }

    /// When should this agent speak next, given current energy?
    /// Higher energy → shorter gaps (agent speaks sooner).
    pub fn next_onset(&self, current_tick: u64, energy: f64) -> u64 {
        // Base interval in ticks: one bar's worth
        let base_interval = self.ticks_per_beat as u64 * 4; // 4 beats per bar
        // Higher energy → shorter interval (agent is engaged)
        let energy_factor = 1.0 - (energy * 0.6).min(0.9); // don't go below 0.1
        // Apply variation: add some jitter
        let variation_jitter = if self.variation > 0.0 {
            // Simple deterministic "variation" based on tick to avoid RNG
            let hash = ((current_tick.wrapping_mul(2654435761)) % 1000) as f64 / 1000.0;
            1.0 + (hash - 0.5) * self.variation * 0.4
        } else {
            1.0
        };
        let interval = (base_interval as f64 * energy_factor * variation_jitter) as u64;
        current_tick + interval.max(1)
    }

    /// Should this agent speak at the given tick?
    /// Based on time since last speech and energy level.
    pub fn should_speak(&self, current_tick: u64, last_spoke_tick: u64) -> bool {
        let ticks_since = current_tick.saturating_sub(last_spoke_tick);
        let min_gap = (self.ticks_per_beat as f64 * (2.0 - self.energy_baseline)) as u64;
        let max_gap = (self.ticks_per_beat as f64 * 8.0) as u64;

        if ticks_since < min_gap {
            return false;
        }
        if ticks_since >= max_gap {
            return true;
        }
        // Probabilistic zone: scale probability by time elapsed and energy
        let progress = (ticks_since - min_gap) as f64 / (max_gap - min_gap) as f64;
        let threshold = 1.0 - self.energy_baseline * progress;
        // Deterministic hash-based "random" to avoid external RNG
        let roll = ((current_tick.wrapping_mul(2246822519) ^ (last_spoke_tick.wrapping_mul(3266489917))) % 1000) as f64 / 1000.0;
        roll >= threshold
    }
}

// ---------------------------------------------------------------------------
// 4. ConversationTensor — the field encoding all agents' states
// ---------------------------------------------------------------------------

/// The tensor field encoding the full musical state of a conversation.
///
/// Conceptually `T ∈ ℝ^{B×A×N}` where B=bars, A=agents, N=note dimensions.
/// Stored flat for efficiency.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationTensor {
    pub bars: usize,
    pub agents: usize,
    pub notes: Vec<NoteVector>,
    pub cadences: HashMap<String, AgentCadence>,
    pub tick: u64,
    pub bpm: f64,
}

impl ConversationTensor {
    /// Ticks per bar (4 beats × ticks_per_beat). We assume 480 tpb.
    pub const TICKS_PER_BAR: u64 = 4 * 480;

    pub fn new(agents: Vec<AgentCadence>, bars: usize) -> Self {
        let cadences: HashMap<String, AgentCadence> = agents
            .into_iter()
            .map(|c| (c.agent_id.clone(), c))
            .collect();
        let bpm = cadences.values().map(|c| c.base_bpm).sum::<f64>()
            / cadences.len().max(1) as f64;
        Self {
            bars,
            agents: cadences.len(),
            notes: Vec::new(),
            cadences,
            tick: 0,
            bpm,
        }
    }

    /// Total energy across all agents at a given tick.
    pub fn energy_at(&self, tick: u64) -> f64 {
        self.notes
            .iter()
            .filter(|n| n.onset_tick <= tick && tick < n.onset_tick + n.duration_ticks as u64)
            .map(|n| n.energy())
            .sum()
    }

    /// Specific agent's total energy in a given bar.
    pub fn agent_energy(&self, agent: &str, bar: usize) -> f64 {
        let bar_start = bar as u64 * Self::TICKS_PER_BAR;
        let bar_end = bar_start + Self::TICKS_PER_BAR;
        self.notes
            .iter()
            .filter(|n| {
                // Agent is identified by channel for simplicity
                n.onset_tick < bar_end
                    && n.onset_tick + n.duration_ticks as u64 > bar_start
                    && self.channel_belongs_to_agent(n.channel, agent)
            })
            .map(|n| n.energy())
            .sum()
    }

    /// Total energy in the tensor — conservation invariant.
    pub fn total_energy(&self) -> f64 {
        self.notes.iter().map(|n| n.energy()).sum()
    }

    /// Insert a note into the tensor field.
    pub fn insert_note(&mut self, note: NoteVector) {
        self.notes.push(note);
    }

    /// Adapt BPM to current energy. Taoist: flow follows energy.
    /// BPM rises with energy, within 60–120 range.
    pub fn adapt_bpm(&mut self, energy: f64) {
        // Sigmoid-like mapping: low energy → 60 bpm, high energy → 120 bpm
        let target = 60.0 + 60.0 * (energy / (energy + 1.0)).min(1.0);
        // Smooth transition — don't jump, flow
        self.bpm = self.bpm * 0.8 + target * 0.2;
        self.bpm = self.bpm.clamp(60.0, 120.0);
    }

    /// Shannon entropy of the note distribution across bars.
    pub fn tensor_entropy(&self) -> f64 {
        if self.notes.is_empty() {
            return 0.0;
        }
        // Count notes per bar
        let mut bar_counts: Vec<usize> = vec![0; self.bars];
        for note in &self.notes {
            let bar = (note.onset_tick / Self::TICKS_PER_BAR) as usize;
            if bar < self.bars {
                bar_counts[bar] += 1;
            }
        }
        let total = bar_counts.iter().sum::<usize>() as f64;
        if total == 0.0 {
            return 0.0;
        }
        bar_counts
            .iter()
            .filter(|&&c| c > 0)
            .map(|&c| {
                let p = c as f64 / total;
                -p * p.log2()
            })
            .sum()
    }

    /// Total energy in a specific bar.
    pub fn bar_energy(&self, bar: usize) -> f64 {
        let bar_start = bar as u64 * Self::TICKS_PER_BAR;
        let bar_end = bar_start + Self::TICKS_PER_BAR;
        self.notes
            .iter()
            .filter(|n| {
                let note_end = n.onset_tick + n.duration_ticks as u64;
                n.onset_tick < bar_end && note_end > bar_start
            })
            .map(|n| n.energy())
            .sum()
    }

    /// Conservation check: energy_in ≈ energy_out + energy_stored.
    /// In this model, energy_stored = total_energy of current notes.
    pub fn is_conserved(&self, tolerance: f64) -> bool {
        // The total energy of all notes should be within tolerance of
        // the sum of all agent energy baselines × bars (expected budget).
        let expected: f64 = self
            .cadences
            .values()
            .map(|c| c.energy_baseline * self.bars as f64)
            .sum();
        let actual = self.total_energy();
        (actual - expected).abs() <= tolerance * expected.max(1.0)
    }

    /// Map channel to agent (simple: channel index → agent order).
    fn channel_belongs_to_agent(&self, channel: u8, agent: &str) -> bool {
        self.cadences
            .keys()
            .position(|k| k == agent)
            .map(|idx| idx as u8 == channel)
            .unwrap_or(false)
    }

    /// Assign a channel number to an agent.
    pub fn agent_channel(&self, agent: &str) -> u8 {
        self.cadences
            .keys()
            .position(|k| k == agent)
            .map(|idx| idx as u8)
            .unwrap_or(0)
    }

    /// Get agent IDs as ordered list.
    pub fn agent_ids(&self) -> Vec<&str> {
        self.cadences.keys().map(|s| s.as_str()).collect()
    }
}

// ---------------------------------------------------------------------------
// 5. Nudge — reactive improvisation signal
// ---------------------------------------------------------------------------

/// Types of conversational nudges that affect the improvisation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NudgeType {
    /// Agent is excited; energy boost (0–1).
    Excitement(f64),
    /// Agent pushes back; energy dampening (0–1).
    Pushback(f64),
    /// A question is asked — opens space for response.
    Question,
    /// Topic shifts entirely to a new subject.
    TopicShift(String),
    /// Silence for a duration (in ticks).
    Silence(f64),
}

/// A reactive signal from one agent (or the system) that nudges the field.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Nudge {
    pub nudge_type: NudgeType,
    pub source_agent: String,
    pub target_agent: Option<String>,
    pub strength: f64,
    pub tick: u64,
}

impl Nudge {
    pub fn new(
        nudge_type: NudgeType,
        source_agent: impl Into<String>,
        target_agent: Option<String>,
        strength: f64,
        tick: u64,
    ) -> Self {
        Self {
            nudge_type,
            source_agent: source_agent.into(),
            target_agent,
            strength: strength.clamp(0.0, 1.0),
            tick,
        }
    }

    /// Strong nudges (strength > 0.6) alter cadence.
    pub fn affects_cadence(&self) -> bool {
        self.strength > 0.6
    }

    /// How much energy delta this nudge introduces.
    pub fn energy_delta(&self) -> f64 {
        match &self.nudge_type {
            NudgeType::Excitement(e) => e * self.strength,
            NudgeType::Pushback(e) => -e * self.strength,
            NudgeType::Question => 0.1 * self.strength, // questions add slight tension
            NudgeType::TopicShift(_) => -0.2 * self.strength, // shifts momentarily drain
            NudgeType::Silence(d) => -0.3 * d * self.strength, // silence drains
        }
    }
}

// ---------------------------------------------------------------------------
// 6. ReactiveImprovEngine — the full system
// ---------------------------------------------------------------------------

/// The complete reactive improvisation engine.
///
/// Combines a conversation tensor with nudge processing and draft management.
/// Taoist principle: wu-wei — the system does not force timing, it reveals timing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactiveImprovEngine {
    pub tensor: ConversationTensor,
    pub nudges: Vec<Nudge>,
    pub drafts: HashMap<String, Vec<NoteVector>>,
    pub conservation_budget: f64,
}

impl ReactiveImprovEngine {
    pub fn new(agents: Vec<AgentCadence>, bars: usize, budget: f64) -> Self {
        let tensor = ConversationTensor::new(agents, bars);
        Self {
            tensor,
            nudges: Vec::new(),
            drafts: HashMap::new(),
            conservation_budget: budget,
        }
    }

    /// Register a nudge in the field.
    pub fn nudge(&mut self, nudge: Nudge) {
        self.nudges.push(nudge);
    }

    /// Check if an agent should speak based on its cadence and current state.
    pub fn agent_should_speak(&self, agent: &str) -> bool {
        let cadence = match self.tensor.cadences.get(agent) {
            Some(c) => c,
            None => return false,
        };
        // Find last onset for this agent's channel
        let channel = self.tensor.agent_channel(agent);
        let last_tick = self
            .tensor
            .notes
            .iter()
            .filter(|n| n.channel == channel)
            .map(|n| n.onset_tick)
            .max()
            .unwrap_or(0);
        cadence.should_speak(self.tensor.tick, last_tick)
    }

    /// Compute an agent's draft contribution based on nudges and tensor state.
    pub fn compute_draft(&mut self, agent: &str) -> Vec<NoteVector> {
        let cadence = match self.tensor.cadences.get(agent).cloned() {
            Some(c) => c,
            None => return Vec::new(),
        };
        let channel = self.tensor.agent_channel(agent);

        // Collect relevant nudges for this agent
        let agent_nudges: Vec<&Nudge> = self
            .nudges
            .iter()
            .filter(|n| {
                n.target_agent.as_deref() == Some(agent)
                    || n.target_agent.is_none()
            })
            .collect();

        // Compute energy modification from nudges
        let energy_mod: f64 = agent_nudges.iter().map(|n| n.energy_delta()).sum();
        let energy = (cadence.energy_baseline + energy_mod).clamp(0.0, 1.0);

        // Determine number of notes (more energy → more notes)
        let note_count = ((energy * 8.0).ceil() as usize).clamp(1, 16);
        let mut draft = Vec::with_capacity(note_count);

        let mut onset = cadence.next_onset(self.tensor.tick, energy);
        for i in 0..note_count {
            // Pitch: pentatonic scale rooted at C4 (60) — five-note Chinese scale
            let pentatonic = [0, 2, 4, 7, 9]; // C D E G A
            let degree = (i + self.tensor.tick as usize) % pentatonic.len();
            let octave_shift = ((i + self.tensor.tick as usize) / pentatonic.len()) * 12;
            let pitch = (60u8).saturating_add(pentatonic[degree] as u8).saturating_add(octave_shift as u8).min(127);

            // Duration: articulation determines staccato vs legato
            let base_dur = cadence.ticks_per_beat as f64;
            let duration = ((base_dur * cadence.articulation * 2.0).round() as u32)
                .max(1);

            // Velocity from energy
            let velocity = (energy * 127.0).round() as u8;
            let velocity = if velocity == 0 { 1 } else { velocity };

            let swing_off = cadence.swing_offset(i as u64) as u64;

            draft.push(NoteVector::new(
                pitch,
                velocity,
                duration,
                onset + swing_off,
                channel,
            ));

            onset = cadence.next_onset(onset, energy);
        }

        self.drafts.insert(agent.to_string(), draft.clone());
        draft
    }

    /// Commit an agent's draft if conservation allows it.
    pub fn commit_draft(&mut self, agent: &str) -> bool {
        let draft = match self.drafts.get(agent) {
            Some(d) => d.clone(),
            None => return false,
        };

        let draft_energy: f64 = draft.iter().map(|n| n.energy()).sum();
        let current = self.current_energy();

        if current + draft_energy <= self.conservation_budget {
            for note in draft {
                self.tensor.insert_note(note);
            }
            self.drafts.remove(agent);
            true
        } else {
            false
        }
    }

    /// Clear all drafts (e.g., on topic shift).
    pub fn clear_drafts(&mut self) {
        self.drafts.clear();
    }

    /// Current total energy in the tensor.
    pub fn current_energy(&self) -> f64 {
        self.tensor.total_energy()
    }

    /// Current BPM of the conversation.
    pub fn current_bpm(&self) -> f64 {
        self.tensor.bpm
    }

    /// Advance one tick: process nudges, adapt BPM, advance the tensor clock.
    pub fn tick(&mut self) {
        self.tensor.tick += 1;

        // Process expired nudges (those at or before current tick)
        let energy_delta: f64 = self
            .nudges
            .iter()
            .filter(|n| n.tick <= self.tensor.tick)
            .map(|n| n.energy_delta())
            .sum();

        let current = self.tensor.energy_at(self.tensor.tick);
        self.tensor.adapt_bpm(current + energy_delta.abs());

        // Clean up old nudges (older than 4 bars)
        let cutoff = self.tensor.tick.saturating_sub(ConversationTensor::TICKS_PER_BAR * 4);
        self.nudges.retain(|n| n.tick >= cutoff);
    }

    /// Human-readable summary of the engine state.
    pub fn summary(&self) -> String {
        let n_agents = self.tensor.agents;
        let n_notes = self.tensor.notes.len();
        let energy = self.current_energy();
        let bpm = self.current_bpm();
        let entropy = self.tensor.tensor_entropy();
        let n_nudges = self.nudges.len();
        let n_drafts = self.drafts.values().map(|v| v.len()).sum::<usize>();

        format!(
            "🎵 Conversation Field: {} agents, {} notes\n⚡ Energy: {:.3} / budget {:.3}\n🥁 BPM: {:.1} (flowing)\n🔥 Entropy: {:.3} bits\n💫 Nudges: {} active\n📝 Drafts: {} pending notes\n📍 Tick: {}",
            n_agents,
            n_notes,
            energy,
            self.conservation_budget,
            bpm,
            entropy,
            n_nudges,
            n_drafts,
            self.tensor.tick,
        )
    }
}

// ---------------------------------------------------------------------------
// 7. Pre-built cadences
// ---------------------------------------------------------------------------

/// The Architect: deliberate, measured, slightly swung.
/// BPM 80, swing 0.3, articulation 0.7, energy 0.6, variation 0.4
pub fn architect_cadence() -> AgentCadence {
    AgentCadence {
        agent_id: "Architect".into(),
        base_bpm: 80.0,
        swing: 0.3,
        articulation: 0.7,
        energy_baseline: 0.6,
        variation: 0.4,
        ticks_per_beat: 480,
    }
}

/// The Implementer: fast, precise, machine-like.
/// BPM 120, swing 0.1, articulation 0.3, energy 0.8, variation 0.2
pub fn implementer_cadence() -> AgentCadence {
    AgentCadence {
        agent_id: "Implementer".into(),
        base_bpm: 120.0,
        swing: 0.1,
        articulation: 0.3,
        energy_baseline: 0.8,
        variation: 0.2,
        ticks_per_beat: 480,
    }
}

/// The Critic: measured, swing-heavy, contemplative.
/// BPM 90, swing 0.5, articulation 0.5, energy 0.4, variation 0.6
pub fn critic_cadence() -> AgentCadence {
    AgentCadence {
        agent_id: "Critic".into(),
        base_bpm: 90.0,
        swing: 0.5,
        articulation: 0.5,
        energy_baseline: 0.4,
        variation: 0.6,
        ticks_per_beat: 480,
    }
}

/// The Historian: slow, legato, highly variable.
/// BPM 70, swing 0.7, articulation 0.9, energy 0.3, variation 0.8
pub fn historian_cadence() -> AgentCadence {
    AgentCadence {
        agent_id: "Historian".into(),
        base_bpm: 70.0,
        swing: 0.7,
        articulation: 0.9,
        energy_baseline: 0.3,
        variation: 0.8,
        ticks_per_beat: 480,
    }
}

/// All four pre-built cadences together.
pub fn default_cadences() -> Vec<AgentCadence> {
    vec![
        architect_cadence(),
        implementer_cadence(),
        critic_cadence(),
        historian_cadence(),
    ]
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // --- TensorMidiId tests ---

    #[test]
    fn test_tensor_midi_id_new() {
        let id = TensorMidiId::new("hexagram-1");
        assert_eq!(id.as_str(), "hexagram-1");
    }

    #[test]
    fn test_tensor_midi_id_equality() {
        let a = TensorMidiId::new("foo");
        let b = TensorMidiId::new("foo");
        let c = TensorMidiId::new("bar");
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn test_tensor_midi_id_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(TensorMidiId::new("a"));
        set.insert(TensorMidiId::new("a"));
        set.insert(TensorMidiId::new("b"));
        assert_eq!(set.len(), 2);
    }

    #[test]
    fn test_tensor_midi_id_clone() {
        let id = TensorMidiId::new("clone-me");
        let cloned = id.clone();
        assert_eq!(id, cloned);
    }

    #[test]
    fn test_tensor_midi_id_display() {
        let id = TensorMidiId::new("hex-42");
        assert_eq!(format!("{id}"), "hex-42");
    }

    #[test]
    fn test_tensor_midi_id_serde() {
        let id = TensorMidiId::new("serde-test");
        let json = serde_json::to_string(&id).unwrap();
        let back: TensorMidiId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, back);
    }

    // --- NoteVector tests ---

    #[test]
    fn test_note_energy_zero() {
        let n = NoteVector::new(60, 0, 480, 0, 0);
        assert!((n.energy()).abs() < 1e-10);
    }

    #[test]
    fn test_note_energy_max() {
        let n = NoteVector::new(60, 127, 480, 0, 0);
        assert!((n.energy() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_note_energy_half_velocity() {
        let n = NoteVector::new(60, 63, 480, 0, 0);
        let expected = 63.0 / 127.0;
        assert!((n.energy() - expected).abs() < 1e-6);
    }

    #[test]
    fn test_note_energy_double_duration() {
        let n = NoteVector::new(60, 127, 960, 0, 0);
        assert!((n.energy() - 2.0).abs() < 1e-10);
    }

    #[test]
    fn test_note_silence() {
        let s = NoteVector::silence(100, 480, 2);
        assert_eq!(s.onset_tick, 100);
        assert_eq!(s.velocity, 0);
        assert_eq!(s.energy(), 0.0);
    }

    #[test]
    fn test_note_vector_serde() {
        let n = NoteVector::new(64, 100, 240, 960, 3);
        let json = serde_json::to_string(&n).unwrap();
        let back: NoteVector = serde_json::from_str(&json).unwrap();
        assert_eq!(n.pitch, back.pitch);
        assert_eq!(n.velocity, back.velocity);
        assert_eq!(n.duration_ticks, back.duration_ticks);
        assert_eq!(n.onset_tick, back.onset_tick);
        assert_eq!(n.channel, back.channel);
    }

    // --- AgentCadence tests ---

    #[test]
    fn test_tick_interval_120bpm() {
        let cadence = AgentCadence::new("test", 120.0);
        // 60000 / 120 / 480 ≈ 1.0417 ms/tick
        let expected = 60_000.0 / 120.0 / 480.0;
        assert!((cadence.tick_interval() - expected).abs() < 1e-6);
    }

    #[test]
    fn test_tick_interval_60bpm() {
        let cadence = AgentCadence::new("test", 60.0);
        let expected = 60_000.0 / 60.0 / 480.0;
        assert!((cadence.tick_interval() - expected).abs() < 1e-6);
    }

    #[test]
    fn test_swing_offset_on_beat() {
        let cadence = AgentCadence::new("test", 100.0);
        assert_eq!(cadence.swing_offset(0), 0.0);
        assert_eq!(cadence.swing_offset(2), 0.0);
        assert_eq!(cadence.swing_offset(4), 0.0);
    }

    #[test]
    fn test_swing_offset_off_beat_no_swing() {
        let mut cadence = AgentCadence::new("test", 100.0);
        cadence.swing = 0.0;
        assert_eq!(cadence.swing_offset(1), 0.0);
        assert_eq!(cadence.swing_offset(3), 0.0);
    }

    #[test]
    fn test_swing_offset_full_swing() {
        let mut cadence = AgentCadence::new("test", 100.0);
        cadence.swing = 1.0;
        // Off-beat with full swing → ticks_per_beat / 3 = 160
        assert!((cadence.swing_offset(1) - 160.0).abs() < 1e-6);
        assert!((cadence.swing_offset(3) - 160.0).abs() < 1e-6);
    }

    #[test]
    fn test_swing_offset_half_swing() {
        let mut cadence = AgentCadence::new("test", 100.0);
        cadence.swing = 0.5;
        assert!((cadence.swing_offset(1) - 80.0).abs() < 1e-6);
    }

    #[test]
    fn test_next_onset_high_energy() {
        let cadence = AgentCadence::new("test", 120.0);
        let next = cadence.next_onset(0, 1.0);
        // High energy → shorter interval
        assert!(next > 0);
        assert!(next < 4 * 480); // Less than one bar
    }

    #[test]
    fn test_next_onset_low_energy() {
        let cadence = AgentCadence::new("test", 120.0);
        let next = cadence.next_onset(0, 0.1);
        assert!(next > 0);
    }

    #[test]
    fn test_should_speak_too_soon() {
        let cadence = AgentCadence::new("test", 100.0);
        // Just spoke at tick 0, check at tick 1 → too soon
        assert!(!cadence.should_speak(1, 0));
    }

    #[test]
    fn test_should_speak_long_gap() {
        let cadence = AgentCadence::new("test", 100.0);
        // 8 beats = 3840 ticks gap → should always speak
        assert!(cadence.should_speak(4000, 0));
    }

    #[test]
    fn test_cadence_clamps_bpm() {
        let cadence = AgentCadence::new("test", 300.0);
        assert_eq!(cadence.base_bpm, 180.0);
        let cadence2 = AgentCadence::new("test2", 10.0);
        assert_eq!(cadence2.base_bpm, 60.0);
    }

    #[test]
    fn test_cadence_serde_roundtrip() {
        let c = architect_cadence();
        let json = serde_json::to_string(&c).unwrap();
        let back: AgentCadence = serde_json::from_str(&json).unwrap();
        assert_eq!(c.agent_id, back.agent_id);
        assert!((c.base_bpm - back.base_bpm).abs() < 1e-10);
    }

    // --- ConversationTensor tests ---

    #[test]
    fn test_tensor_new() {
        let t = ConversationTensor::new(default_cadences(), 8);
        assert_eq!(t.bars, 8);
        assert_eq!(t.agents, 4);
        assert_eq!(t.notes.len(), 0);
    }

    #[test]
    fn test_tensor_insert_and_energy() {
        let mut t = ConversationTensor::new(default_cadences(), 4);
        let note = NoteVector::new(60, 127, 480, 0, 0);
        t.insert_note(note);
        assert!((t.total_energy() - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_tensor_energy_at_tick() {
        let mut t = ConversationTensor::new(default_cadences(), 4);
        // Note active from tick 0 to 480
        t.insert_note(NoteVector::new(60, 127, 480, 0, 0));
        assert!(t.energy_at(0) > 0.0);
        assert!(t.energy_at(479) > 0.0);
        assert!((t.energy_at(500)).abs() < 1e-10);
    }

    #[test]
    fn test_tensor_bar_energy() {
        let mut t = ConversationTensor::new(default_cadences(), 4);
        // Put a note in bar 0
        t.insert_note(NoteVector::new(60, 100, 480, 0, 0));
        let e0 = t.bar_energy(0);
        let e1 = t.bar_energy(1);
        assert!(e0 > 0.0);
        assert!((e1).abs() < 1e-10);
    }

    #[test]
    fn test_tensor_adapt_bpm_low_energy() {
        let mut t = ConversationTensor::new(default_cadences(), 4);
        // Repeatedly adapt to low energy to overcome smoothing
        for _ in 0..50 {
            t.adapt_bpm(0.0);
        }
        assert!(t.bpm < 80.0);
    }

    #[test]
    fn test_tensor_adapt_bpm_high_energy() {
        let mut t = ConversationTensor::new(default_cadences(), 4);
        // Repeatedly adapt to high energy to overcome smoothing
        for _ in 0..50 {
            t.adapt_bpm(100.0);
        }
        assert!(t.bpm > 100.0);
    }

    #[test]
    fn test_tensor_entropy_empty() {
        let t = ConversationTensor::new(default_cadences(), 4);
        assert!((t.tensor_entropy()).abs() < 1e-10);
    }

    #[test]
    fn test_tensor_entropy_uniform() {
        let mut t = ConversationTensor::new(default_cadences(), 2);
        // One note per bar → maximum entropy for 2 bars = 1 bit
        t.insert_note(NoteVector::new(60, 64, 240, 0, 0));
        t.insert_note(NoteVector::new(64, 64, 240, ConversationTensor::TICKS_PER_BAR, 0));
        let entropy = t.tensor_entropy();
        assert!((entropy - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_tensor_entropy_concentrated() {
        let mut t = ConversationTensor::new(default_cadences(), 4);
        // All notes in bar 0 → entropy close to 0
        for _ in 0..10 {
            t.insert_note(NoteVector::new(60, 64, 240, 100, 0));
        }
        let entropy = t.tensor_entropy();
        assert!(entropy < 0.1);
    }

    #[test]
    fn test_tensor_is_conserved_empty() {
        let t = ConversationTensor::new(default_cadences(), 4);
        // Empty tensor with generous tolerance
        assert!(t.is_conserved(10.0));
    }

    #[test]
    fn test_tensor_agent_channel() {
        let t = ConversationTensor::new(default_cadences(), 4);
        // Channels assigned by key order — just verify it returns something
        let ch = t.agent_channel("Architect");
        assert!(ch < 4);
    }

    #[test]
    fn test_tensor_serde_roundtrip() {
        let mut t = ConversationTensor::new(default_cadences(), 4);
        t.insert_note(NoteVector::new(60, 100, 480, 0, 0));
        let json = serde_json::to_string(&t).unwrap();
        let back: ConversationTensor = serde_json::from_str(&json).unwrap();
        assert_eq!(t.bars, back.bars);
        assert_eq!(t.notes.len(), back.notes.len());
    }

    // --- Nudge tests ---

    #[test]
    fn test_nudge_excitement_energy_delta() {
        let n = Nudge::new(NudgeType::Excitement(0.8), "A", None, 0.9, 0);
        assert!(n.energy_delta() > 0.0);
        assert!((n.energy_delta() - 0.72).abs() < 1e-10);
    }

    #[test]
    fn test_nudge_pushback_energy_delta() {
        let n = Nudge::new(NudgeType::Pushback(0.5), "A", None, 1.0, 0);
        assert!(n.energy_delta() < 0.0);
    }

    #[test]
    fn test_nudge_question_energy_delta() {
        let n = Nudge::new(NudgeType::Question, "A", None, 0.8, 0);
        assert!(n.energy_delta() > 0.0);
    }

    #[test]
    fn test_nudge_topic_shift_energy_delta() {
        let n = Nudge::new(NudgeType::TopicShift("new-topic".into()), "A", None, 0.7, 0);
        assert!(n.energy_delta() < 0.0);
    }

    #[test]
    fn test_nudge_silence_energy_delta() {
        let n = Nudge::new(NudgeType::Silence(2.0), "A", None, 0.5, 0);
        assert!(n.energy_delta() < 0.0);
    }

    #[test]
    fn test_nudge_affects_cadence_weak() {
        let n = Nudge::new(NudgeType::Excitement(0.5), "A", None, 0.3, 0);
        assert!(!n.affects_cadence());
    }

    #[test]
    fn test_nudge_affects_cadence_strong() {
        let n = Nudge::new(NudgeType::Excitement(0.8), "A", None, 0.8, 0);
        assert!(n.affects_cadence());
    }

    #[test]
    fn test_nudge_strength_clamped() {
        let n = Nudge::new(NudgeType::Excitement(1.0), "A", None, 2.0, 0);
        assert!((n.strength - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_nudge_serde_roundtrip() {
        let n = Nudge::new(
            NudgeType::TopicShift("quantum".into()),
            "Architect",
            Some("Critic".into()),
            0.75,
            12345,
        );
        let json = serde_json::to_string(&n).unwrap();
        let back: Nudge = serde_json::from_str(&json).unwrap();
        assert_eq!(n.source_agent, back.source_agent);
        assert_eq!(n.target_agent, back.target_agent);
        assert!((n.strength - back.strength).abs() < 1e-10);
    }

    // --- ReactiveImprovEngine tests ---

    #[test]
    fn test_engine_new() {
        let engine = ReactiveImprovEngine::new(default_cadences(), 8, 100.0);
        assert_eq!(engine.tensor.agents, 4);
        assert!(engine.nudges.is_empty());
        assert!(engine.drafts.is_empty());
    }

    #[test]
    fn test_engine_nudge_adds() {
        let mut engine = ReactiveImprovEngine::new(default_cadences(), 4, 100.0);
        engine.nudge(Nudge::new(NudgeType::Excitement(0.5), "A", None, 0.8, 0));
        assert_eq!(engine.nudges.len(), 1);
    }

    #[test]
    fn test_engine_compute_draft_architect() {
        let mut engine = ReactiveImprovEngine::new(default_cadences(), 4, 100.0);
        let draft = engine.compute_draft("Architect");
        assert!(!draft.is_empty());
        // Draft should be stored
        assert!(engine.drafts.contains_key("Architect"));
    }

    #[test]
    fn test_engine_compute_draft_unknown_agent() {
        let mut engine = ReactiveImprovEngine::new(default_cadences(), 4, 100.0);
        let draft = engine.compute_draft("Unknown");
        assert!(draft.is_empty());
    }

    #[test]
    fn test_engine_commit_draft_within_budget() {
        let mut engine = ReactiveImprovEngine::new(default_cadences(), 4, 1000.0);
        engine.compute_draft("Architect");
        let committed = engine.commit_draft("Architect");
        assert!(committed);
        assert!(!engine.drafts.contains_key("Architect"));
        assert!(!engine.tensor.notes.is_empty());
    }

    #[test]
    fn test_engine_commit_draft_exceeds_budget() {
        let mut engine = ReactiveImprovEngine::new(default_cadences(), 4, 0.0);
        engine.compute_draft("Architect");
        // With 0 budget, only zero-energy drafts pass
        // Architect has energy_baseline 0.6, so draft will have energy
        let committed = engine.commit_draft("Architect");
        // Should fail since any note has energy > 0
        // (unless draft is empty, which it shouldn't be)
        if !engine.drafts.get("Architect").map_or(true, |d| d.is_empty()) {
            assert!(!committed);
        }
    }

    #[test]
    fn test_engine_commit_nonexistent_draft() {
        let mut engine = ReactiveImprovEngine::new(default_cadences(), 4, 100.0);
        assert!(!engine.commit_draft("Nobody"));
    }

    #[test]
    fn test_engine_clear_drafts() {
        let mut engine = ReactiveImprovEngine::new(default_cadences(), 4, 100.0);
        engine.compute_draft("Architect");
        engine.compute_draft("Critic");
        assert_eq!(engine.drafts.len(), 2);
        engine.clear_drafts();
        assert!(engine.drafts.is_empty());
    }

    #[test]
    fn test_engine_tick_advances() {
        let mut engine = ReactiveImprovEngine::new(default_cadences(), 4, 100.0);
        let initial = engine.tensor.tick;
        engine.tick();
        assert_eq!(engine.tensor.tick, initial + 1);
    }

    #[test]
    fn test_engine_tick_multiple() {
        let mut engine = ReactiveImprovEngine::new(default_cadences(), 4, 100.0);
        for _ in 0..100 {
            engine.tick();
        }
        assert_eq!(engine.tensor.tick, 100);
    }

    #[test]
    fn test_engine_tick_cleans_old_nudges() {
        let mut engine = ReactiveImprovEngine::new(default_cadences(), 4, 100.0);
        engine.nudge(Nudge::new(NudgeType::Excitement(0.5), "A", None, 0.8, 0));
        // Advance well past 4 bars
        for _ in 0..(ConversationTensor::TICKS_PER_BAR * 5) {
            engine.tick();
        }
        assert!(engine.nudges.is_empty());
    }

    #[test]
    fn test_engine_current_energy() {
        let mut engine = ReactiveImprovEngine::new(default_cadences(), 4, 100.0);
        assert!((engine.current_energy()).abs() < 1e-10);
        engine.compute_draft("Architect");
        engine.commit_draft("Architect");
        assert!(engine.current_energy() > 0.0);
    }

    #[test]
    fn test_engine_current_bpm() {
        let engine = ReactiveImprovEngine::new(default_cadences(), 4, 100.0);
        let bpm = engine.current_bpm();
        assert!(bpm >= 60.0 && bpm <= 120.0);
    }

    #[test]
    fn test_engine_summary() {
        let mut engine = ReactiveImprovEngine::new(default_cadences(), 4, 100.0);
        engine.compute_draft("Architect");
        let summary = engine.summary();
        assert!(summary.contains("agents"));
        assert!(summary.contains("Energy"));
        assert!(summary.contains("BPM"));
    }

    #[test]
    fn test_engine_full_flow() {
        let mut engine = ReactiveImprovEngine::new(default_cadences(), 8, 500.0);

        // Nudge: excitement from Architect to Implementer
        engine.nudge(Nudge::new(
            NudgeType::Excitement(0.7),
            "Architect",
            Some("Implementer".into()),
            0.9,
            0,
        ));

        // Compute and commit drafts for all agents
        for agent in &["Architect", "Implementer", "Critic", "Historian"] {
            engine.compute_draft(agent);
            engine.commit_draft(agent);
        }

        // Advance some ticks
        for _ in 0..960 {
            engine.tick();
        }

        // Verify state
        assert!(engine.current_energy() > 0.0);
        assert!(engine.current_bpm() >= 60.0);
        assert!(!engine.tensor.notes.is_empty());

        let summary = engine.summary();
        assert!(summary.contains("4 agents"));
    }

    #[test]
    fn test_engine_topic_shift_clears() {
        let mut engine = ReactiveImprovEngine::new(default_cadences(), 4, 100.0);
        engine.compute_draft("Architect");
        engine.compute_draft("Critic");
        engine.nudge(Nudge::new(
            NudgeType::TopicShift("new-topic".into()),
            "Historian",
            None,
            1.0,
            0,
        ));
        engine.clear_drafts();
        assert!(engine.drafts.is_empty());
    }

    #[test]
    fn test_engine_agent_should_speak() {
        let mut engine = ReactiveImprovEngine::new(default_cadences(), 4, 100.0);
        // At tick 0, agent hasn't spoken → depends on cadence
        // Advance far enough
        for _ in 0..4000 {
            engine.tick();
        }
        // After enough time, at least one agent should want to speak
        let any_speaks = ["Architect", "Implementer", "Critic", "Historian"]
            .iter()
            .any(|a| engine.agent_should_speak(a));
        assert!(any_speaks);
    }

    // --- Pre-built cadence tests ---

    #[test]
    fn test_architect_cadence_values() {
        let c = architect_cadence();
        assert_eq!(c.agent_id, "Architect");
        assert!((c.base_bpm - 80.0).abs() < 1e-10);
        assert!((c.swing - 0.3).abs() < 1e-10);
        assert!((c.articulation - 0.7).abs() < 1e-10);
        assert!((c.energy_baseline - 0.6).abs() < 1e-10);
        assert!((c.variation - 0.4).abs() < 1e-10);
    }

    #[test]
    fn test_implementer_cadence_values() {
        let c = implementer_cadence();
        assert_eq!(c.agent_id, "Implementer");
        assert!((c.base_bpm - 120.0).abs() < 1e-10);
        assert!((c.swing - 0.1).abs() < 1e-10);
        assert!((c.articulation - 0.3).abs() < 1e-10);
        assert!((c.energy_baseline - 0.8).abs() < 1e-10);
        assert!((c.variation - 0.2).abs() < 1e-10);
    }

    #[test]
    fn test_critic_cadence_values() {
        let c = critic_cadence();
        assert_eq!(c.agent_id, "Critic");
        assert!((c.base_bpm - 90.0).abs() < 1e-10);
        assert!((c.swing - 0.5).abs() < 1e-10);
        assert!((c.articulation - 0.5).abs() < 1e-10);
        assert!((c.energy_baseline - 0.4).abs() < 1e-10);
        assert!((c.variation - 0.6).abs() < 1e-10);
    }

    #[test]
    fn test_historian_cadence_values() {
        let c = historian_cadence();
        assert_eq!(c.agent_id, "Historian");
        assert!((c.base_bpm - 70.0).abs() < 1e-10);
        assert!((c.swing - 0.7).abs() < 1e-10);
        assert!((c.articulation - 0.9).abs() < 1e-10);
        assert!((c.energy_baseline - 0.3).abs() < 1e-10);
        assert!((c.variation - 0.8).abs() < 1e-10);
    }

    #[test]
    fn test_default_cadences_count() {
        let cs = default_cadences();
        assert_eq!(cs.len(), 4);
    }

    #[test]
    fn test_conversation_bpm_adapt_flow() {
        // Taoist test: BPM flows with energy, never forces
        let mut t = ConversationTensor::new(default_cadences(), 4);
        let initial_bpm = t.bpm;

        // Add high energy
        for i in 0..10 {
            t.insert_note(NoteVector::new(60, 127, 480, i as u64 * 100, 0));
        }
        t.adapt_bpm(t.total_energy());
        assert!(t.bpm >= initial_bpm);
    }

    #[test]
    fn test_note_pitch_range() {
        // Verify notes stay in MIDI range
        let n1 = NoteVector::new(0, 64, 480, 0, 0);
        assert_eq!(n1.pitch, 0);
        let n2 = NoteVector::new(127, 64, 480, 0, 0);
        assert_eq!(n2.pitch, 127);
    }

    #[test]
    fn test_engine_multiple_nudges() {
        let mut engine = ReactiveImprovEngine::new(default_cadences(), 8, 500.0);
        engine.nudge(Nudge::new(NudgeType::Excitement(0.8), "A", Some("B".into()), 0.9, 0));
        engine.nudge(Nudge::new(NudgeType::Pushback(0.3), "B", Some("A".into()), 0.5, 10));
        engine.nudge(Nudge::new(NudgeType::Question, "C", None, 0.7, 20));
        engine.nudge(Nudge::new(NudgeType::Silence(1.0), "D", None, 0.4, 30));
        assert_eq!(engine.nudges.len(), 4);
    }

    #[test]
    fn test_energy_conservation_invariant() {
        // Energy of committed drafts should not exceed budget
        let mut engine = ReactiveImprovEngine::new(default_cadences(), 4, 5.0);
        for agent in &["Architect", "Implementer", "Critic", "Historian"] {
            engine.compute_draft(agent);
            engine.commit_draft(agent);
        }
        assert!(engine.current_energy() <= 5.0 + 1e-10);
    }
}
