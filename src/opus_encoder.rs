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
            bitrate: 12000,
        }
    }

    fn resample(input: &[f32], input_rate: u32, output_rate: u32) -> Vec<f32> {
        let input_duration = input.len() as f32 / input_rate as f32;
        let output_len = (input_duration * output_rate as f32) as usize;
        
        println!("Resampling:");
        println!("  Input samples: {}, rate: {}", input.len(), input_rate);
        println!("  Input duration: {} seconds", input_duration);
        println!("  Target rate: {}", output_rate);
        println!("  Output length needed: {}", output_len);
        
        let mut output = Vec::with_capacity(output_len);
        let scale = (input.len() - 1) as f32 / (output_len - 1) as f32;
        
        for i in 0..output_len {
            let pos = i as f32 * scale;
            let index = pos as usize;
            output.push(input[index]);
        }
        
        println!("  Output samples: {}", output.len());
        println!("  Output duration: {} seconds", output.len() as f32 / output_rate as f32);
        
        output
    }

    pub fn encode_wav_to_opus(&self, wav_path: &str, opus_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut reader = hound::WavReader::open(wav_path)?;
        let spec = reader.spec();

        println!("WAV file specs:");
        println!("  Sample rate: {}", spec.sample_rate);
        println!("  Channels: {}", spec.channels);
        println!("  Bits per sample: {}", spec.bits_per_sample);

        // Read all samples and convert to f32
        let samples: Vec<f32> = if spec.sample_format == hound::SampleFormat::Float {
            reader.samples::<f32>().map(|s| s.unwrap()).collect()
        } else {
            reader.samples::<i16>()
                .map(|s| s.unwrap() as f32 / 32768.0)
                .collect()
        };

        let input_duration = samples.len() as f32 / spec.sample_rate as f32;
        println!("Input file duration: {} seconds", input_duration);

        // Resample to 48kHz if needed
        let resampled_samples = if spec.sample_rate != 48000 {
            Self::resample(&samples, spec.sample_rate, 48000)
        } else {
            samples
        };

        let resampled_duration = resampled_samples.len() as f32 / 48000.0;
        println!("Resampled duration: {} seconds", resampled_duration);

        // Convert back to i16 for Opus encoding
        let samples_i16: Vec<i16> = resampled_samples.iter()
            .map(|&s| (s * 32767.0).min(32767.0).max(-32768.0) as i16)
            .collect();

        println!("Converting to Opus:");
        println!("  Frame size: 960 samples (20ms at 48kHz)");
        println!("  Total frames: {}", samples_i16.len() / 960);

        let mut encoder = audiopus::coder::Encoder::new(
            SampleRate::Hz48000,
            self.channels,
            Application::Audio
        )?;

        encoder.set_bitrate(Bitrate::BitsPerSecond(self.bitrate))?;

        let file = BufWriter::new(File::create(opus_path)?);
        let serial = rand::random();
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