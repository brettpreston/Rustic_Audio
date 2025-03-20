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
    let sample_rate = config.sample_rate();
    let config = config.config();

    println!("Recording with: format={:?}, rate={}, channels={}", 
             sample_format, sample_rate.0, channels);

    let spec = hound::WavSpec {
        channels,
        sample_rate: sample_rate.0,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    
    let writer = Arc::new(Mutex::new(Some(hound::WavWriter::create(file_path, spec)?)));
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
    
    if let Ok(metadata) = std::fs::metadata(file_path) {
        println!("Output file size: {} bytes", metadata.len());
    }

    Ok(())
}
