use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use hound;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub fn playback_audio(file_path: &str, is_playing_flag: Arc<AtomicBool>) -> Result<(), Box<dyn std::error::Error>> {
    let host = cpal::default_host();
    let device = host.default_output_device().expect("Failed to get default output device");
    let config = device.default_output_config()?;

    let reader = Arc::new(std::sync::Mutex::new(
        hound::WavReader::open(file_path)?
    ));
    let _spec = reader.lock().unwrap().spec();
    let sample_format = config.sample_format();

    let is_playing_clone = is_playing_flag.clone();

    let stream = match sample_format {
        cpal::SampleFormat::I16 => {
            let reader = Arc::clone(&reader);
            device.build_output_stream(
                &config.config(),
                move |output: &mut [i16], _| {
                    if is_playing_clone.load(Ordering::Relaxed) {
                        let mut reader = reader.lock().unwrap();
                        for out in output.iter_mut() {
                            if let Some(Ok(sample)) = reader.samples::<i16>().next() {
                                *out = sample;
                            } else {
                                // End of file or error, stop playback
                                is_playing_clone.store(false, Ordering::Relaxed);
                                *out = 0;
                            }
                        }
                    } else {
                        // Output silence when not playing
                        for out in output.iter_mut() {
                            *out = 0;
                        }
                    }
                },
                |err| eprintln!("Error: {:?}", err),
                None,
            )?
        },
        cpal::SampleFormat::F32 => {
            let reader = Arc::clone(&reader);
            device.build_output_stream(
                &config.config(),
                move |output: &mut [f32], _| {
                    if is_playing_clone.load(Ordering::Relaxed) {
                        let mut reader = reader.lock().unwrap();
                        for out in output.iter_mut() {
                            if let Some(Ok(sample)) = reader.samples::<i16>().next() {
                                *out = sample as f32 / i16::MAX as f32;
                            } else {
                                // End of file or error, stop playback
                                is_playing_clone.store(false, Ordering::Relaxed);
                                *out = 0.0;
                            }
                        }
                    } else {
                        // Output silence when not playing
                        for out in output.iter_mut() {
                            *out = 0.0;
                        }
                    }
                },
                |err| eprintln!("Error: {:?}", err),
                None,
            )?
        },
        cpal::SampleFormat::U16 => {
            let reader = Arc::clone(&reader);
            device.build_output_stream(
                &config.config(),
                move |output: &mut [u16], _| {
                    if is_playing_clone.load(Ordering::Relaxed) {
                        let mut reader = reader.lock().unwrap();
                        for out in output.iter_mut() {
                            if let Some(Ok(sample)) = reader.samples::<i16>().next() {
                                *out = (sample as i32 + i16::MAX as i32) as u16;
                            } else {
                                // End of file or error, stop playback
                                is_playing_clone.store(false, Ordering::Relaxed);
                                *out = 32768; // Midpoint for u16 (silence)
                            }
                        }
                    } else {
                        // Output silence when not playing
                        for out in output.iter_mut() {
                            *out = 32768; // Midpoint for u16 (silence)
                        }
                    }
                },
                |err| eprintln!("Error: {:?}", err),
                None,
            )?
        },
        _ => return Err("Unsupported sample format".into()),
    };

    stream.play()?;

    // Wait while playing is true
    while is_playing_flag.load(Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    // Ensure the stream is dropped
    drop(stream);
    
    Ok(())
}
