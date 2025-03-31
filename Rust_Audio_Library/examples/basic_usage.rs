use rustio::RusticAudio;

fn main() {
    println!("RusticAudio library - Example usage");
    
    // Create a new instance of RusticAudio
    let mut audio = RusticAudio::new();
    
    // Configure audio processing parameters
    audio.processor.threshold_db = -30.0;                // Spectral gate threshold
    audio.processor.amplitude_threshold_db = -40.0;      // Amplitude gate threshold
    audio.processor.amplitude_attack_ms = 5.0;           // Attack time for amplitude gate
    audio.processor.amplitude_release_ms = 50.0;         // Release time for amplitude gate
    audio.processor.gain_db = 3.0;                       // Gain boost in dB
    audio.processor.limiter_threshold_db = -1.0;         // Limiter threshold
    audio.processor.limiter_release_ms = 50.0;           // Limiter release time
    audio.processor.lowpass_freq = 20000.0;              // Lowpass filter cutoff frequency
    audio.processor.highpass_freq = 75.0;                // Highpass filter cutoff frequency
    audio.processor.rms_target_db = -18.0;               // Target RMS level for normalization
    
    // Enable/disable specific processing stages
    audio.processor.rms_enabled = true;                  // Enable RMS normalization
    audio.processor.filters_enabled = true;              // Enable filters
    audio.processor.spectral_gate_enabled = true;        // Enable spectral gate
    audio.processor.amplitude_gate_enabled = true;       // Enable amplitude gate
    audio.processor.gain_boost_enabled = false;          // Disable gain boost
    audio.processor.limiter_enabled = true;              // Enable limiter
    
    // Configure Opus encoder
    audio.set_opus_bitrate(16000);                       // Set Opus bitrate to 16 kbps
    
    println!("Library configured with the following parameters:");
    println!("  Spectral gate threshold: {} dB", audio.processor.threshold_db);
    println!("  Amplitude gate threshold: {} dB", audio.processor.amplitude_threshold_db);
    println!("  Highpass filter: {} Hz", audio.processor.highpass_freq);
    println!("  Lowpass filter: {} Hz", audio.processor.lowpass_freq);
    println!("  RMS target level: {} dB", audio.processor.rms_target_db);
    println!("  Opus bitrate: {} bps", audio.get_opus_bitrate());
    
    println!("For more examples, see the documentation");
} 