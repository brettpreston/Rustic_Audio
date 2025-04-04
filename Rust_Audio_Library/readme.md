```markdown
# Rustic Audio Tool

`RusticAudio` is a Rust library for recording, processing, and playing back audio with support for DSP effects and Opus compression. It provides a simple API for handling audio files, including recording, playback, and encoding.

## Features

- Record audio to WAV files.
- Apply DSP effects to audio files.
  - RMS normalization
  - Spectral noise gate
  - Amplitude gate
  - High-pass and low-pass filters
  - Gain boost
  - Lookahead limiter
- Encode audio to Opus format.
- Playback original, processed, and Opus-encoded audio.
- Thread-safe operations with atomic flags for state management.

## Installation

Add the following to your `Cargo.toml`:

```toml
[dependencies]
rustic_audio_tool = "0.0.1"
```

## Linux Debian/Ubuntu-based Dependencies Installation
```bash
sudo apt-get install libasound2-dev pkg-config
sudo apt-get install libopus-dev
```

## Usage

### Creating an Instance

```rust
use rustic_audio_tool::RusticAudio;

let mut audio_tool = RusticAudio::new();
```

### Recording Audio

```rust
if let Err(e) = audio_tool.start_recording("output.wav") {
    eprintln!("Failed to start recording: {}", e);
}

// Stop recording
if let Err(e) = audio_tool.stop_recording() {
    eprintln!("Failed to stop recording: {}", e);
}
```

### Playing Audio

- **Play Original WAV:**
  ```rust
  if let Err(e) = audio_tool.play_original_wav("output_original.wav") {
      eprintln!("Failed to play original WAV: {}", e);
  }
  ```

- **Play Processed WAV:**
  ```rust
  if let Err(e) = audio_tool.play_processed_wav("output_processed.wav") {
      eprintln!("Failed to play processed WAV: {}", e);
  }
  ```

- **Play Unprocessed Opus:**
  ```rust
  if let Err(e) = audio_tool.play_unprocessed_opus("output_unprocessed.opus") {
      eprintln!("Failed to play unprocessed Opus: {}", e);
  }
  ```

- **Play Processed Opus:**
  ```rust
  if let Err(e) = audio_tool.play_processed_opus("output_processed.opus") {
      eprintln!("Failed to play processed Opus: {}", e);
  }
  ```

- **Stop Playback:**
  ```rust
  if let Err(e) = audio_tool.stop_playback() {
      eprintln!("Failed to stop playback: {}", e);
  }
  ```

### Processing Audio

- **Apply DSP Effects:**
  ```rust
  if let Err(e) = audio_tool.process_file("input.wav", "output_processed.wav") {
      eprintln!("Failed to process file: {}", e);
  }
  ```

- **Encode to Opus:**
  ```rust
  if let Err(e) = audio_tool.encode_to_opus("input.wav", "output.opus") {
      eprintln!("Failed to encode to Opus: {}", e);
  }
  ```

### Managing Opus Settings

- **Set Opus Bitrate:**
  ```rust
  audio_tool.set_opus_bitrate(64000); // Set bitrate to 64 kbps
  ```

- **Get Opus Bitrate:**
  ```rust
  let bitrate = audio_tool.get_opus_bitrate();
  println!("Current Opus bitrate: {} kbps", bitrate);
  ```

### Querying Audio Information

```rust
let info = audio_tool.get_audio_info();
println!("File size: {} bytes", info.file_size);
println!("Duration: {} seconds", info.duration);
println!("Last message: {}", info.last_message);
```

### Checking States

- **Check if Recording:**
  ```rust
  if audio_tool.is_recording() {
      println!("Currently recording...");
  }
  ```

- **Check if Playing:**
  ```rust
  if audio_tool.is_playing() {
      println!("Currently playing...");
  }
  ```

## Flags

The library uses the following flags to manage state:

- `is_recording`: Indicates if recording is in progress.
- `is_playing`: Indicates if processed audio playback is in progress.
- `is_playing_original`: Indicates if original WAV playback is in progress.
- `is_playing_unprocessed_opus`: Indicates if unprocessed Opus playback is in progress.

---

### DSP Settings

The `AudioProcessor` provides several configurable DSP settings to control audio processing. These settings can be adjusted to enable or disable specific effects or fine-tune their behavior.

#### **Available DSP Settings**

| Setting                     | Type    | Default Value | Description                                                                 |
|-----------------------------|---------|---------------|-----------------------------------------------------------------------------|
| `sample_rate`               | `f32`  | `48000.0`     | The sample rate of the audio in Hz.                                         |
| `threshold_db`              | `f32`  | `5.0`         | Threshold in dB for the spectral noise gate.                                |
| `amplitude_threshold_db`    | `f32`  | `-20.0`       | Threshold in dB for the amplitude gate.                                     |
| `amplitude_attack_ms`       | `f32`  | `10.0`        | Attack time in milliseconds for the amplitude gate.                         |
| `amplitude_release_ms`      | `f32`  | `100.0`       | Release time in milliseconds for the amplitude gate.                        |
| `amplitude_lookahead_ms`    | `f32`  | `5.0`         | Lookahead time in milliseconds for the amplitude gate.                      |
| `gain_db`                   | `f32`  | `6.0`         | Gain boost in dB.                                                           |
| `limiter_threshold_db`      | `f32`  | `-1.0`        | Threshold in dB for the lookahead limiter.                                  |
| `limiter_release_ms`        | `f32`  | `50.0`        | Release time in milliseconds for the limiter.                               |
| `limiter_lookahead_ms`      | `f32`  | `5.0`         | Lookahead time in milliseconds for the limiter.                             |
| `lowpass_freq`              | `f32`  | `20000.0`     | Low-pass filter cutoff frequency in Hz.                                     |
| `highpass_freq`             | `f32`  | `75.0`        | High-pass filter cutoff frequency in Hz.                                    |
| `rms_target_db`             | `f32`  | `-20.0`       | Target RMS level in dB for normalization.                                   |
| `rms_enabled`               | `bool` | `true`        | Enables or disables RMS normalization.                                      |
| `filters_enabled`           | `bool` | `true`        | Enables or disables high-pass and low-pass filters.                         |
| `spectral_gate_enabled`     | `bool` | `true`        | Enables or disables the spectral noise gate.                                |
| `amplitude_gate_enabled`    | `bool` | `true`        | Enables or disables the amplitude gate.                                     |
| `gain_boost_enabled`        | `bool` | `false`       | Enables or disables gain boosting.                                          |
| `limiter_enabled`           | `bool` | `true`        | Enables or disables the lookahead limiter.                                  |

#### **Example: Configuring DSP Settings**

You can customize the DSP settings by modifying the `AudioProcessor` instance:

```rust
use rustic_audio_tool::AudioProcessor;

let mut processor = AudioProcessor::new(48000.0); // Set sample rate to 48 kHz

// Enable or disable specific effects
processor.rms_enabled = true;
processor.filters_enabled = true;
processor.spectral_gate_enabled = true;

// Adjust parameters
processor.gain_db = 10.0; // Increase gain boost to 10 dB
processor.lowpass_freq = 20000.0; // Set low-pass filter cutoff to 20 kHz
processor.highpass_freq = 75.0; // Set high-pass filter cutoff to 75 Hz
processor.limiter_threshold_db = -3.0; // Set limiter threshold to -3 dB
```

#### **Processing an Audio File**

Once the DSP settings are configured, you can process an audio file:

```rust
if let Err(e) = processor.process_file("input.wav", "output_processed.wav") {
    eprintln!("Failed to process file: {}", e);
}
```

---

### Opus Encoding Settings

The `OpusEncoder` provides configurable settings for encoding audio to the Opus format. These settings allow you to control the bitrate and other encoding parameters.

#### **Available Settings**

| Setting       | Type    | Default Value | Description                                                                 |
|---------------|---------|---------------|-----------------------------------------------------------------------------|
| `channels`    | `Channels` | `Mono`        | The number of audio channels (`Mono` or `Stereo`).                          |
| `bitrate`     | `i32`   | `12000`       | The bitrate for Opus encoding in bits per second (e.g., 12000 for 12 kbps). |

#### **Example: Configuring Opus Encoder**

You can customize the Opus encoder settings by modifying the `OpusEncoder` instance:

```rust
use rustic_audio_tool::OpusEncoder;

let mut encoder = OpusEncoder::new();

// Set the bitrate to 12 kbps
encoder.set_bitrate(12000);

// Get the current bitrate
let bitrate = encoder.get_bitrate();
println!("Current Opus bitrate: {} bps", bitrate);
```

#### **Encoding a WAV File to Opus**

To encode a WAV file to Opus format, use the `encode_wav_to_opus` method:

```rust
if let Err(e) = encoder.encode_wav_to_opus("input.wav", "output.opus") {
    eprintln!("Failed to encode WAV to Opus: {}", e);
}
```

#### **How the Encoding Works**

1. **Resampling**: If the input WAV file is not 48 kHz, it will be resampled to 48 kHz.
2. **Mono Conversion**: If the input WAV file is stereo, it will be converted to mono by isolating the left channel.
3. **Encoding**: The audio is encoded to Opus format using the specified bitrate and channel configuration.
4. **Output**: The encoded Opus file is saved to the specified output path.

#### **Opus Header and Metadata**

The encoder automatically adds the following metadata to the Opus file:
- **OpusHead**: Contains information about the Opus stream (e.g., version, channel count, sample rate).
- **OpusTags**: Contains vendor information.

---

## License

This project is licensed under the GPLv3 License. See the LICENSE file for details.
```