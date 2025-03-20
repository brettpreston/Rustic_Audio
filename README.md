# Rustio

Rustio is a voice recorder application with a DSP effects chain for audio cleaning and a small file Opus encoder. This project is licensed under the GPL-3.0 License.

## Features

- Record audio from the default input device
- Apply DSP effects for audio cleaning
- Encode processed audio to Opus format
- Playback original and processed audio

## DSP Effects and Settings

The DSP effects chain includes the following effects:

1. **Filters**
   - **Highpass Filter**: Removes frequencies below a certain threshold.
     - `highpass_freq`: Frequency threshold in Hz (20.0 - 1000.0 Hz)
   - **Lowpass Filter**: Removes frequencies above a certain threshold.
     - `lowpass_freq`: Frequency threshold in Hz (1000.0 - 20000.0 Hz)

2. **Spectral Gate**
   - Reduces noise by applying a threshold to the frequency spectrum.
     - `threshold_db`: Threshold in dB (-50.0 - 24.0 dB)

3. **Noise Gate**
   - Reduces noise by applying a threshold to the amplitude.
     - `amplitude_threshold_db`: Threshold in dB (-60.0 - 0.0 dB)
     - `amplitude_attack_ms`: Attack time in milliseconds (0.1 - 100.0 ms)
     - `amplitude_release_ms`: Release time in milliseconds (1.0 - 1000.0 ms)
     - `amplitude_lookahead_ms`: Lookahead time in milliseconds (0.0 - 20.0 ms)

4. **Gain Booster**
   - Increases the amplitude of the audio signal.
     - `gain_db`: Gain in dB (0.0 - 24.0 dB)

5. **Lookahead Limiter**
   - Limits the amplitude of the audio signal to prevent clipping.
     - `limiter_threshold_db`: Threshold in dB (-12.0 - 0.0 dB)
     - `limiter_release_ms`: Release time in milliseconds (10.0 - 500.0 ms)
     - `limiter_lookahead_ms`: Lookahead time in milliseconds (1.0 - 20.0 ms)

## License

This project is licensed under the GPL-3.0 License. See the LICENSE file for more details.
