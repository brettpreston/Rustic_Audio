use audiopus::{Channels, Application, SampleRate, Bitrate};
use ogg::{PacketWriter, writing::PacketWriteEndInfo};
use std::fs::File;
use std::io::BufWriter;

#[derive(Clone)]
pub struct OpusEncoder {
    // Remove the unused field if it's not needed
    // sample_rate: SampleRate,
    channels: Channels,
    bitrate: i32,
}

impl OpusEncoder {
    pub fn new() -> Self {
        Self {
            // Remove from constructor if removed from struct
            // sample_rate: SampleRate::Hz48000,
            channels: Channels::Mono,
            bitrate: 12000, // Default 12kbps
        }
    }

    // Add setter for bitrate
    pub fn set_bitrate(&mut self, bitrate: i32) {
        self.bitrate = bitrate;
    }

    // Get current bitrate
    pub fn get_bitrate(&self) -> i32 {
        self.bitrate
    }

    pub fn encode_wav_to_opus(&self, input_path: &str, output_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        // Read the WAV file
        let mut reader = hound::WavReader::open(input_path)?;
        let spec = reader.spec();
        
        // Convert to 48kHz mono if needed
        let samples: Vec<f32> = if spec.sample_format == hound::SampleFormat::Float {
            reader.samples::<f32>().map(|s| s.unwrap()).collect()
        } else {
            reader.samples::<i16>().map(|s| s.unwrap() as f32 / 32768.0).collect()
        };
        
        // Convert to mono if stereo
        let mono_samples: Vec<f32> = if spec.channels == 2 {
            samples.chunks(2)
                .map(|chunk| chunk[0]) // Take left channel
                .collect()
        } else {
            samples
        };
        
        // Resample to 48kHz if needed
        let resampled_samples = if spec.sample_rate != 48000 {
            let input_duration = mono_samples.len() as f32 / spec.sample_rate as f32;
            let output_len = (input_duration * 48000.0) as usize;
            let scale = (mono_samples.len() - 1) as f32 / (output_len - 1) as f32;
            
            let mut output = Vec::with_capacity(output_len);
            for i in 0..output_len {
                let pos = i as f32 * scale;
                let index = pos as usize;
                let frac = pos - index as f32;
                
                let sample = if index + 1 < mono_samples.len() {
                    mono_samples[index] * (1.0 - frac) + mono_samples[index + 1] * frac
                } else {
                    mono_samples[index]
                };
                
                output.push(sample);
            }
            output
        } else {
            mono_samples
        };
        
        // Create Opus encoder
        let mut encoder = audiopus::coder::Encoder::new(
            SampleRate::Hz48000,
            self.channels,
            Application::Audio
        )?;
        
        encoder.set_bitrate(Bitrate::BitsPerSecond(self.bitrate))?;
        
        // Convert resampled_samples to i16 for encoding
        let samples_i16: Vec<i16> = resampled_samples.iter()
            .map(|&s| (s * 32767.0).round() as i16)
            .collect();
        
        println!("Converting to Opus:");
        println!("  Frame size: 960 samples (20ms at 48kHz)");
        println!("  Total frames: {}", samples_i16.len() / 960);

        let file = BufWriter::new(File::create(output_path)?);
        let serial = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs() as u32;
        let mut packet_writer = PacketWriter::new(file);

        // Opus header
        let mut id_header = Vec::new();
        id_header.extend_from_slice(b"OpusHead");
        id_header.push(1);  // Version
        id_header.push(1);  // Channel count
        id_header.extend_from_slice(&(0u16).to_le_bytes());  // Pre-skip
        id_header.extend_from_slice(&(48000u32).to_le_bytes());  // Input sample rate
        id_header.extend_from_slice(&[0, 0]);  // Output gain
        id_header.push(0);  // Channel mapping family

        packet_writer.write_packet(
            id_header.into(),
            serial,
            PacketWriteEndInfo::EndPage,
            0
        )?;

        // Comment header
        let mut comment_header = Vec::new();
        comment_header.extend_from_slice(b"OpusTags");
        let vendor = b"rustio";
        comment_header.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
        comment_header.extend_from_slice(vendor);
        comment_header.extend_from_slice(&[0, 0, 0, 0]);

        packet_writer.write_packet(
            comment_header.into(),
            serial,
            PacketWriteEndInfo::EndPage,
            0
        )?;

        let frame_size = 960;  // 20ms at 48kHz
        let mut input_buffer = vec![0i16; frame_size];
        let mut encoded_data = vec![0u8; 1275];
        let mut granulepos = 0i64;

        for chunk in samples_i16.chunks(frame_size) {
            input_buffer.clear();
            input_buffer.extend(chunk);
            if input_buffer.len() < frame_size {
                input_buffer.resize(frame_size, 0);
            }

            let encoded_len = encoder.encode(&input_buffer, &mut encoded_data)?;
            let encoded_packet = &encoded_data[..encoded_len];

            granulepos += frame_size as i64;

            packet_writer.write_packet(
                encoded_packet.to_vec().into(),
                serial,
                PacketWriteEndInfo::NormalPacket,
                granulepos as u64
            )?;
        }

        packet_writer.write_packet(
            Vec::new().into(),
            serial,
            PacketWriteEndInfo::EndStream,
            granulepos as u64
        )?;

        let final_duration = granulepos as f32 / 48000.0;
        println!("Final Opus duration: {} seconds", final_duration);

        Ok(())
    }
}