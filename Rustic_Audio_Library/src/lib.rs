mod record;
mod playback;
mod dsp;
mod opus_encoder;
mod opus_playback;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::sync::Mutex;
use crate::dsp::AudioProcessor;
use crate::opus_encoder::OpusEncoder;
use crate::record::record_audio;
use crate::playback::playback_audio;
use crate::opus_playback::playback_opus;

#[derive(Clone)]
pub struct AudioFileInfo {
    pub file_size: u64,
    pub duration: f64,
    pub original_wav_size: u64,
    pub unprocessed_opus_size: u64,
    pub processed_opus_size: u64,
    pub last_message: String,
}

pub struct RusticAudio {
    is_recording: Arc<AtomicBool>,
    is_playing: Arc<AtomicBool>,
    is_playing_original: Arc<AtomicBool>,
    is_playing_unprocessed_opus: Arc<AtomicBool>,
    recording_thread: Option<thread::JoinHandle<()>>,
    playback_thread: Option<thread::JoinHandle<()>>,
    playback_original_thread: Option<thread::JoinHandle<()>>,
    playback_unprocessed_opus_thread: Option<thread::JoinHandle<()>>,
    audio_info: Arc<Mutex<AudioFileInfo>>,
    pub processor: AudioProcessor,
    pub opus_encoder: OpusEncoder,
}

impl Default for RusticAudio {
    fn default() -> Self {
        Self {
            is_recording: Arc::new(AtomicBool::new(false)),
            is_playing: Arc::new(AtomicBool::new(false)),
            is_playing_original: Arc::new(AtomicBool::new(false)),
            is_playing_unprocessed_opus: Arc::new(AtomicBool::new(false)),
            recording_thread: None,
            playback_thread: None,
            playback_original_thread: None,
            playback_unprocessed_opus_thread: None,
            audio_info: Arc::new(Mutex::new(AudioFileInfo {
                file_size: 0,
                duration: 0.0,
                original_wav_size: 0,
                unprocessed_opus_size: 0,
                processed_opus_size: 0,
                last_message: String::new(),
            })),
            processor: AudioProcessor::new(44100.0),
            opus_encoder: OpusEncoder::new(),
        }
    }
}

impl RusticAudio {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start_recording(&mut self, output_path: &str) -> Result<(), String> {
        if self.is_recording.load(Ordering::Relaxed) || 
           self.is_playing.load(Ordering::Relaxed) || 
           self.is_playing_original.load(Ordering::Relaxed) || 
           self.is_playing_unprocessed_opus.load(Ordering::Relaxed) {
            return Err("Another operation is already in progress".to_string());
        }

        let is_recording = Arc::clone(&self.is_recording);
        let audio_info = Arc::clone(&self.audio_info);
        let processor = self.processor.clone();
        let opus_encoder = self.opus_encoder.clone();
        let output_path = output_path.to_string();
        
        self.is_recording.store(true, Ordering::Relaxed);
        self.recording_thread = Some(thread::spawn(move || {
            if let Ok(_) = record_audio(&output_path, is_recording, processor.clone()) {
                let mut info = audio_info.lock().unwrap();
                info.last_message = "Recording completed successfully".to_string();
                
                // Copy output.wav to original.wav
                let original_path = format!("{}_original.wav", output_path.trim_end_matches(".wav"));
                if let Err(e) = std::fs::copy(&output_path, &original_path) {
                    info.last_message = format!("Error copying to original file: {:?}", e);
                    return;
                }
                
                // Update original WAV file size
                if let Ok(metadata) = std::fs::metadata(&original_path) {
                    info.original_wav_size = metadata.len();
                }
                
                // Process audio
                let mut processor_instance = processor;
                let processed_path = format!("{}_processed.wav", output_path.trim_end_matches(".wav"));
                if let Err(e) = processor_instance.process_file(&output_path, &processed_path) {
                    info.last_message = format!("Error processing audio: {:?}", e);
                    return;
                }
                
                // Encode to Opus
                let processed_opus_path = format!("{}_processed.opus", output_path.trim_end_matches(".wav"));
                if let Err(e) = opus_encoder.encode_wav_to_opus(&processed_path, &processed_opus_path) {
                    info.last_message = format!("Error encoding to Opus: {:?}", e);
                } else {
                    // Update file info after successful encoding
                    match opus_playback::get_opus_info(&processed_opus_path) {
                        Ok((size, duration)) => {
                            info.file_size = size;
                            info.processed_opus_size = size;
                            info.duration = duration;
                            info.last_message = "Processing and Opus encoding completed successfully".to_string();
                        }
                        Err(e) => {
                            info.last_message = format!("Error getting Opus file info: {:?}", e);
                        }
                    }
                }
                
                // Also encode original to opus for comparison
                let unprocessed_opus_path = format!("{}_unprocessed.opus", output_path.trim_end_matches(".wav"));
                if let Err(e) = opus_encoder.encode_wav_to_opus(&original_path, &unprocessed_opus_path) {
                    info.last_message = format!("Error encoding unprocessed audio: {:?}", e);
                } else {
                    // Update unprocessed opus file size
                    if let Ok(metadata) = std::fs::metadata(&unprocessed_opus_path) {
                        info.unprocessed_opus_size = metadata.len();
                    }
                }
            }
        }));

        Ok(())
    }

    pub fn stop_recording(&mut self) -> Result<(), String> {
        if !self.is_recording.load(Ordering::Relaxed) {
            return Err("Not currently recording".to_string());
        }
        
        self.is_recording.store(false, Ordering::Relaxed);
        
        // Wait for recording thread to finish
        if let Some(thread) = self.recording_thread.take() {
            if thread.join().is_err() {
                return Err("Failed to join recording thread".to_string());
            }
        }
        
        Ok(())
    }

    pub fn play_original_wav(&mut self, file_path: &str) -> Result<(), String> {
        if self.is_recording.load(Ordering::Relaxed) || 
           self.is_playing.load(Ordering::Relaxed) || 
           self.is_playing_original.load(Ordering::Relaxed) || 
           self.is_playing_unprocessed_opus.load(Ordering::Relaxed) {
            return Err("Another operation is already in progress".to_string());
        }
        
        let is_playing = Arc::clone(&self.is_playing_original);
        let audio_info = Arc::clone(&self.audio_info);
        let file_path = file_path.to_string();
        
        self.is_playing_original.store(true, Ordering::Relaxed);
        self.playback_original_thread = Some(thread::spawn(move || {
            match playback_audio(&file_path, is_playing) {
                Ok(_) => {
                    let mut info = audio_info.lock().unwrap();
                    info.last_message = "Original playback completed successfully".to_string();
                },
                Err(e) => {
                    let mut info = audio_info.lock().unwrap();
                    info.last_message = format!("Error during original playback: {:?}", e);
                },
            }
        }));
        
        Ok(())
    }

    pub fn play_processed_wav(&mut self, file_path: &str) -> Result<(), String> {
        if self.is_recording.load(Ordering::Relaxed) || 
           self.is_playing.load(Ordering::Relaxed) || 
           self.is_playing_original.load(Ordering::Relaxed) || 
           self.is_playing_unprocessed_opus.load(Ordering::Relaxed) {
            return Err("Another operation is already in progress".to_string());
        }
        
        let is_playing = Arc::clone(&self.is_playing);
        let audio_info = Arc::clone(&self.audio_info);
        let file_path = file_path.to_string();
        
        self.is_playing.store(true, Ordering::Relaxed);
        self.playback_thread = Some(thread::spawn(move || {
            match playback_audio(&file_path, is_playing) {
                Ok(_) => {
                    let mut info = audio_info.lock().unwrap();
                    info.last_message = "Processed WAV playback completed successfully".to_string();
                },
                Err(e) => {
                    let mut info = audio_info.lock().unwrap();
                    info.last_message = format!("Error during processed WAV playback: {:?}", e);
                },
            }
        }));
        
        Ok(())
    }

    pub fn play_unprocessed_opus(&mut self, file_path: &str) -> Result<(), String> {
        if self.is_recording.load(Ordering::Relaxed) || 
           self.is_playing.load(Ordering::Relaxed) || 
           self.is_playing_original.load(Ordering::Relaxed) || 
           self.is_playing_unprocessed_opus.load(Ordering::Relaxed) {
            return Err("Another operation is already in progress".to_string());
        }
        
        let is_playing = Arc::clone(&self.is_playing_unprocessed_opus);
        let audio_info = Arc::clone(&self.audio_info);
        let file_path = file_path.to_string();
        
        self.is_playing_unprocessed_opus.store(true, Ordering::Relaxed);
        self.playback_unprocessed_opus_thread = Some(thread::spawn(move || {
            match playback_opus(&file_path, is_playing) {
                Ok(_) => {
                    let mut info = audio_info.lock().unwrap();
                    info.last_message = "Unprocessed opus playback completed successfully".to_string();
                },
                Err(e) => {
                    let mut info = audio_info.lock().unwrap();
                    info.last_message = format!("Error during unprocessed opus playback: {:?}", e);
                },
            }
        }));
        
        Ok(())
    }

    pub fn play_processed_opus(&mut self, file_path: &str) -> Result<(), String> {
        if self.is_recording.load(Ordering::Relaxed) || 
           self.is_playing.load(Ordering::Relaxed) || 
           self.is_playing_original.load(Ordering::Relaxed) || 
           self.is_playing_unprocessed_opus.load(Ordering::Relaxed) {
            return Err("Another operation is already in progress".to_string());
        }
        
        let is_playing = Arc::clone(&self.is_playing);
        let audio_info = Arc::clone(&self.audio_info);
        let file_path = file_path.to_string();
        
        self.is_playing.store(true, Ordering::Relaxed);
        self.playback_thread = Some(thread::spawn(move || {
            match playback_opus(&file_path, is_playing) {
                Ok(_) => {
                    let mut info = audio_info.lock().unwrap();
                    info.last_message = "Processed opus playback completed successfully".to_string();
                },
                Err(e) => {
                    let mut info = audio_info.lock().unwrap();
                    info.last_message = format!("Error during processed opus playback: {:?}", e);
                },
            }
        }));
        
        Ok(())
    }

    pub fn stop_playback(&mut self) -> Result<(), String> {
        if self.is_playing.load(Ordering::Relaxed) {
            self.is_playing.store(false, Ordering::Relaxed);
            if let Some(thread) = self.playback_thread.take() {
                if thread.join().is_err() {
                    return Err("Failed to join playback thread".to_string());
                }
            }
        }
        
        if self.is_playing_original.load(Ordering::Relaxed) {
            self.is_playing_original.store(false, Ordering::Relaxed);
            if let Some(thread) = self.playback_original_thread.take() {
                if thread.join().is_err() {
                    return Err("Failed to join original playback thread".to_string());
                }
            }
        }
        
        if self.is_playing_unprocessed_opus.load(Ordering::Relaxed) {
            self.is_playing_unprocessed_opus.store(false, Ordering::Relaxed);
            if let Some(thread) = self.playback_unprocessed_opus_thread.take() {
                if thread.join().is_err() {
                    return Err("Failed to join unprocessed opus playback thread".to_string());
                }
            }
        }
        
        Ok(())
    }

    pub fn get_audio_info(&self) -> AudioFileInfo {
        self.audio_info.lock().unwrap().clone()
    }

    pub fn set_opus_bitrate(&mut self, bitrate: i32) {
        self.opus_encoder.set_bitrate(bitrate);
    }

    pub fn get_opus_bitrate(&self) -> i32 {
        self.opus_encoder.get_bitrate()
    }

    pub fn process_file(&mut self, input_path: &str, output_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.processor.process_file(input_path, output_path)
    }

    pub fn encode_to_opus(&self, input_path: &str, output_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.opus_encoder.encode_wav_to_opus(input_path, output_path)
    }

    pub fn is_recording(&self) -> bool {
        self.is_recording.load(Ordering::Relaxed)
    }

    pub fn is_playing(&self) -> bool {
        self.is_playing.load(Ordering::Relaxed) || 
        self.is_playing_original.load(Ordering::Relaxed) || 
        self.is_playing_unprocessed_opus.load(Ordering::Relaxed)
    }
}

// Example of how to use the library
fn main() {
    println!("RusticAudio library - Example usage");
    
    // Create a new instance of RusticAudio
    let mut audio = RusticAudio::new();
    
    // Configure audio processing parameters
    audio.processor.threshold_db = -30.0;                // Spectral gate threshold
    audio.processor.amplitude_threshold_db = -40.0;      // Amplitude gate threshold
    audio.processor.amplitude_attack_ms = 5.0;           // Attack time for amplitude gate
    audio.processor.amplitude_release_ms = 50.0;         // Release time for amplitude gate
    audio.processor.gain_db = 3.0;                       // Gain boost in dB
    audio.processor.limiter_threshold_db = -1.0;         // Limiter threshold
    audio.processor.limiter_release_ms = 50.0;           // Limiter release time
    audio.processor.lowpass_freq = 18000.0;              // Lowpass filter cutoff frequency
    audio.processor.highpass_freq = 100.0;               // Highpass filter cutoff frequency
    audio.processor.rms_target_db = -18.0;               // Target RMS level for normalization
    
    // Enable/disable specific processing stages
    audio.processor.rms_enabled = true;                  // Enable RMS normalization
    audio.processor.filters_enabled = true;              // Enable filters
    audio.processor.spectral_gate_enabled = true;        // Enable spectral gate
    audio.processor.amplitude_gate_enabled = true;       // Enable amplitude gate
    audio.processor.gain_boost_enabled = false;          // Disable gain boost
    audio.processor.limiter_enabled = true;              // Enable limiter
    
    // Configure Opus encoder
    audio.set_opus_bitrate(16000);                       // Set Opus bitrate to 16 kbps
    
    println!("Library configured with the following parameters:");
    println!("  Spectral gate threshold: {} dB", audio.processor.threshold_db);
    println!("  Amplitude gate threshold: {} dB", audio.processor.amplitude_threshold_db);
    println!("  Highpass filter: {} Hz", audio.processor.highpass_freq);
    println!("  Lowpass filter: {} Hz", audio.processor.lowpass_freq);
    println!("  RMS target level: {} dB", audio.processor.rms_target_db);
    println!("  Opus bitrate: {} bps", audio.get_opus_bitrate());
    
    // Example of processing a file (commented out)
    // if let Err(e) = audio.process_file("input.wav", "processed.wav") {
    //     println!("Error processing file: {:?}", e);
    // }
    
    // Example of recording (commented out)
    // match audio.start_recording("recording.wav") {
    //     Ok(_) => {
    //         println!("Recording started. Press Enter to stop...");
    //         let mut input = String::new();
    //         std::io::stdin().read_line(&mut input).unwrap();
    //         
    //         if let Err(e) = audio.stop_recording() {
    //             println!("Error stopping recording: {}", e);
    //         } else {
    //             println!("Recording stopped and processed");
    //         }
    //     },
    //     Err(e) => println!("Error starting recording: {}", e),
    // }
    
    println!("For more examples, see the documentation");
}
