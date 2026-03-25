//! # Brique V : Synthèse Sensorielle (R2D2 Sensory)
//!
//! Cette crate agit comme le système nerveux périphérique de l'écosystème R2D2.
//! Elle intercepte les stimuli du monde physique (Fichiers locaux, Audio, Vidéo)
//! et les aiguille vers les Agents Corticaux spécialisés (Vision, Ouïe) pour traitement.

pub mod gateway;
pub mod stimulus;
