//! # Brique 6 : R2D2-Chunker (Moteur Universel d'Ingestion)
//!
//! Cette bibliothèque implémente la séparation des préoccupations pour l'ingestion IA.
//! Les agents (ex: Whisper, LLaVA) doivent recevoir des fragments parfaits et formatés,
//! sans avoir à gérer eux-mêmes le traitement du signal, la vidéo ou les flux PDF.
//! Le `Chunker` est la moulinette industrielle avant le Cortex.

pub mod strategy;
pub mod audio_chunker;

pub use strategy::MediaChunker;
pub use audio_chunker::AudioChunker;
