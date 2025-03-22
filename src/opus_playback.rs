use opus::{Decoder, Channels};
use ogg::reading::PacketReader;
use std::fs::File;
use std::io::BufReader;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

pub fn get_opus_info(file_path: &str) -> Result<(u64, f64), Box<dyn std::error::Error>> {
    let file = File::open(file_path)?;
    let file_size = file.metadata()?.len();
    
    // Count the number of audio packets to estimate duration
    let reader = BufReader::new(file);
    let mut packet_reader = PacketReader::new(reader);
    
    // Skip headers
    packet_reader.read_packet()?; // OpusHead
    packet_reader.read_packet()?; // OpusTags
    
    let mut packet_count = 0;
    while let Ok(Some(_)) = packet_reader.read_packet() {
        packet_count += 1;
    }
    
    // Each packet is 20ms of audio
    let duration = (packet_count as f64) * 0.02;
    
    Ok((file_size, duration))
}

pub fn playback_opus(file_path: &str, is_playing_flag: Arc<AtomicBool>) -> Result<(), Box<dyn std::error::Error>> {
    // Create Opus decoder (48kHz is the default for Opus)
    let decoder = Decoder::new(48000, Channels::Mono)?;

    // Open Opus file
    let file = BufReader::new(File::open(file_path)?);
    let packet_reader = PacketReader::new(file);

    // Skip header packets
    let mut packet_reader = packet_reader;
    packet_reader.read_packet()?; // OpusHead
    packet_reader.read_packet()?; // OpusTags

    // Setup audio output
    let host = cpal::default_host();
    let device = host.default_output_device()
        .expect("Failed to get default output device");
    let config = device.default_output_config()?;

    // Force 48kHz output config
    let output_config = cpal::StreamConfig {
        channels: config.channels(),
        sample_rate: cpal::SampleRate(48000),
        buffer_size: cpal::BufferSize::Default,
    };

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => {
            let decoder = Arc::new(std::sync::Mutex::new(decoder));
            let packet_reader = Arc::new(std::sync::Mutex::new(packet_reader));
            let is_playing = Arc::clone(&is_playing_flag);
            
            // Create a fixed-size buffer for decoded audio
            let decoded_buffer = Arc::new(std::sync::Mutex::new(vec![0f32; 960]));
            let decoded_samples = Arc::new(std::sync::Mutex::new(0));
            let buffer_position = Arc::new(std::sync::Mutex::new(0)); // Track position in decoded buffer

            device.build_output_stream(
                &output_config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    if is_playing.load(Ordering::Relaxed) {
                        let channels = output_config.channels as usize;
                        let mut pos = 0;

                        while pos < data.len() {
                            // Check if we need more samples from the current buffer
                            let (need_new_packet, _current_pos) = {
                                let samples = *decoded_samples.lock().unwrap();
                                let position = *buffer_position.lock().unwrap();
                                (position >= samples, position)
                            };

                            if need_new_packet {
                                // Reset buffer position
                                *buffer_position.lock().unwrap() = 0;
                                
                                if let Ok(mut reader) = packet_reader.lock() {
                                    if let Ok(Some(packet)) = reader.read_packet() {
                                        if let Ok(mut decoder) = decoder.lock() {
                                            if let Ok(mut buffer) = decoded_buffer.lock() {
                                                if let Ok(n_samples) = decoder.decode_float(&packet.data, &mut buffer, false) {
                                                    *decoded_samples.lock().unwrap() = n_samples;
                                                }
                                            }
                                        }
                                    } else {
                                        is_playing.store(false, Ordering::Relaxed);
                                        break;
                                    }
                                }
                            } else {
                                // Copy available samples to output
                                if let (Ok(buffer), Ok(samples), Ok(mut position)) = 
                                    (decoded_buffer.lock(), decoded_samples.lock(), buffer_position.lock()) 
                                {
                                    let samples_remaining = *samples - *position;
                                    if samples_remaining > 0 {
                                        let samples_to_copy = ((data.len() - pos) / channels).min(samples_remaining);
                                        
                                        // Copy to all channels
                                        for i in 0..samples_to_copy {
                                            let sample = buffer[*position + i];
                                            for c in 0..channels {
                                                data[pos + i * channels + c] = sample;
                                            }
                                        }

                                        pos += samples_to_copy * channels;
                                        *position += samples_to_copy;
                                    }
                                }
                            }
                        }

                        // Fill any remaining space with silence
                        for sample in data[pos..].iter_mut() {
                            *sample = 0.0;
                        }
                    } else {
                        // Output silence when not playing
                        for sample in data.iter_mut() {
                            *sample = 0.0;
                        }
                    }
                },
                |err| eprintln!("Playback error: {:?}", err),
                None,
            )?
        },
        _ => return Err("Unsupported output format".into()),
    };

    stream.play()?;

    while is_playing_flag.load(Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    Ok(())
} 