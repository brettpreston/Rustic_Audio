use rustfft::num_complex::Complex;
use rustfft::FftPlanner;
use rustfft::num_traits::Zero;
use std::collections::VecDeque;

#[derive(Clone)]
pub struct AudioProcessor {
    pub sample_rate: f32,
    pub threshold_db: f32,
    pub amplitude_threshold_db: f32,
    pub amplitude_attack_ms: f32,
    pub amplitude_release_ms: f32,
    pub amplitude_lookahead_ms: f32,
    pub gain_db: f32,
    pub limiter_threshold_db: f32,
    pub limiter_release_ms: f32,
    pub limiter_lookahead_ms: f32,
    pub lowpass_freq: f32,
    pub highpass_freq: f32,
    pub rms_target_db: f32,
    pub rms_enabled: bool,
    pub filters_enabled: bool,
    pub spectral_gate_enabled: bool,
    pub amplitude_gate_enabled: bool,
    pub gain_boost_enabled: bool,
    pub limiter_enabled: bool,
}
//AudioProcessor Default 
impl AudioProcessor {
    pub fn new(sample_rate: f32) -> Self {
        Self {
            sample_rate,
            threshold_db: 5.0,
            amplitude_threshold_db: -20.0,
            amplitude_attack_ms: 10.0,
            amplitude_release_ms: 100.0,
            amplitude_lookahead_ms: 5.0,
            gain_db: 6.0,
            limiter_threshold_db: -1.0,
            limiter_release_ms: 50.0,
            limiter_lookahead_ms: 5.0,
            lowpass_freq: 20000.0,
            highpass_freq: 75.0,
            rms_target_db: -20.0,
            rms_enabled: true,
            filters_enabled: true,
            spectral_gate_enabled: true,
            amplitude_gate_enabled: true,
            gain_boost_enabled: false,
            limiter_enabled: true,
        }
    }

    pub fn process_file(&mut self, input_path: &str, output_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Read input file
        let mut reader = hound::WavReader::open(input_path)?;
        let spec = reader.spec();
        self.sample_rate = spec.sample_rate as f32;
        
        // Read samples
        let mut samples: Vec<f32> = if spec.sample_format == hound::SampleFormat::Float {
            reader.samples::<f32>().map(|s| s.unwrap()).collect()
        } else {
            reader.samples::<i16>().map(|s| s.unwrap() as f32 / 32768.0).collect()
        };
        
        // Apply RMS normalization if enabled
        if self.rms_enabled {
            self.apply_rms_normalization(&mut samples);
        }
        
        // Apply processing in order, but only if enabled
        if self.filters_enabled {
            self.apply_filters(&mut samples);         // 1. Filters
        }
        if self.spectral_gate_enabled {
            self.apply_noise_gate(&mut samples);      // 2. Spectral Gate
        }
        if self.amplitude_gate_enabled {
            self.apply_amplitude_gate(&mut samples);  // 3. Amplitude Gate
        }
        if self.gain_boost_enabled {
            self.apply_gain_boost(&mut samples);      // 4. Gain Boost
        }
        if self.limiter_enabled {
            self.apply_lookahead_limiter(&mut samples); // 5. Limiter
        }
        
        // Apply a 200ms fade-in to avoid clicks
        self.apply_fade_in(&mut samples, 200.0);
        
        // Write output file - use the SAME spec as input
        let spec = hound::WavSpec {
            channels: spec.channels,           // Keep original channel count
            sample_rate: spec.sample_rate,     // Keep original sample rate
            bits_per_sample: spec.bits_per_sample,  // Keep original bit depth
            sample_format: spec.sample_format, // Keep original format
        };
        
        let mut writer = hound::WavWriter::create(output_path, spec)?;
        
        // Write samples in the original format
        match spec.sample_format {
            hound::SampleFormat::Float => {
                for &sample in &samples {
                    writer.write_sample(sample)?;
                }
            },
            hound::SampleFormat::Int => {
                for &sample in &samples {
                    let sample_i16 = (sample * 32767.0).min(32767.0).max(-32768.0) as i16;
                    writer.write_sample(sample_i16)?;
                }
            }
        }
        
        writer.finalize()?;
        Ok(())
    }

    // separate filter function
    fn apply_filters(&mut self, samples: &mut Vec<f32>) {
        let fft_size = 4096;
        let hop_size = fft_size / 2;
        
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(fft_size);
        let ifft = planner.plan_fft_inverse(fft_size);

        let window: Vec<f32> = (0..fft_size)
            .map(|n| {
                0.5 - 0.5 * (2.0 * std::f32::consts::PI * n as f32 / fft_size as f32).cos()
            })
            .collect();

        let mut output = vec![0.0; samples.len()];
        let mut normalization = vec![0.0; samples.len()];
        let mut pos = 0;

        while pos < samples.len() {
            let mut complex_input: Vec<Complex<f32>> = vec![Complex::zero(); fft_size];
            let copy_len = fft_size.min(samples.len() - pos);
            
            for i in 0..copy_len {
                complex_input[i] = Complex::new(samples[pos + i] * window[i], 0.0);
            }

            fft.process(&mut complex_input);

            for i in 0..complex_input.len() {
                let frequency = if i <= fft_size/2 {
                    i as f32
                } else {
                    i as f32 - fft_size as f32
                } * self.sample_rate / fft_size as f32;

                let freq_abs = frequency.abs();

                // Apply highpass and lowpass filters
                if freq_abs < self.highpass_freq || freq_abs > self.lowpass_freq {
                    complex_input[i] = Complex::zero();
                    continue;
                }

                if complex_input[i].norm() < 1e-10 {
                    complex_input[i] = Complex::zero();
                }
            }

            ifft.process(&mut complex_input);

            for i in 0..fft_size {
                if pos + i < output.len() {
                    output[pos + i] += complex_input[i].re * window[i] / fft_size as f32;
                    normalization[pos + i] += window[i] * window[i];
                }
            }

            pos += hop_size;
        }

        for i in 0..samples.len() {
            if normalization[i] > 1e-10 {
                output[i] /= normalization[i];
            }
        }

        samples.copy_from_slice(&output);
    }

    // Spectral noise gate function
    fn apply_noise_gate(&self, samples: &mut Vec<f32>) {
        let fft_size = 4096;
        let hop_size = fft_size / 2;
        
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(fft_size);
        let ifft = planner.plan_fft_inverse(fft_size);

        let window: Vec<f32> = (0..fft_size)
            .map(|n| {
                0.5 - 0.5 * (2.0 * std::f32::consts::PI * n as f32 / fft_size as f32).cos()
            })
            .collect();

        let mut output = vec![0.0; samples.len()];
        let mut normalization = vec![0.0; samples.len()];
        let mut pos = 0;

        let threshold = 10.0f32.powf(self.threshold_db / 20.0);

        while pos < samples.len() {
            let mut complex_input: Vec<Complex<f32>> = vec![Complex::zero(); fft_size];
            let copy_len = fft_size.min(samples.len() - pos);
            
            for i in 0..copy_len {
                complex_input[i] = Complex::new(samples[pos + i] * window[i], 0.0);
            }

            fft.process(&mut complex_input);

            // Apply spectral noise gate
            for i in 0..complex_input.len() {
                let magnitude = complex_input[i].norm();
                if magnitude < threshold {
                    complex_input[i] = Complex::zero();
                }
            }

            ifft.process(&mut complex_input);

            for i in 0..fft_size {
                if pos + i < output.len() {
                    output[pos + i] += complex_input[i].re * window[i] / fft_size as f32;
                    normalization[pos + i] += window[i] * window[i];
                }
            }

            pos += hop_size;
        }

        for i in 0..samples.len() {
            if normalization[i] > 1e-10 {
                output[i] /= normalization[i];
            }
        }

        samples.copy_from_slice(&output);
    }
    
    // amplitude gate function
    fn apply_amplitude_gate(&self, samples: &mut Vec<f32>) {
        let threshold = 10.0f32.powf(self.amplitude_threshold_db / 20.0);
        let lookahead_samples = (self.amplitude_lookahead_ms / 1000.0 * self.sample_rate) as usize;
        let attack_coef = (-2.2 / (self.amplitude_attack_ms / 1000.0 * self.sample_rate)).exp();
        let release_coef = (-2.2 / (self.amplitude_release_ms / 1000.0 * self.sample_rate)).exp();
        
        let mut lookahead_buffer = VecDeque::with_capacity(lookahead_samples + 1);
        let mut gate_gain = 0.0;
        let mut output = vec![0.0; samples.len()];
        let mut output_idx = 0;
        
        // Pre-fill lookahead buffer
        for _ in 0..lookahead_samples {
            lookahead_buffer.push_back(0.0);
        }
        
        // Process all input samples
        for &sample in samples.iter() {
            lookahead_buffer.push_back(sample);
            
            // Find peak in lookahead window
            let peak = lookahead_buffer.iter().map(|&s| s.abs()).fold(0.0, f32::max);
            
            // Calculate target gate gain
            let target_gain = if peak >= threshold { 1.0 } else { 0.0 };
            
            // Apply attack/release smoothing
            if target_gain > gate_gain {
                gate_gain = gate_gain * attack_coef + target_gain * (1.0 - attack_coef);
            } else {
                gate_gain = gate_gain * release_coef + target_gain * (1.0 - release_coef);
            }
            
            // Apply gain to the oldest sample in buffer
            if let Some(oldest_sample) = lookahead_buffer.pop_front() {
                if output_idx < output.len() {
                    output[output_idx] = oldest_sample * gate_gain;
                    output_idx += 1;
                }
            }
        }
        
        // Process remaining samples in buffer
        while !lookahead_buffer.is_empty() && output_idx < output.len() {
            if let Some(oldest_sample) = lookahead_buffer.pop_front() {
                output[output_idx] = oldest_sample * gate_gain;
                output_idx += 1;
            }
        }
        
        samples.copy_from_slice(&output);
    }
    
    // gain boost function
    fn apply_gain_boost(&self, samples: &mut Vec<f32>) {
        let gain_linear = 10.0f32.powf(self.gain_db / 20.0);
        
        for sample in samples.iter_mut() {
            *sample *= gain_linear;
        }
    }
    
    // lookahead limiter function
    fn apply_lookahead_limiter(&self, samples: &mut Vec<f32>) {
        let threshold = 10.0f32.powf(self.limiter_threshold_db / 20.0);
        let lookahead_samples = (self.limiter_lookahead_ms / 1000.0 * self.sample_rate) as usize;
        let release_coef = (-2.2 / (self.limiter_release_ms / 1000.0 * self.sample_rate)).exp();
        
        let mut lookahead_buffer = VecDeque::with_capacity(lookahead_samples + 1);
        let mut gain_reduction = 1.0;
        
        let mut output = vec![0.0; samples.len()];  // Initialize with correct size
        let mut output_idx = 0;
        
        // Pre-fill lookahead buffer
        for _ in 0..lookahead_samples {
            lookahead_buffer.push_back(0.0);
        }
        
        // Process all input samples
        for &sample in samples.iter() {
            // Add sample to lookahead buffer
            lookahead_buffer.push_back(sample);
            
            // Find peak in lookahead window
            let peak = lookahead_buffer.iter().map(|&s| s.abs()).fold(0.0, f32::max);
            
            // Calculate target gain reduction
            let target_gain = if peak > threshold {
                threshold / peak
            } else {
                1.0
            };
            
            // Apply release time (smoothing)
            if target_gain < gain_reduction {
                gain_reduction = target_gain; // Attack is instant
            } else {
                gain_reduction = gain_reduction * release_coef + target_gain * (1.0 - release_coef);
            }
            
            // Apply gain reduction to the oldest sample in buffer
            if let Some(oldest_sample) = lookahead_buffer.pop_front() {
                if output_idx < output.len() {
                    output[output_idx] = oldest_sample * gain_reduction;
                    output_idx += 1;
                }
            }
        }
        
        // Process remaining samples in buffer
        while !lookahead_buffer.is_empty() && output_idx < output.len() {
            if let Some(oldest_sample) = lookahead_buffer.pop_front() {
                output[output_idx] = oldest_sample * gain_reduction;
                output_idx += 1;
            }
        }
        
        // Ensure output length matches input length
        output.truncate(samples.len());
        samples.copy_from_slice(&output);
    }

    // The Root Mean Square (RMS) normalization function
    fn apply_rms_normalization(&self, samples: &mut Vec<f32>) {
        // Calculate current RMS
        let rms_current = (samples.iter().map(|&x| x * x).sum::<f32>() / samples.len() as f32).sqrt();
        let rms_current_db = 20.0 * rms_current.log10();
        
        // Convert target RMS from dB to linear
        let target_rms = 10.0f32.powf(self.rms_target_db / 20.0);
        
        // Calculate gain factor
        let gain_factor = target_rms / rms_current;
        
        println!("RMS Normalization:");
        println!("  Current RMS: {:.2} dB", rms_current_db);
        println!("  Target RMS: {:.2} dB", self.rms_target_db);
        println!("  Gain factor: {:.2}x", gain_factor);
        
        // Apply gain with peak limiting
        for sample in samples.iter_mut() {
            // Apply gain
            *sample *= gain_factor;
            
            // Apply soft clipping to prevent hard clipping
            if *sample > 0.95 {
                *sample = 0.95 + (1.0 - 0.95) * (1.0 - (1.0 - (*sample - 0.95) / (1.0 - 0.95)).powi(2));
            } else if *sample < -0.95 {
                *sample = -0.95 - (1.0 - 0.95) * (1.0 - (1.0 - (-*sample - 0.95) / (1.0 - 0.95)).powi(2));
            }
            
            // Hard limit as a safety measure
            *sample = sample.max(-1.0).min(1.0);
        }
        
        // Calculate new RMS after normalization
        let new_rms = (samples.iter().map(|&x| x * x).sum::<f32>() / samples.len() as f32).sqrt();
        let new_rms_db = 20.0 * new_rms.log10();
        
        println!("  New RMS after normalization: {:.2} dB", new_rms_db);
    }

    // Add a fade-in function to the processor
    fn apply_fade_in(&self, samples: &mut Vec<f32>, fade_ms: f32) {
        let fade_samples = (fade_ms / 1000.0 * self.sample_rate) as usize;
        let fade_samples = fade_samples.min(samples.len());
        
        println!("Applying {:.0}ms fade-in ({} samples)", fade_ms, fade_samples);
        
        for i in 0..fade_samples {
            let gain = (i as f32) / (fade_samples as f32);
            // Use a smooth curve for the fade (cubic)
            let smooth_gain = gain * gain * (3.0 - 2.0 * gain);
            samples[i] *= smooth_gain;
        }
    }
}

impl Default for AudioProcessor {
    fn default() -> Self {
        Self::new(48000.0) 
    }
}