//! # Décodeur Audio Symphonia
//! 
//! Extrait physiquement les flux OGG/WAV en échantillons Float32 Mono (16kHz).
//! Requis pour la projection Mel-Spectrogram de l'agent Whisper.

use crate::error::CortexError;
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::errors::Error;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

pub fn decode_audio_to_pcm(path: &str) -> Result<Vec<f32>, CortexError> {
    let file = std::fs::File::open(path)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    // Hint optionnel basé sur l'extension
    if path.ends_with("ogg") {
        hint.with_extension("ogg");
    } else if path.ends_with("wav") {
        hint.with_extension("wav");
    }

    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &fmt_opts, &meta_opts)
        .map_err(|e| CortexError::ComponentDecouplingError(format!("Erreur Probe Symphonia: {:?}", e)))?;

    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| CortexError::ComponentDecouplingError("Aucune piste audio exploitable trouvée.".into()))?;

    let dec_opts: DecoderOptions = Default::default();
    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &dec_opts)
        .map_err(|e| CortexError::ComponentDecouplingError(format!("Erreur Codec Symphonia: {:?}", e)))?;

    let track_id = track.id;
    let mut sample_buf = None;
    let mut pcm_data = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            // Symphonia Loop Fix: Si Erreur (Fin de fichier ou Flux Corrompu), on arrête d'extraire. 
            // `continue` sur une IO Error bloque le CPU à 100%.
            Err(_) => break,
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(audio_buf) => {
                let spec = *audio_buf.spec();
                let duration = audio_buf.capacity() as u64;

                // Initialiser le buffer d'échantillons si besoin
                if sample_buf.is_none() {
                    sample_buf = Some(SampleBuffer::<f32>::new(duration, spec));
                }

                if let Some(buf) = &mut sample_buf {
                    buf.copy_interleaved_ref(audio_buf);
                    
                    // Récupération des frames entrelacées (On suppose du mono pour Whisper,
                    // sinon on fera la moyenne des canaux à l'avenir)
                    for sample in buf.samples() {
                        pcm_data.push(*sample);
                    }
                }
            }
            Err(Error::DecodeError(_)) => continue, // Le paquet est passé, on peut l'ignorer
            Err(_) => break,
        }
    }

    Ok(pcm_data)
}
