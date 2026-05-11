use ogg::{PacketWriter, writing::PacketWriteEndInfo};
use opus_rs::{Application, OpusEncoder as CodecEncoder};
use std::fs::File;
use std::io::BufWriter;

#[derive(Clone)]
pub struct OpusEncoder {
    bitrate: i32,
    sample_rate: u32,
}

impl OpusEncoder {
    pub fn new() -> Self {
        Self {
            bitrate: 12000, // Default 12kbps
            sample_rate: 48000,
        }
    }

    pub fn set_sample_rate(&mut self, sample_rate: u32) {
        self.sample_rate = sample_rate;
    }

    pub fn set_bitrate(&mut self, bitrate: i32) {
        self.bitrate = bitrate;
    }

    pub fn get_bitrate(&self) -> i32 {
        self.bitrate
    }

    fn validate_settings(&self) -> Result<(), Box<dyn std::error::Error>> {
        if !matches!(self.sample_rate, 8000 | 12000 | 16000 | 24000 | 48000) {
            return Err("Invalid sample rate. Must be 8kHz, 12kHz, 16kHz, 24kHz, or 48kHz".into());
        }

        if self.bitrate < 6000 || self.bitrate > 510000 {
            return Err("Invalid bitrate. Must be between 6kbps and 510kbps".into());
        }

        Ok(())
    }

    fn frame_size(&self) -> usize {
        match self.sample_rate {
            8000 => 160,
            12000 => 240,
            16000 => 320,
            24000 => 480,
            48000 => 960,
            _ => 160,
        }
    }

    pub fn encode_wav_to_opus(&self, input_path: &str, output_path: &str) -> Result<(), Box<dyn std::error::Error>> {
        self.validate_settings()?;
        
        let mut reader = hound::WavReader::open(input_path)?;
        let spec = reader.spec();
        
        let samples: Vec<f32> = if spec.sample_format == hound::SampleFormat::Float {
            reader.samples::<f32>().map(|s| s.unwrap()).collect()
        } else {
            reader.samples::<i16>().map(|s| s.unwrap() as f32 / 32768.0).collect()
        };
        
        let mono_samples: Vec<f32> = if spec.channels == 2 {
            samples.chunks(2)
                .map(|chunk| chunk[0])
                .collect()
        } else {
            samples
        };
        
        let resampled_samples = if spec.sample_rate != self.sample_rate {
            let input_duration = mono_samples.len() as f32 / spec.sample_rate as f32;
            let output_len = (input_duration * self.sample_rate as f32) as usize;
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

        let frame_size = self.frame_size();
        let mut encoder = CodecEncoder::new(self.sample_rate as i32, 1, Application::Audio)
            .map_err(std::io::Error::other)?;
        encoder.bitrate_bps = self.bitrate;
        
        println!("Converting to Opus:");
        println!("  Frame size: {} samples (20ms)", frame_size);
        println!("  Total frames: {}", resampled_samples.len() / frame_size);

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
        id_header.extend_from_slice(&(self.sample_rate).to_le_bytes());  // Input sample rate
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

        let mut input_buffer = vec![0.0f32; frame_size];
        let mut encoded_data = vec![0u8; 1275];
        let mut granulepos = 0i64;

        for chunk in resampled_samples.chunks(frame_size) {
            input_buffer.clear();
            input_buffer.extend(chunk);
            if input_buffer.len() < frame_size {
                input_buffer.resize(frame_size, 0.0);
            }

            let encoded_len = encoder
                .encode(&input_buffer, frame_size, &mut encoded_data)
                .map_err(std::io::Error::other)?;
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

        let final_duration = granulepos as f32 / self.sample_rate as f32;
        println!("Final Opus duration: {} seconds", final_duration);

        Ok(())
    }
}