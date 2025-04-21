mod record;
mod playback;
mod dsp;
mod opus_encoder;
mod opus_playback;

use eframe::egui;
use record::record_audio;
use playback::playback_audio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::sync::Mutex;
use crate::dsp::AudioProcessor;
use opus_encoder::OpusEncoder;
use opus_playback::playback_opus;

struct AudioFileInfo {
    file_size: u64,
    duration: f64,
    original_wav_size: u64,
    unprocessed_opus_size: u64,
    processed_opus_size: u64,
    last_message: String,
    loaded_file_path: Option<String>,
}

struct AudioApp {
    is_recording: Arc<AtomicBool>,
    is_playing: Arc<AtomicBool>,
    is_playing_original: Arc<AtomicBool>,
    is_playing_unprocessed_opus: Arc<AtomicBool>,
    recording_thread: Option<thread::JoinHandle<()>>,
    playback_thread: Option<thread::JoinHandle<()>>,
    playback_original_thread: Option<thread::JoinHandle<()>>,
    playback_unprocessed_opus_thread: Option<thread::JoinHandle<()>>,
    should_cleanup_recording: bool,
    should_cleanup_playback: bool,
    should_cleanup_playback_original: bool,
    should_cleanup_playback_unprocessed_opus: bool,
    audio_info: Arc<Mutex<AudioFileInfo>>,
    processor: AudioProcessor,
    opus_encoder: OpusEncoder,
    use_low_bitrate: bool,
    use_high_bitrate: bool,
    processing_thread: Option<thread::JoinHandle<()>>,
    is_processing: Arc<AtomicBool>,
    should_cleanup_processing: bool,
}

impl Default for AudioApp {
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
            should_cleanup_recording: false,
            should_cleanup_playback: false,
            should_cleanup_playback_original: false,
            should_cleanup_playback_unprocessed_opus: false,
            audio_info: Arc::new(Mutex::new(AudioFileInfo {
                file_size: 0,
                duration: 0.0,
                original_wav_size: 0,
                unprocessed_opus_size: 0,
                processed_opus_size: 0,
                last_message: String::new(),
                loaded_file_path: None,
            })),
            processor: AudioProcessor::new(44100.0),
            opus_encoder: OpusEncoder::new(),
            use_low_bitrate: false,
            use_high_bitrate: false,
            processing_thread: None,
            is_processing: Arc::new(AtomicBool::new(false)),
            should_cleanup_processing: false,
        }
    }
}

impl eframe::App for AudioApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Create a layout with left and right panels
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                // Use a horizontal layout with two panels
                ui.horizontal(|ui| {
                    // Left panel
                    ui.vertical(|ui| {
                        let panel_width = ui.available_width() / 2.0 - 10.0;
                        ui.set_min_width(400.0);
                        
                        ui.heading("Audio Processor");
                        ui.add_space(20.0);
                        
                        // Add effect toggles section
                        ui.group(|ui| {
                            ui.set_width(panel_width);
                            ui.heading("Effect Toggles");
                            
                            // Add RMS Normalization toggle first
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut self.processor.rms_enabled, "RMS Normalization");
                                ui.checkbox(&mut self.processor.filters_enabled, "Filters");
                                ui.checkbox(&mut self.processor.spectral_gate_enabled, "Spectral Gate");
                            });
                            ui.horizontal(|ui| {
                                ui.checkbox(&mut self.processor.amplitude_gate_enabled, "Noise Gate");
                                ui.checkbox(&mut self.processor.gain_boost_enabled, "Gain Boost");
                                ui.checkbox(&mut self.processor.limiter_enabled, "Limiter");
                            });
                        });
                        
                        ui.add_space(10.0);
                        
                        // Add RMS Normalization section
                        ui.group(|ui| {
                            ui.set_width(panel_width);
                            ui.horizontal(|ui| {
                                ui.heading("RMS Normalization");
                                ui.checkbox(&mut self.processor.rms_enabled, "Enabled");
                            });
                            
                            ui.add_enabled_ui(self.processor.rms_enabled, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label("Target RMS:");
                                    ui.add(egui::Slider::new(
                                        &mut self.processor.rms_target_db,
                                        -30.0..=-6.0
                                    ).suffix(" dB"));
                                });
                            });
                        });
                        
                        ui.add_space(10.0);
                        
                        // 1. Filters
                        ui.group(|ui| {
                            ui.set_width(panel_width);
                            ui.horizontal(|ui| {
                                ui.heading("Filters");
                                ui.checkbox(&mut self.processor.filters_enabled, "Enabled");
                            });
                            
                            ui.add_enabled_ui(self.processor.filters_enabled, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label("Highpass:");
                                    ui.add(egui::Slider::new(
                                        &mut self.processor.highpass_freq,
                                        20.0..=1000.0
                                    ).suffix(" Hz")
                                    .logarithmic(true));
                                });
                                
                                ui.horizontal(|ui| {
                                    ui.label("Lowpass:");
                                    ui.add(egui::Slider::new(
                                        &mut self.processor.lowpass_freq,
                                        1000.0..=20000.0
                                    ).suffix(" Hz")
                                    .logarithmic(true));
                                });
                            });
                        });
                        
                        ui.add_space(10.0);
                        
                        // 2. Spectral Gate
                        ui.group(|ui| {
                            ui.set_width(panel_width);
                            ui.horizontal(|ui| {
                                ui.heading("Spectral Gate");
                                ui.checkbox(&mut self.processor.spectral_gate_enabled, "Enabled");
                            });
                            
                            ui.add_enabled_ui(self.processor.spectral_gate_enabled, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label("Threshold:");
                                    ui.add(egui::Slider::new(
                                        &mut self.processor.threshold_db,
                                        -50.0..=24.0
                                    ).suffix(" dB"));
                                });
                            });
                        });
                        
                        ui.add_space(10.0);
                        
                        // 3. Noise Gate
                        ui.group(|ui| {
                            ui.set_width(panel_width);
                            ui.horizontal(|ui| {
                                ui.heading("Noise Gate");
                                ui.checkbox(&mut self.processor.amplitude_gate_enabled, "Enabled");
                            });
                            
                            ui.add_enabled_ui(self.processor.amplitude_gate_enabled, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label("Threshold:");
                                    ui.add(egui::Slider::new(
                                        &mut self.processor.amplitude_threshold_db,
                                        -60.0..=0.0
                                    ).suffix(" dB"));
                                });
                                
                                ui.label("Attack:");
                                ui.add(egui::Slider::new(
                                    &mut self.processor.amplitude_attack_ms,
                                    0.1..=100.0
                                ).suffix(" ms")
                                .logarithmic(true));
                                
                                ui.label("Release:");
                                ui.add(egui::Slider::new(
                                    &mut self.processor.amplitude_release_ms,
                                    1.0..=1000.0
                                ).suffix(" ms")
                                .logarithmic(true));
                                
                                ui.label("Lookahead:");
                                ui.add(egui::Slider::new(
                                    &mut self.processor.amplitude_lookahead_ms,
                                    0.0..=20.0
                                ).suffix(" ms"));
                            });
                        });
                        
                        ui.add_space(10.0);
                        
                        // 4. Gain Booster
                        ui.group(|ui| {
                            ui.set_width(panel_width);
                            ui.horizontal(|ui| {
                                ui.heading("Gain Booster");
                                ui.checkbox(&mut self.processor.gain_boost_enabled, "Enabled");
                            });
                            
                            ui.add_enabled_ui(self.processor.gain_boost_enabled, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label("Gain:");
                                    ui.add(egui::Slider::new(
                                        &mut self.processor.gain_db,
                                        0.0..=24.0
                                    ).suffix(" dB"));
                                });
                            });
                        });
                        
                        ui.add_space(10.0);
                        
                        // 5. Limiter
                        ui.group(|ui| {
                            ui.set_width(panel_width);
                            ui.horizontal(|ui| {
                                ui.heading("Lookahead Limiter");
                                ui.checkbox(&mut self.processor.limiter_enabled, "Enabled");
                            });
                            
                            ui.add_enabled_ui(self.processor.limiter_enabled, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label("Threshold:");
                                    ui.add(egui::Slider::new(
                                        &mut self.processor.limiter_threshold_db,
                                        -12.0..=0.0
                                    ).suffix(" dB"));
                                });
                                
                                ui.horizontal(|ui| {
                                    ui.label("Release Time:");
                                    ui.add(egui::Slider::new(
                                        &mut self.processor.limiter_release_ms,
                                        10.0..=500.0
                                    ).suffix(" ms"));
                                });
                                
                                ui.horizontal(|ui| {
                                    ui.label("Lookahead:");
                                    ui.add(egui::Slider::new(
                                        &mut self.processor.limiter_lookahead_ms,
                                        1.0..=20.0
                                    ).suffix(" ms"));
                                });
                            });
                        });
                    });
                    
                    // Right panel
                    ui.vertical(|ui| {
                        let panel_width = ui.available_width() - 10.0;
                        ui.set_min_width(400.0);
                        
                        ui.heading("Opus Encoding");
                        ui.add_space(20.0);
                        
                        // Add Opus settings section
                        ui.group(|ui| {
                            ui.set_width(panel_width);
                            ui.heading("Opus Encoding Settings");
                            
                            // Add bitrate options with three choices
                            ui.horizontal(|ui| {
                                ui.label("Bitrate:");
                            });
                            
                            // Use a single variable for bitrate selection
                            let mut bitrate_option = if self.use_high_bitrate {
                                0 // 24 kbps
                            } else if !self.use_low_bitrate {
                                1 // 12 kbps
                            } else {
                                2 // 6 kbps
                            };
                            
                            if ui.radio_value(&mut bitrate_option, 0, "24 kbps (highest quality)").clicked() {
                                self.use_high_bitrate = true;
                                self.use_low_bitrate = false;
                                self.opus_encoder.set_bitrate(24000);
                            }
                            
                            if ui.radio_value(&mut bitrate_option, 1, "12 kbps (balanced)").clicked() {
                                self.use_high_bitrate = false;
                                self.use_low_bitrate = false;
                                self.opus_encoder.set_bitrate(12000);
                            }
                            
                            if ui.radio_value(&mut bitrate_option, 2, "6 kbps (smallest size)").clicked() {
                                self.use_high_bitrate = false;
                                self.use_low_bitrate = true;
                                self.opus_encoder.set_bitrate(6000);
                            }
                            
                            // Show current bitrate
                            ui.label(format!("Current bitrate: {} kbps", self.opus_encoder.get_bitrate() / 1000));
                        });
                        
                        ui.add_space(20.0);
                        
                        // Recording and playback controls
                        ui.group(|ui| {
                            ui.set_width(panel_width);
                            let recording = self.is_recording.load(Ordering::Relaxed);
                            let playing = self.is_playing.load(Ordering::Relaxed);
                            let playing_original = self.is_playing_original.load(Ordering::Relaxed);
                            let playing_unprocessed_opus = self.is_playing_unprocessed_opus.load(Ordering::Relaxed);
                            let processing = self.is_processing.load(Ordering::Relaxed);
                            
                            // Recording and Open File buttons in one row
                            ui.horizontal(|ui| {
                                // Recording button - make it red
                                if recording {
                                    if ui.add(egui::Button::new("Stop Recording").fill(egui::Color32::from_rgb(200, 60, 60))).clicked() {
                                        self.is_recording.store(false, Ordering::Relaxed);
                                        self.should_cleanup_recording = true;
                                    }
                                } else if !playing && !playing_original && !playing_unprocessed_opus && !processing {
                                    if ui.add(egui::Button::new(egui::RichText::new("Record").color(egui::Color32::BLACK)).fill(egui::Color32::from_rgb(200, 60, 60))).clicked() {
                                        let is_recording = Arc::clone(&self.is_recording);
                                        let audio_info = Arc::clone(&self.audio_info);
                                        let processor = self.processor.clone();
                                        let opus_encoder = self.opus_encoder.clone();
                                        self.is_recording.store(true, Ordering::Relaxed);
                                        self.recording_thread = Some(thread::spawn(move || {
                                            if let Ok(_) = record_audio("output.wav", is_recording, processor.clone()) {
                                                let mut info = audio_info.lock().unwrap();
                                                info.last_message = "Recording completed successfully".to_string();
                                                
                                                // Copy output.wav to original.wav
                                                if let Err(e) = std::fs::copy("output.wav", "original.wav") {
                                                    info.last_message = format!("Error copying to original.wav: {:?}", e);
                                                    return;
                                                }
                                                
                                                // Update original WAV file size
                                                if let Ok(metadata) = std::fs::metadata("original.wav") {
                                                    info.original_wav_size = metadata.len();
                                                }
                                                
                                                // Process audio
                                                let mut processor_instance = processor;
                                                if let Err(e) = processor_instance.process_file("output.wav", "processed.wav") {
                                                    info.last_message = format!("Error processing audio: {:?}", e);
                                                    return;
                                                }
                                                
                                                // Encode to Opus
                                                if let Err(e) = opus_encoder.encode_wav_to_opus("processed.wav", "processed.opus") {
                                                    info.last_message = format!("Error encoding to Opus: {:?}", e);
                                                } else {
                                                    // Update file info after successful encoding
                                                    match opus_playback::get_opus_info("processed.opus") {
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
                                                if let Err(e) = opus_encoder.encode_wav_to_opus("original.wav", "unprocessed.opus") {
                                                    info.last_message = format!("Error encoding unprocessed audio: {:?}", e);
                                                } else {
                                                    // Update unprocessed opus file size
                                                    if let Ok(metadata) = std::fs::metadata("unprocessed.opus") {
                                                        info.unprocessed_opus_size = metadata.len();
                                                    }
                                                }
                                            }
                                        }));
                                    }
                                }
                                
                                // Open File button - make it green
                                if !recording && !processing {
                                    if ui.add(egui::Button::new(egui::RichText::new("Open").color(egui::Color32::BLACK)).fill(egui::Color32::from_rgb(60, 200, 60))).clicked() {
                                        // Use native file dialog
                                        if let Some(path) = rfd::FileDialog::new()
                                            .add_filter("WAV Audio", &["wav"])
                                            .set_title("Select WAV file to process")
                                            .pick_file() 
                                        {
                                            let path_str = path.to_string_lossy().to_string();
                                            let path_clone = path_str.clone();
                                            
                                            // Copy file to original.wav
                                            if let Err(e) = std::fs::copy(&path, "original.wav") {
                                                let mut info = self.audio_info.lock().unwrap();
                                                info.last_message = format!("Error copying file: {:?}", e);
                                            } else {
                                                let processor = self.processor.clone();
                                                let opus_encoder = self.opus_encoder.clone();
                                                let audio_info = Arc::clone(&self.audio_info);
                                                let is_processing = Arc::clone(&self.is_processing);
                                                
                                                self.is_processing.store(true, Ordering::Relaxed);
                                                self.processing_thread = Some(thread::spawn(move || {
                                                    // Update original WAV file size
                                                    if let Ok(metadata) = std::fs::metadata("original.wav") {
                                                        let mut info = audio_info.lock().unwrap();
                                                        info.original_wav_size = metadata.len();
                                                        info.loaded_file_path = Some(path_clone.clone());
                                                        info.last_message = format!("Opened file: {}", path_clone);
                                                    }
                                                    
                                                    // Process audio
                                                    let mut processor_instance = processor;
                                                    if let Err(e) = processor_instance.process_file("original.wav", "processed.wav") {
                                                        let mut info = audio_info.lock().unwrap();
                                                        info.last_message = format!("Error processing audio: {:?}", e);
                                                        is_processing.store(false, Ordering::Relaxed);
                                                        return;
                                                    }
                                                    
                                                    // Encode to Opus
                                                    if let Err(e) = opus_encoder.encode_wav_to_opus("processed.wav", "processed.opus") {
                                                        let mut info = audio_info.lock().unwrap();
                                                        info.last_message = format!("Error encoding to Opus: {:?}", e);
                                                    } else {
                                                        // Update file info after successful encoding
                                                        match opus_playback::get_opus_info("processed.opus") {
                                                            Ok((size, duration)) => {
                                                                let mut info = audio_info.lock().unwrap();
                                                                info.file_size = size;
                                                                info.processed_opus_size = size;
                                                                info.duration = duration;
                                                                info.last_message = "Processing and Opus encoding completed successfully".to_string();
                                                            }
                                                            Err(e) => {
                                                                let mut info = audio_info.lock().unwrap();
                                                                info.last_message = format!("Error getting Opus file info: {:?}", e);
                                                            }
                                                        }
                                                    }
                                                    
                                                    // Also encode original to opus for comparison
                                                    if let Err(e) = opus_encoder.encode_wav_to_opus("original.wav", "unprocessed.opus") {
                                                        let mut info = audio_info.lock().unwrap();
                                                        info.last_message = format!("Error encoding unprocessed audio: {:?}", e);
                                                    } else {
                                                        // Update unprocessed opus file size
                                                        if let Ok(metadata) = std::fs::metadata("unprocessed.opus") {
                                                            let mut info = audio_info.lock().unwrap();
                                                            info.unprocessed_opus_size = metadata.len();
                                                        }
                                                    }
                                                    
                                                    is_processing.store(false, Ordering::Relaxed);
                                                }));
                                            }
                                        }
                                    }
                                }
                            });
                            
                            // Reprocess button
                            if !recording && !processing {
                                if ui.add(egui::Button::new(egui::RichText::new("Reprocess").color(egui::Color32::BLACK)).fill(egui::Color32::from_rgb(255, 255, 0))).clicked() {
                                    // Check if we have an original.wav file to reprocess
                                    if let Ok(_) = std::fs::metadata("original.wav") {
                                        let processor = self.processor.clone();
                                        let opus_encoder = self.opus_encoder.clone();
                                        let audio_info = Arc::clone(&self.audio_info);
                                        let is_processing = Arc::clone(&self.is_processing);
                                        
                                        self.is_processing.store(true, Ordering::Relaxed);
                                        self.processing_thread = Some(thread::spawn(move || {
                                            // Process audio with current settings
                                            let mut processor_instance = processor;
                                            if let Err(e) = processor_instance.process_file("original.wav", "processed.wav") {
                                                let mut info = audio_info.lock().unwrap();
                                                info.last_message = format!("Error reprocessing audio: {:?}", e);
                                                is_processing.store(false, Ordering::Relaxed);
                                                return;
                                            }
                                            
                                            // Encode to Opus
                                            if let Err(e) = opus_encoder.encode_wav_to_opus("processed.wav", "processed.opus") {
                                                let mut info = audio_info.lock().unwrap();
                                                info.last_message = format!("Error encoding to Opus: {:?}", e);
                                            } else {
                                                // Update file info after successful encoding
                                                match opus_playback::get_opus_info("processed.opus") {
                                                    Ok((size, duration)) => {
                                                        let mut info = audio_info.lock().unwrap();
                                                        info.file_size = size;
                                                        info.processed_opus_size = size;
                                                        info.duration = duration;
                                                        info.last_message = "Reprocessing completed successfully".to_string();
                                                    }
                                                    Err(e) => {
                                                        let mut info = audio_info.lock().unwrap();
                                                        info.last_message = format!("Error getting Opus file info: {:?}", e);
                                                    }
                                                }
                                            }
                                            
                                            is_processing.store(false, Ordering::Relaxed);
                                        }));
                                    } else {
                                        let mut info = self.audio_info.lock().unwrap();
                                        info.last_message = "No audio file available to reprocess".to_string();
                                    }
                                }
                            } else if processing {
                                ui.add(egui::Button::new(egui::RichText::new("Processing...").color(egui::Color32::BLACK)).fill(egui::Color32::from_rgb(150, 150, 150)));
                            }
                            
                            ui.add_space(10.0);
                            ui.heading("WAV Playback");
                            
                            // WAV playback buttons in one row
                            ui.horizontal(|ui| {
                                // Original WAV button - make it blue
                                if playing_original {
                                    if ui.add(egui::Button::new("Stop Original WAV").fill(egui::Color32::from_rgb(60, 60, 200))).clicked() {
                                        self.is_playing_original.store(false, Ordering::Relaxed);
                                        self.should_cleanup_playback_original = true;
                                    }
                                } else if !recording && !playing && !playing_unprocessed_opus {
                                    if ui.add(egui::Button::new("Play Original WAV").fill(egui::Color32::from_rgb(60, 60, 200))).clicked() {
                                        let is_playing = Arc::clone(&self.is_playing_original);
                                        let audio_info = Arc::clone(&self.audio_info);
                                        self.is_playing_original.store(true, Ordering::Relaxed);
                                        self.playback_original_thread = Some(thread::spawn(move || {
                                            match playback_audio("original.wav", is_playing) {
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
                                    }
                                }
                                
                                // Processed WAV button - make it blue
                                if playing {
                                    if ui.add(egui::Button::new("Stop Processed WAV").fill(egui::Color32::from_rgb(60, 60, 200))).clicked() {
                                        self.is_playing.store(false, Ordering::Relaxed);
                                        self.should_cleanup_playback = true;
                                    }
                                } else if !recording && !playing_original && !playing_unprocessed_opus {
                                    if ui.add(egui::Button::new("Play Processed WAV").fill(egui::Color32::from_rgb(60, 60, 200))).clicked() {
                                        let is_playing = Arc::clone(&self.is_playing);
                                        let audio_info = Arc::clone(&self.audio_info);
                                        self.is_playing.store(true, Ordering::Relaxed);
                                        self.playback_thread = Some(thread::spawn(move || {
                                            match playback_audio("processed.wav", is_playing) {
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
                                    }
                                }
                            });
                            
                            ui.add_space(5.0);
                            ui.heading("Opus Playback");
                            
                            // Opus playback buttons in one row
                            ui.horizontal(|ui| {
                                // Unprocessed Opus button - make it blue
                                if playing_unprocessed_opus {
                                    if ui.add(egui::Button::new("Stop Unprocessed Opus").fill(egui::Color32::from_rgb(60, 60, 200))).clicked() {
                                        self.is_playing_unprocessed_opus.store(false, Ordering::Relaxed);
                                        self.should_cleanup_playback_unprocessed_opus = true;
                                    }
                                } else if !recording && !playing && !playing_original {
                                    if ui.add(egui::Button::new("Play Unprocessed Opus").fill(egui::Color32::from_rgb(60, 60, 200))).clicked() {
                                        // First, ensure we have an unprocessed opus file
                                        let audio_info = Arc::clone(&self.audio_info);
                                        let opus_encoder = self.opus_encoder.clone();
                                        
                                        // Create unprocessed opus file if it doesn't exist
                                        if let Err(e) = opus_encoder.encode_wav_to_opus("original.wav", "unprocessed.opus") {
                                            let mut info = audio_info.lock().unwrap();
                                            info.last_message = format!("Error encoding unprocessed audio: {:?}", e);
                                        } else {
                                            // Update unprocessed opus file size
                                            if let Ok(metadata) = std::fs::metadata("unprocessed.opus") {
                                                let mut info = audio_info.lock().unwrap();
                                                info.unprocessed_opus_size = metadata.len();
                                            }
                                            
                                            // Play the unprocessed opus file
                                            let is_playing = Arc::clone(&self.is_playing_unprocessed_opus);
                                            let audio_info = Arc::clone(&self.audio_info);
                                            self.is_playing_unprocessed_opus.store(true, Ordering::Relaxed);
                                            self.playback_unprocessed_opus_thread = Some(thread::spawn(move || {
                                                match playback_opus("unprocessed.opus", is_playing) {
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
                                        }
                                    }
                                }
                                
                                // Processed Opus button
                                if playing {
                                    if ui.add(egui::Button::new("Stop Processed Opus").fill(egui::Color32::from_rgb(60, 60, 200))).clicked() {
                                        self.is_playing.store(false, Ordering::Relaxed);
                                        self.should_cleanup_playback = true;
                                    }
                                } else if !recording && !playing_original && !playing_unprocessed_opus {
                                    if ui.add(egui::Button::new("Play Processed Opus").fill(egui::Color32::from_rgb(60, 60, 200))).clicked() {
                                        // Update processed opus file size
                                        if let Ok(metadata) = std::fs::metadata("processed.opus") {
                                            let mut info = self.audio_info.lock().unwrap();
                                            info.processed_opus_size = metadata.len();
                                        }
                                        
                                        let is_playing = Arc::clone(&self.is_playing);
                                        let audio_info = Arc::clone(&self.audio_info);
                                        self.is_playing.store(true, Ordering::Relaxed);
                                        self.playback_thread = Some(thread::spawn(move || {
                                            match playback_opus("processed.opus", is_playing) {
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
                                    }
                                }
                            });
                        });
                        
                        ui.add_space(10.0);
                        
                        // Status information with file sizes in KB
                        ui.group(|ui| {
                            ui.set_width(panel_width);
                            let info = self.audio_info.lock().unwrap();
                            ui.label(format!("Original WAV size: {:.1} KB", info.original_wav_size as f64 / 1024.0));
                            ui.label(format!("Processed Opus size: {:.1} KB", info.processed_opus_size as f64 / 1024.0));
                            ui.label(format!("Duration: {:.2} seconds", info.duration));
                            ui.label(&info.last_message);
                        });
                    });
                });
            });
        });
        
        // Request repaint if needed
        if self.is_recording.load(Ordering::Relaxed) || 
           self.is_playing.load(Ordering::Relaxed) || 
           self.is_playing_original.load(Ordering::Relaxed) ||
           self.is_playing_unprocessed_opus.load(Ordering::Relaxed) ||
           self.is_processing.load(Ordering::Relaxed) {
            ctx.request_repaint();
        }

        // Handle cleanup for the new thread
        if self.should_cleanup_playback_unprocessed_opus {
            if let Some(thread) = self.playback_unprocessed_opus_thread.take() {
                if thread.is_finished() {
                    let _ = thread.join();
                    self.should_cleanup_playback_unprocessed_opus = false;
                }
            }
        }

        // Handle cleanup for processing thread
        if self.should_cleanup_processing {
            if let Some(thread) = self.processing_thread.take() {
                if thread.is_finished() {
                    let _ = thread.join();
                    self.should_cleanup_processing = false;
                }
            }
        }
    }
}

fn main() {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([800.0, 800.0])  // Doubled width for two columns
            .with_min_inner_size([800.0, 600.0]), // Set minimum window size
        ..Default::default()
    };
    
    eframe::run_native(
        "Rustic_Audio",
        options,
        Box::new(|_cc| Box::new(AudioApp::default())),
    ).unwrap();
}
