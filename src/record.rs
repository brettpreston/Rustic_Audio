use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use hound;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::error::Error;

pub fn record_audio(file_path: &str, is_recording_flag: Arc<AtomicBool>) -> Result<(), Box<dyn Error>> {
    let host = cpal::default_host();
    let device = host.default_input_device().expect("Failed to get default input device");
    let config = device.default_input_config()?;

    let sample_format = config.sample_format();
    let channels = config.channels();
    let input_sample_rate = config.sample_rate();
    let config = config.config();

    println!("Recording with: format={:?}, rate={}, channels={}", 
             sample_format, input_sample_rate.0, channels);

    // Create a temporary file for initial recording
    let temp_file = "temp_recording.wav";
    let spec = hound::WavSpec {
        channels,
        sample_rate: input_sample_rate.0,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    
    let writer = Arc::new(Mutex::new(Some(hound::WavWriter::create(temp_file, spec)?)));
    let samples_written = Arc::new(Mutex::new(0u32));

    let stream = match sample_format {
        cpal::SampleFormat::F32 => {
            let writer_clone = Arc::clone(&writer);
            let is_recording = Arc::clone(&is_recording_flag);
            let samples_count = Arc::clone(&samples_written);
            
            device.build_input_stream(
                &config,
                move |data: &[f32], _| {
                    if is_recording.load(Ordering::Relaxed) {
                        if let Ok(mut guard) = writer_clone.try_lock() {
                            if let Some(writer) = guard.as_mut() {
                                for &sample in data {
                                    let sample = (sample * i16::MAX as f32) as i16;
                                    let _ = writer.write_sample(sample);
                                    if let Ok(mut count) = samples_count.try_lock() {
                                        *count += 1;
                                    }
                                }
                            }
                        }
                    }
                },
                |err| eprintln!("Stream error: {:?}", err),
                None,
            )?
        },
        cpal::SampleFormat::I16 => {
            let writer_clone = Arc::clone(&writer);
            let is_recording = Arc::clone(&is_recording_flag);
            let samples_count = Arc::clone(&samples_written);
            
            device.build_input_stream(
                &config,
                move |data: &[i16], _| {
                    if is_recording.load(Ordering::Relaxed) {
                        if let Ok(mut guard) = writer_clone.try_lock() {
                            if let Some(writer) = guard.as_mut() {
                                for &sample in data {
                                    let _ = writer.write_sample(sample);
                                    if let Ok(mut count) = samples_count.try_lock() {
                                        *count += 1;
                                    }
                                }
                            }
                        }
                    }
                },
                |err| eprintln!("Stream error: {:?}", err),
                None,
            )?
        },
        cpal::SampleFormat::U16 => {
            let writer_clone = Arc::clone(&writer);
            let is_recording = Arc::clone(&is_recording_flag);
            let samples_count = Arc::clone(&samples_written);
            
            device.build_input_stream(
                &config,
                move |data: &[u16], _| {
                    if is_recording.load(Ordering::Relaxed) {
                        if let Ok(mut guard) = writer_clone.try_lock() {
                            if let Some(writer) = guard.as_mut() {
                                for &sample in data {
                                    let sample = sample as i16 - i16::MAX;
                                    let _ = writer.write_sample(sample);
                                    if let Ok(mut count) = samples_count.try_lock() {
                                        *count += 1;
                                    }
                                }
                            }
                        }
                    }
                },
                |err| eprintln!("Stream error: {:?}", err),
                None,
            )?
        },
        _ => return Err("Unsupported sample format".into()),
    };

    println!("Stream created, starting playback");
    stream.play()?;
    println!("Stream started");

    while is_recording_flag.load(Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if let Ok(count) = samples_written.try_lock() {
            println!("Samples written: {}", *count);
        }
    }

    // Give a small delay for the stream to finish
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Drop the stream first
    drop(stream);
    println!("Stream dropped");

    // Then finalize the writer
    if let Ok(mut guard) = writer.try_lock() {
        if let Some(writer) = guard.take() {
            match writer.finalize() {
                Ok(_) => println!("Writer finalized successfully"),
                Err(e) => eprintln!("Error finalizing writer: {:?}", e),
            }
        }
    }

    if let Ok(count) = samples_written.try_lock() {
        println!("Total samples recorded: {}", *count);
    }
    
    if let Ok(metadata) = std::fs::metadata(temp_file) {
        println!("Output file size: {} bytes", metadata.len());
    }

    // Save the original recording first
    std::fs::copy(temp_file, "original.wav")?;
    
    // Read the temporary file for processing
    let mut reader = hound::WavReader::open(temp_file)?;
    let input_spec = reader.spec();
    
    // Create the final 48kHz mono file
    let output_spec = hound::WavSpec {
        channels: 1, // Mono
        sample_rate: 48000,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    
    let mut writer = hound::WavWriter::create(file_path, output_spec)?;
    
    // Read all samples into memory
    let samples: Vec<i16> = reader.samples::<i16>()
        .filter_map(Result::ok)
        .collect();
    
    // Convert to mono if stereo (take left channel)
    let mono_samples: Vec<i16> = if input_spec.channels == 2 {
        samples.chunks(2)
            .map(|chunk| chunk[0]) // Take left channel
            .collect()
    } else {
        samples
    };
    
    // Resample to 48kHz if needed
    if input_spec.sample_rate != 48000 {
        let mono_float: Vec<f32> = mono_samples.iter()
            .map(|&s| s as f32 / 32768.0)
            .collect();
            
        // Simple linear interpolation resampling
        let input_duration = mono_float.len() as f32 / input_spec.sample_rate as f32;
        let output_len = (input_duration * 48000.0) as usize;
        let scale = (mono_float.len() - 1) as f32 / (output_len - 1) as f32;
        
        for i in 0..output_len {
            let pos = i as f32 * scale;
            let index = pos as usize;
            let frac = pos - index as f32;
            
            let sample = if index + 1 < mono_float.len() {
                mono_float[index] * (1.0 - frac) + mono_float[index + 1] * frac
            } else {
                mono_float[index]
            };
            
            let sample_i16 = (sample * 32767.0).min(32767.0).max(-32768.0) as i16;
            writer.write_sample(sample_i16)?;
        }
    } else {
        // No resampling needed, just write mono samples
        for sample in mono_samples {
            writer.write_sample(sample)?;
        }
    }
    
    writer.finalize()?;
    
    // Clean up temporary file
    std::fs::remove_file(temp_file)?;

    Ok(())
}
