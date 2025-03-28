

# Rustic Audio

Welcome to the Rustic Audio tool set. It allows you to record voice, apply a DSP effects chain tailored to cleaning voice recordings, and encode it as a highly compressed Opus file. It was created for settings where bandwidth is scarce, such as powerful privacy-preserving communication, but we still want to preserve the legibility and quality of voice recording. Any project interested in low-footprint, efficiently computed and good quality voice recordings may be interested in using it.

The presets are an effects chain that reliably produces clean, good quality voice audio, and guarantees that a 30sec recording will result in a file no larger than 45kBs. However, all parameters can be easily adjusted for other settings.

The **Rust_Audio_Gui** repo includes a generic GUI for testing and debugging the software. It allows for recording of a voice sample, and playing it back both unprocessed and processed. It uses the egui framework, and allows for changing of the DSP parameters interactively to achieve the desired effect.

The **Rust_Audio_Library** is the light version, with no GUI or associated dependencies, that you may want to use in your project. It can be found on crates.io as **rustic-audio**.

This is a graph of spectral information of a sample audio recording, with the waveform overlaid on top for clarity. Before the processing (top) and after (bottom):

![enter image description here](https://brettpreston.github.io/img/before.jpeg) ![enter image description here](https://brettpreston.github.io/img/after.jpeg)

The implemented DSP effects are **Root Mean Square (RMS) Normalization**, **highpass filter**, **lowpass filter**, a **spectral gate** implementing a **Fast Fourier Transform (FFT) noise reduction**, **noise gate**, **gain booster** and a **lookahead limiter**. 

The default output is an **Opus** encoding at **12 kbps VBR mono** , with **complexity 10** and a frame size of **20ms**.

See the README file for usage and dependencies.

Below is an analysis which explains some of these decisions.

## Codec and encoding paremeters

Opus is the leading audio codec today, and with good reason. It is versatile, known for its high-quality, crisp sound reproduction. It can handle both voice communication and music streaming. It has ready implementations in many programming languages, and is well documented, simplifying its potential integration into various projects. I have briefly considered using Speex, and even Codec2 which is capable of truly impressive compression, but their quality is not nearly as reliable as with Opus. Codec 2 would also involve a lot of rather fundamental tweaks to be usable in a project like this one.

### Bitrate

The following figure comes from "Voice quality characterization of ietf opus codec" by  Anssi Rämö and Henri Toukomaa.[^1]

![enter image description here](https://brettpreston.github.io/img/opus.jpeg)
We can see from their research that Opus experiences diminishing returns in quality above 12 or 16 kbps, and so if we are looking for an optimum point between quality and file size, a bitrate in this range would be a reasonable point to pick. I have decided to make 12 kbps my default setting, since that results in 30 sec voice recordings coming in under 45 kBs, which was particularly useful for the first use case of Rustic Audio - the Katzenpost/Echomix mixnet. However, 16 kbps would have been a fine bitrate as well.

### Other codec parameters

I recommend using wide band compression as it has little impact on file size, and improves quality, and complexity 10 since we usually expect to use modern devices. Encoding with Opus, frames’ lengths under 20ms at low bit rates have audible distortions (as well as frames sizes over 80ms.) I would therefore recommend sticking to frame size of 20ms. It is also encoded in mono, since there is really no reason for multiple channels if your use case is voice communication.

I use VBR encoding. Now, if you are streaming encrypted audio and are worried about privacy you want to be careful about that. It has been shown[^2] that different phonemes correspond to peaks and valleys in the bitrate in VBR, and the content of voice communication can sometimes be reconstructed from the bandwidth graph. This is likely to get even worse as machine learning tools automate statistical analysis.[^3] However, I expected these files to be sent in chunks in a push-to-talk application, encrypted and padded to a certain packet size. So I chose VBR because to reduce the file size.

## The DSP pipeline

The tool set lets you customize any of this, but here is the effects chain I use as well as the reasons for it.

The tool set implements a digital signal processing (DSP) pipeline for audio processing. The main structure is the **AudioProcessor** struct, which contains various parameters and flags to control the processing steps. Here's a breakdown of how the DSP works:

  1. Reading the Input File

The process_file method starts by reading an input WAV file using the hound crate. It extracts the audio samples and converts them to a Vec<f32> for processing.

  2. Processing Pipeline

The audio samples are processed in a series of steps, each of which can be enabled or disabled using flags in the **AudioProcessor** struct. The steps are:

  * **RMS Normalization**. If **rms_enabled** is true, the apply_rms_normalization method adjusts the audio's root mean square (RMS) level to match a target value (rms_target_db). This ensures consistent loudness across audio files.
  * The information is converted to the frequency domain with an **FFT (Fast Fourier Transform)**.
* **Filters** If **filters_enabled** is true, the **apply_filters** method applies high-pass and low-pass filters.
* **Spectral Noise Gate** If **spectral_gate_enabled** is true, the apply_noise_gate method reduces noise by zeroing out frequency components below a threshold (threshold_db).
* The information id converted back into the time domain.
* **Amplitude Gate** If **amplitude_gate_enabled** is true, the apply_amplitude_gate method applies a gate based on the amplitude of the signal. It uses attack, release, and lookahead parameters to smooth transitions.
* **Gain Boost** If **gain_boost_enabled** is true, the apply_gain_boost method amplifies the signal by a specified gain (gain_db).
* **Lookahead Limiter** If **limiter_enabled** is true, the **apply_lookahead_limiter** method prevents clipping by dynamically reducing the gain of the signal when it exceeds a threshold (limiter_threshold_db). It uses lookahead and release parameters for smooth operation.
* **Fade-In** The **apply_fade_in** method applies a fade-in effect over a specified duration (**fade_ms**) to avoid clicks at the start of the audio.

  

3. Writing the Output File

After processing, the samples are written back to a new WAV file using the same format as the input file. The **hound::WavWriter** is used for this purpose.
  
4. Key DSP Techniques

**FFT and IFFT**: Used for frequency-domain processing (filters and noise gate).

**Windowing**: A Hamming window is applied to reduce spectral leakage during FFT.

**Lookahead Buffers**: Used in the amplitude gate and limiter to anticipate future samples and apply smoother transitions.

**Soft Clipping**: Prevents hard clipping during RMS normalization by applying a non-linear curve to limit peaks.

5. Customizability

The **AudioProcessor** struct allows fine-grained control over the DSP pipeline through its parameters and flags. For example, you can adjust the filter cutoff frequencies, gain, or limiter threshold, or enable and disable specific processing steps.

6. Error Handling

The **process_file** method uses **Result** to handle errors gracefully, such as file I/O issues or invalid sample formats.

  

## The Katzenpost use case

Rustic Audio was originally built for use in the Katzenpost/Echomix anonymity system, a powerful privacy tool that aims to leak the least amount of information about you it can. No information about when or to whom you're talking, or in fact if you're talking at all. Bandwidth is a precious resource there. In https://github.com/katzenpost/echo you can find an example implementation of Rustic Audio for communication.


[^1]: Anssi Rämö and Henri Toukomaa. Voice quality characterization of ietf opus codec. In Interspeech, 2011

[^2]: Andrew M. White, Austin R. Matthews, Kevin Z. Snow, and Fabian Monrose. Phonotactic reconstruction of encrypted voip conversations: Hookt on fon-iks, 2011.

[^3]: Chenggang Wang, Sean Kennedy, Haipeng Li, King Hudson, Gowtham Atluri, Xuetao Wei, Wenhai Sun, and Boyang Wang. Fingerprinting encrypted voice traffic on smart speakers with deep learning, 07 2020

