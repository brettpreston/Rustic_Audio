use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use hound;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::error::Error;

// Function to resample audio
fn resample_audio(input_samples: &[f32], input_rate: f32, output_rate: f32) -> Vec<f32> {
    let input_duration = input_samples.len() as f32 / input_rate;
    let output_len = (input_duration * output_rate) as usize;
    let scale = (input_samples.len() - 1) as f32 / (output_len - 1).max(1) as f32;
    
    let mut output = Vec::with_capacity(output_len);
    for i in 0..output_len {
        let pos = i as f32 * scale;
        let index = pos.floor() as usize;
        let frac = pos - index as f32;
        
        let sample = if index + 1 < input_samples.len() {
            input_samples[index] * (1.0 - frac) + input_samples[index + 1] * frac
        } else {
            input_samples[index.min(input_samples.len() - 1)]
        };
        
        output.push(sample);
    }
    
    output
}

pub fn playback_audio(file_path: &str, is_playing_flag: Arc<AtomicBool>) -> Result<(), Box<dyn Error>> {
    let mut reader = hound::WavReader::open(file_path)?;
    let spec = reader.spec();
    
    println!("Playing audio: channels={}, sample_rate={}, bits={}, format={:?}",
             spec.channels, spec.sample_rate, spec.bits_per_sample, spec.sample_format);
    
    let host = cpal::default_host();
    let device = host.default_output_device().expect("No output device available");
    
    // Get default config for sample format
    let default_config = device.default_output_config()?;
    let default_sample_rate = default_config.sample_rate().0;
    
    // Read all samples into memory
    let mut samples: Vec<f32> = if spec.sample_format == hound::SampleFormat::Float {
        reader.samples::<f32>().map(|s| s.unwrap()).collect()
    } else {
        reader.samples::<i16>().map(|s| s.unwrap() as f32 / 32768.0).collect()
    };
    
    // Try to use the WAV file's sample rate
    let stream_config = cpal::StreamConfig {
        channels: default_config.channels(),
        sample_rate: cpal::SampleRate(spec.sample_rate),
        buffer_size: cpal::BufferSize::Default,
    };
    
    // Try to build the stream with file's sample rate
    let stream_result = device.build_output_stream(
        &stream_config,
        |_: &mut [f32], _: &cpal::OutputCallbackInfo| { /* Empty callback */ },
        |err| eprintln!("Error initializing stream: {:?}", err),
        None,
    );
    
    let mut using_original_rate = true;
    let sample_index = Arc::new(Mutex::new(0usize));
    
    // If the original sample rate isn't supported, resample to device rate
    if stream_result.is_err() {
        println!("WAV sample rate {} not supported by device, resampling to {}", 
                 spec.sample_rate, default_sample_rate);
        samples = resample_audio(&samples, spec.sample_rate as f32, default_sample_rate as f32);
        using_original_rate = false;
    } else {
        println!("Using original sample rate: {}", spec.sample_rate);
    }
    
    // Store samples in Arc for thread safety
    let samples_arc = Arc::new(samples);
    
    // Create the actual playback stream with appropriate sample rate
    let config = if using_original_rate {
        stream_config
    } else {
        cpal::StreamConfig {
            channels: default_config.channels(),
            sample_rate: cpal::SampleRate(default_sample_rate),
            buffer_size: cpal::BufferSize::Default,
        }
    };
    
    let samples_for_stream = Arc::clone(&samples_arc);
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
