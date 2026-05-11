use ogg::reading::PacketReader;
use opus_rs::OpusDecoder;
use std::fs::File;
use std::io::BufReader;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

pub struct OpusFileInfo {
    sample_rate: u32,
}

impl OpusFileInfo {
    pub fn get_sample_rate(&self) -> u32 {
        self.sample_rate
    }
}

fn frame_size_for(sample_rate: u32) -> usize {
    match sample_rate {
        8000 => 160,
        12000 => 240,
        16000 => 320,
        24000 => 480,
        48000 => 960,
        _ => 160,
    }
}

pub fn get_opus_info(file_path: &str) -> Result<(u64, f64, OpusFileInfo), Box<dyn std::error::Error>> {
    let file = File::open(file_path)?;
    let file_size = file.metadata()?.len();
    
    // Read the OpusHead packet to get sample rate
    let reader = BufReader::new(file);
    let mut packet_reader = PacketReader::new(reader);
    
    // Read OpusHead packet
    let head_packet = packet_reader.read_packet()?;
    let head_data = head_packet.ok_or("Failed to read OpusHead packet")?.data;
    
    // Parse sample rate from OpusHead (bytes 12-15)
    let sample_rate = u32::from_le_bytes([
        head_data[12], head_data[13], head_data[14], head_data[15]
    ]);
    
    // Skip OpusTags
    packet_reader.read_packet()?;
    
    let mut packet_count = 0;
    while let Ok(Some(_)) = packet_reader.read_packet() {
        packet_count += 1;
    }
    
    let frame_size = frame_size_for(sample_rate);
    let duration = (packet_count as f64 * frame_size as f64) / sample_rate as f64;
    
    Ok((file_size, duration, OpusFileInfo {
        sample_rate,
    }))
}

pub fn playback_opus(file_path: &str, is_playing_flag: Arc<AtomicBool>) -> Result<(), Box<dyn std::error::Error>> {
    // Get file info first
    let (_, _, file_info) = get_opus_info(file_path)?;
    let sample_rate = file_info.get_sample_rate();
    
    let decoder = OpusDecoder::new(sample_rate as i32, 1)
        .map_err(std::io::Error::other)?;

    let frame_size = frame_size_for(sample_rate);

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

    // Use the file's sample rate for output
    let output_config = cpal::StreamConfig {
        channels: config.channels(),
        sample_rate: cpal::SampleRate(sample_rate),
        buffer_size: cpal::BufferSize::Default,
    };

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => {
            let decoder = Arc::new(std::sync::Mutex::new(decoder));
            let packet_reader = Arc::new(std::sync::Mutex::new(packet_reader));
            let is_playing = Arc::clone(&is_playing_flag);
            
            // Calculate buffer size based on frame size
            let decoded_buffer = Arc::new(std::sync::Mutex::new(vec![0f32; frame_size]));
            let decoded_samples = Arc::new(std::sync::Mutex::new(0));
            let buffer_position = Arc::new(std::sync::Mutex::new(0));

            device.build_output_stream(
                &output_config,
                move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                    if is_playing.load(Ordering::Relaxed) {
                        let channels = output_config.channels as usize;
                        let mut pos = 0;

                        while pos < data.len() {
                            let (need_new_packet, _current_pos) = {
                                let samples = *decoded_samples.lock().unwrap();
                                let position = *buffer_position.lock().unwrap();
                                (position >= samples, position)
                            };

                            if need_new_packet {
                                *buffer_position.lock().unwrap() = 0;
                                
                                if let Ok(mut reader) = packet_reader.lock() {
                                    if let Ok(Some(packet)) = reader.read_packet() {
                                        if let Ok(mut decoder) = decoder.lock() {
                                            if let Ok(mut buffer) = decoded_buffer.lock() {
                                                if let Ok(n_samples) = decoder.decode(&packet.data, frame_size, &mut buffer) {
                                                    *decoded_samples.lock().unwrap() = n_samples;
                                                } else {
                                                    is_playing.store(false, Ordering::Relaxed);
                                                    break;
                                                }
                                            }
                                        }
                                    } else {
                                        is_playing.store(false, Ordering::Relaxed);
                                        break;
                                    }
                                }
                            } else {
                                if let (Ok(buffer), Ok(samples), Ok(mut position)) = 
                                    (decoded_buffer.lock(), decoded_samples.lock(), buffer_position.lock()) 
                                {
                                    let samples_remaining = *samples - *position;
                                    if samples_remaining > 0 {
                                        let samples_to_copy = ((data.len() - pos) / channels).min(samples_remaining);
                                        
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

                        for sample in data[pos..].iter_mut() {
                            *sample = 0.0;
                        }
                    } else {
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