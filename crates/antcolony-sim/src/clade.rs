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
