use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use hound;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::error::Error;

pub fn playback_audio(file_path: &str, is_playing_flag: Arc<AtomicBool>) -> Result<(), Box<dyn Error>> {
    let mut reader = hound::WavReader::open(file_path)?;
    let spec = reader.spec();
    
    println!("Playing audio: channels={}, sample_rate={}, bits={}, format={:?}",
             spec.channels, spec.sample_rate, spec.bits_per_sample, spec.sample_format);
    
    let host = cpal::default_host();
    let device = host.default_output_device().expect("No output device available");
    
    // Use default config instead of matching the file's sample rate
    let config = device.default_output_config()?;
    let _sample_format = config.sample_format();
    let config = config.config();
    
    // Read all samples into memory
    let samples: Vec<f32> = if spec.sample_format == hound::SampleFormat::Float {
        reader.samples::<f32>().map(|s| s.unwrap()).collect()
    } else {
        reader.samples::<i16>().map(|s| s.unwrap() as f32 / 32768.0).collect()
    };
    
    // Create Arc before moving into closure
    let samples_arc = Arc::new(samples);
    let samples_for_stream = Arc::clone(&samples_arc);
    let sample_index = Arc::new(Mutex::new(0usize));
    let sample_index_for_stream = Arc::clone(&sample_index);
    let is_playing_for_stream = Arc::clone(&is_playing_flag);
    
    let stream = device.build_output_stream(
        &config,
        move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
            let mut index = sample_index_for_stream.lock().unwrap();
            let samples = &*samples_for_stream;
            
            for frame in data.chunks_mut(config.channels as usize) {
                if !is_playing_for_stream.load(Ordering::Relaxed) || *index >= samples.len() {
                    // Fill with silence and stop
                    for sample in frame.iter_mut() {
                        *sample = 0.0;
                    }
                    
                    if *index >= samples.len() {
                        is_playing_for_stream.store(false, Ordering::Relaxed);
                    }
                    
                    continue;
                }
                
                // Copy samples to output
                for (i, sample) in frame.iter_mut().enumerate() {
                    let channel_index = i % spec.channels as usize;
                    let sample_pos = *index + channel_index;
                    
                    if sample_pos < samples.len() {
                        *sample = samples[sample_pos];
                    } else {
                        *sample = 0.0;
                    }
                }
                
                *index += spec.channels as usize;
            }
        },
        |err| eprintln!("Playback error: {:?}", err),
        None,
    )?;
    
    stream.play()?;
    
    // Use the original Arc references here
    let samples_len = samples_arc.len();
    
    while is_playing_flag.load(Ordering::Relaxed) {
        std::thread::sleep(std::time::Duration::from_millis(100));
        
        // Print playback progress
        let index = *sample_index.lock().unwrap();
        let progress = if samples_len > 0 {
            (index as f32 / samples_len as f32) * 100.0
        } else {
            0.0
        };
        
        println!("Playback progress: {:.1}%", progress);
    }
    
    Ok(())
}
