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
    last_message: String,
}

struct AudioApp {
    is_recording: Arc<AtomicBool>,
    is_playing: Arc<AtomicBool>,
    is_playing_original: Arc<AtomicBool>,
    recording_thread: Option<thread::JoinHandle<()>>,
    playback_thread: Option<thread::JoinHandle<()>>,
    playback_original_thread: Option<thread::JoinHandle<()>>,
    should_cleanup_recording: bool,
    should_cleanup_playback: bool,
    should_cleanup_playback_original: bool,
    audio_info: Arc<Mutex<AudioFileInfo>>,
    processor: AudioProcessor,
    opus_encoder: OpusEncoder,
}

impl Default for AudioApp {
    fn default() -> Self {
        Self {
            is_recording: Arc::new(AtomicBool::new(false)),
            is_playing: Arc::new(AtomicBool::new(false)),
            is_playing_original: Arc::new(AtomicBool::new(false)),
            recording_thread: None,
            playback_thread: None,
            playback_original_thread: None,
            should_cleanup_recording: false,
            should_cleanup_playback: false,
            should_cleanup_playback_original: false,
            audio_info: Arc::new(Mutex::new(AudioFileInfo {
                file_size: 0,
                duration: 0.0,
                last_message: String::new(),
            })),
            processor: AudioProcessor::new(44100.0),
            opus_encoder: OpusEncoder::new(),
        }
    }
}

impl eframe::App for AudioApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Set initial window size and make scrollable
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                // Set a consistent width for all UI elements
                let panel_width = ui.available_width() - 20.0;
                ui.set_min_width(400.0);  // Minimum width to prevent controls from squishing
                
                ui.heading("Audio Processor");
                ui.add_space(20.0);

                // Add effect toggles section
                ui.group(|ui| {
                    ui.set_width(panel_width);
                    ui.heading("Effect Toggles");
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.processor.filters_enabled, "Filters");
                        ui.checkbox(&mut self.processor.spectral_gate_enabled, "Spectral Gate");
                        ui.checkbox(&mut self.processor.amplitude_gate_enabled, "Noise Gate");
                    });
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut self.processor.gain_boost_enabled, "Gain Boost");
                        ui.checkbox(&mut self.processor.limiter_enabled, "Limiter");
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

                // 3. Noise Gate (renamed from Amplitude Gate)
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
                        
                        // Rest of noise gate controls in the same box
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

                ui.add_space(20.0);

                // Recording and playback controls
                ui.group(|ui| {
                    ui.set_width(panel_width);
                    let recording = self.is_recording.load(Ordering::Relaxed);
                    let playing = self.is_playing.load(Ordering::Relaxed);
                    let playing_original = self.is_playing_original.load(Ordering::Relaxed);

                    if recording {
                        if ui.button("Stop Recording").clicked() {
                            self.is_recording.store(false, Ordering::Relaxed);
                            self.should_cleanup_recording = true;
                        }
                    } else if !playing && !playing_original && ui.button("Record").clicked() {
                        let is_recording = Arc::clone(&self.is_recording);
                        let audio_info = Arc::clone(&self.audio_info);
                        let processor = self.processor.clone();
                        let opus_encoder = self.opus_encoder.clone();
                        self.is_recording.store(true, Ordering::Relaxed);
                        self.recording_thread = Some(thread::spawn(move || {
                            if let Ok(_) = record_audio("output.wav", is_recording) {
                                let mut info = audio_info.lock().unwrap();
                                info.last_message = "Recording completed successfully".to_string();
                                
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
                                            info.duration = duration;
                                            info.last_message = "Processing and Opus encoding completed successfully".to_string();
                                        }
                                        Err(e) => {
                                            info.last_message = format!("Error getting Opus file info: {:?}", e);
                                        }
                                    }
                                }
                            }
                        }));
                    }

                    ui.add_space(10.0);
                    ui.horizontal(|ui| {
                        if playing {
                            if ui.button("Stop Processed Playback").clicked() {
                                self.is_playing.store(false, Ordering::Relaxed);
                                self.should_cleanup_playback = true;
                            }
                        } else if !recording && !playing_original && ui.button("Play Processed").clicked() {
                            let is_playing = Arc::clone(&self.is_playing);
                            let audio_info = Arc::clone(&self.audio_info);
                            self.is_playing.store(true, Ordering::Relaxed);
                            self.playback_thread = Some(thread::spawn(move || {
                                match playback_opus("processed.opus", is_playing) {
                                    Ok(_) => {
                                        let mut info = audio_info.lock().unwrap();
                                        info.last_message = "Processed playback completed successfully".to_string();
                                    },
                                    Err(e) => {
                                        let mut info = audio_info.lock().unwrap();
                                        info.last_message = format!("Error during processed playback: {:?}", e);
                                    },
                                }
                            }));
                        }

                        if playing_original {
                            if ui.button("Stop Original Playback").clicked() {
                                self.is_playing_original.store(false, Ordering::Relaxed);
                                self.should_cleanup_playback_original = true;
                            }
                        } else if !recording && !playing && ui.button("Play Original").clicked() {
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
                    });
                });

                ui.add_space(10.0);

                // Status information
                ui.group(|ui| {
                    ui.set_width(panel_width);
                    let info = self.audio_info.lock().unwrap();
                    ui.label(format!("File size: {} bytes", info.file_size));
                    ui.label(format!("Duration: {:.2} seconds", info.duration));
                    ui.label(&info.last_message);
                });
            });
        });

        // Request repaint if needed
        if self.is_recording.load(Ordering::Relaxed) || 
           self.is_playing.load(Ordering::Relaxed) || 
           self.is_playing_original.load(Ordering::Relaxed) {
            ctx.request_repaint();
        }
    }
}

fn main() {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 800.0])  // Increased default window size
            .with_min_inner_size([400.0, 600.0]), // Set minimum window size
        ..Default::default()
    };
    
    eframe::run_native(
        "Rustic_Audio",  // Changed name to match heading
        options,
        Box::new(|_cc| Box::new(AudioApp::default())),
    ).unwrap();
}
