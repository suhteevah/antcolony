//! Ant subfamily clade classification + the venom×defender susceptibility
//! matrix used by cross-species combat. Pure functions, no sim state.
//! Grounded in docs/biology/interspecific/02-combat-mechanics.md §3
//! (clade-specific chemical weapons; Greenberg 2008 684× resistance span,
//! tamed to an in-game spread).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Clade {
    /// Default / genus not recognized — neutral in the venom matrix.
    #[default]
    Unknown,
    /// Ponerinae — functional sting, protein venom (Brachyponera).
    Ponerinae,
    /// Formicinae — formic acid, no sting (Formica, Camponotus, Lasius).
    Formicinae,
    /// Myrmicinae — sting/alkaloid (Aphaenogaster, Pogonomyrmex, Tetramorium, Temnothorax).
    Myrmicinae,
    /// Dolichoderinae — iridoids (Tapinoma, Linepithema).
    Dolichoderinae,
}

use crate::species_extended::Weapon;

/// Map a species `genus` string to its subfamily clade. Case-insensitive
/// on the leading genus token. Unknown genera return `Clade::Unknown`
/// (neutral in the venom matrix).
pub fn clade_from_genus(genus: &str) -> Clade {
    match genus.trim().to_ascii_lowercase().as_str() {
        "brachyponera" | "pachycondyla" | "ponera" | "platythyrea" | "diacamma" => Clade::Ponerinae,
        "formica" | "camponotus" | "lasius" | "nylanderia" | "oecophylla" | "polyergus" => {
            Clade::Formicinae
        }
        "aphaenogaster" | "pogonomyrmex" | "tetramorium" | "temnothorax" | "solenopsis"
        | "myrmica" | "crematogaster" | "pheidole" | "atta" => Clade::Myrmicinae,
        "tapinoma" | "linepithema" | "dolichoderus" | "iridomyrmex" => Clade::Dolichoderinae,
        _ => Clade::Unknown,
    }
}

/// In-game venom susceptibility multiplier for an attacker's `weapon`
/// (scaled by its `attacker_sting_potency`) against a `defender` clade.
///
/// Literature LD50 spans are 330–684× (Greenberg 2008; LeBrun 2014) — far
/// too steep to play. We collapse that to a tame [1.0, 2.0] spread:
/// chemically-armed attackers (Sting/FormicSpray) get an edge ONLY against
/// clades naive to that chemistry; same-clade / Mandible / Unknown = 1.0.
/// `[cite: 02 §3; 05 Finding 21]`
pub fn venom_multiplier(weapon: Weapon, attacker_sting_potency: f32, defender: Clade) -> f32 {
    const MAX_MULT: f32 = 2.0;
    let naive = |d: Clade| matches!(d, Clade::Myrmicinae | Clade::Dolichoderinae);
    match weapon {
        // Ponerine protein-venom sting: edge vs naive clades, scaled by
        // Schmidt-scale potency (B. chinensis 1.5 -> ~1.5×; capped at 2.0).
        Weapon::Sting if attacker_sting_potency > 0.0 && naive(defender) => {
            (1.0 + attacker_sting_potency * 0.5).clamp(1.0, MAX_MULT)
        }
        // Formicine acid contact toxin: flat elevated edge vs naive clades.
        Weapon::FormicSpray if naive(defender) => 1.5,
        // Same clade, mandible-only, unknown, or experienced defender.
        _ => 1.0,
    }
}

/// The clade this clade is chemically STRONG against, in the cyclic type-chart
/// `Ponerinae ▸ Formicinae ▸ Dolichoderinae ▸ Myrmicinae ▸ Ponerinae`.
/// `Unknown` beats nothing. This is the (game-design) rock-paper-scissors closure
/// over the 4 subfamilies — grounded loosely in the fact that ant chemical
/// ecology is genuinely non-transitive (no single venom/defense dominates all):
/// ponerine protein sting overwhelms soft formicine cuticle; formic acid swamps
/// dolichoderine iridoid glands; dolichoderine repellent iridoids rout myrmicine
/// raiders; myrmicine alkaloid stings are resistant to / counter ponerine venom.
/// `[cite: 02 §3 chemical weapons; design idealization — see docs/biology.md]`
fn clade_beats(a: Clade) -> Clade {
    match a {
        Clade::Ponerinae => Clade::Formicinae,
        Clade::Formicinae => Clade::Dolichoderinae,
        Clade::Dolichoderinae => Clade::Myrmicinae,
        Clade::Myrmicinae => Clade::Ponerinae,
        Clade::Unknown => Clade::Unknown,
    }
}

/// Cyclic clade type-chart damage multiplier for an `attacker` clade striking a
/// `defender` clade. `strength` is the amplify factor (e.g. `1.8`): the attacker
/// deals `strength`× against the clade it beats, `1/strength`× against the clade
/// that beats it, and `1.0` against itself or the remaining (opposite) clade.
/// `strength <= 0` (default) ⇒ DISABLED, returns `1.0` everywhere so the matrix
/// reduces to the legacy `venom_multiplier` path (byte-identical for same-clade /
/// single-colony sims, which are always `1.0` here too). `Unknown` ⇒ neutral.
///
/// Unlike `venom_multiplier` (strict "armed beats naive" dominance ⇒ transitive),
/// this produces intransitive cycles: A>B, B>C, C>D, D>A across the 4 subfamilies.
pub fn clade_cycle_multiplier(attacker: Clade, defender: Clade, strength: f32) -> f32 {
    if strength <= 0.0
        || attacker == defender
        || matches!(attacker, Clade::Unknown)
        || matches!(defender, Clade::Unknown)
    {
        return 1.0;
    }
    let s = strength.max(1.0);
    if clade_beats(attacker) == defender {
        s // attacker is strong against this defender
    } else if clade_beats(defender) == attacker {
        1.0 / s // the defender's clade beats the attacker's ⇒ attenuated
    } else {
        1.0 // opposite clade in the 4-cycle ⇒ neutral
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::species_extended::Weapon;

    #[test]
    fn genus_maps_to_clade() {
        assert_eq!(clade_from_genus("Brachyponera"), Clade::Ponerinae);
        assert_eq!(clade_from_genus("Formica"), Clade::Formicinae);
        assert_eq!(clade_from_genus("Camponotus"), Clade::Formicinae);
        assert_eq!(clade_from_genus("Lasius"), Clade::Formicinae);
        assert_eq!(clade_from_genus("Aphaenogaster"), Clade::Myrmicinae);
        assert_eq!(clade_from_genus("Pogonomyrmex"), Clade::Myrmicinae);
        assert_eq!(clade_from_genus("Tetramorium"), Clade::Myrmicinae);
        assert_eq!(clade_from_genus("Temnothorax"), Clade::Myrmicinae);
        assert_eq!(clade_from_genus("Tapinoma"), Clade::Dolichoderinae);
        assert_eq!(clade_from_genus("Nonsense"), Clade::Unknown);
    }

    #[test]
    fn venom_matrix_rewards_ponerine_sting_vs_naive_myrmicine() {
        // B. chinensis (Ponerine, sting_potency 1.5) vs A. rudis (Myrmicinae).
        let m = venom_multiplier(Weapon::Sting, 1.5, Clade::Myrmicinae);
        assert!(m > 1.0, "ponerine sting vs naive myrmicine should exceed 1.0, got {m}");
        assert!(m <= 2.0, "in-game cap is 2.0, got {m}");
    }

    #[test]
    fn venom_matrix_is_neutral_for_mandible_and_same_clade() {
        assert_eq!(venom_multiplier(Weapon::Mandible, 5.0, Clade::Myrmicinae), 1.0);
        // Ponerine sting vs Ponerine = experienced, no edge.
        assert_eq!(venom_multiplier(Weapon::Sting, 1.5, Clade::Ponerinae), 1.0);
        // Unknown defender = neutral.
        assert_eq!(venom_multiplier(Weapon::Sting, 1.5, Clade::Unknown), 1.0);
    }

    #[test]
    fn venom_matrix_zero_potency_sting_is_neutral() {
        // sting weapon but no potency => no chemical edge.
        assert_eq!(venom_multiplier(Weapon::Sting, 0.0, Clade::Myrmicinae), 1.0);
    }

    #[test]
    fn formic_spray_elevated_vs_myrmicine() {
        let m = venom_multiplier(Weapon::FormicSpray, 0.0, Clade::Myrmicinae);
        assert!(m > 1.0 && m <= 2.0, "formic spray vs myrmicine in (1.0, 2.0], got {m}");
    }

    #[test]
    fn clade_cycle_disabled_is_neutral() {
        // strength 0 ⇒ 1.0 everywhere (byte-identical legacy path).
        for a in [Clade::Ponerinae, Clade::Formicinae, Clade::Dolichoderinae, Clade::Myrmicinae] {
            for d in [Clade::Ponerinae, Clade::Formicinae, Clade::Dolichoderinae, Clade::Myrmicinae] {
                assert_eq!(clade_cycle_multiplier(a, d, 0.0), 1.0);
            }
        }
    }

    #[test]
    fn clade_cycle_is_intransitive_rock_paper_scissors() {
        let s = 1.8_f32;
        // The 4-cycle Pon ▸ Form ▸ Dol ▸ Myr ▸ Pon: each amplified vs the next.
        assert_eq!(clade_cycle_multiplier(Clade::Ponerinae, Clade::Formicinae, s), s);
        assert_eq!(clade_cycle_multiplier(Clade::Formicinae, Clade::Dolichoderinae, s), s);
        assert_eq!(clade_cycle_multiplier(Clade::Dolichoderinae, Clade::Myrmicinae, s), s);
        assert_eq!(clade_cycle_multiplier(Clade::Myrmicinae, Clade::Ponerinae, s), s);
        // Reverse direction is attenuated (the loser of each pairing).
        assert!((clade_cycle_multiplier(Clade::Formicinae, Clade::Ponerinae, s) - 1.0 / s).abs() < 1e-6);
        assert!((clade_cycle_multiplier(Clade::Ponerinae, Clade::Myrmicinae, s) - 1.0 / s).abs() < 1e-6);
        // Opposite clade in the cycle ⇒ neutral (Pon vs Dol, Form vs Myr).
        assert_eq!(clade_cycle_multiplier(Clade::Ponerinae, Clade::Dolichoderinae, s), 1.0);
        assert_eq!(clade_cycle_multiplier(Clade::Formicinae, Clade::Myrmicinae, s), 1.0);
        // Same clade + Unknown ⇒ neutral.
        assert_eq!(clade_cycle_multiplier(Clade::Ponerinae, Clade::Ponerinae, s), 1.0);
        assert_eq!(clade_cycle_multiplier(Clade::Unknown, Clade::Myrmicinae, s), 1.0);
        // No clade dominates all: each beats exactly one, loses to exactly one.
        for a in [Clade::Ponerinae, Clade::Formicinae, Clade::Dolichoderinae, Clade::Myrmicinae] {
            let wins = [Clade::Ponerinae, Clade::Formicinae, Clade::Dolichoderinae, Clade::Myrmicinae]
                .iter().filter(|&&d| clade_cycle_multiplier(a, d, s) > 1.0).count();
            let losses = [Clade::Ponerinae, Clade::Formicinae, Clade::Dolichoderinae, Clade::Myrmicinae]
                .iter().filter(|&&d| clade_cycle_multiplier(a, d, s) < 1.0).count();
            assert_eq!(wins, 1, "each clade beats exactly one");
            assert_eq!(losses, 1, "each clade loses to exactly one");
        }
    }
}
