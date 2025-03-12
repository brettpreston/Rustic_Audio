use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use hound;

pub fn record_audio(file_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let host = cpal::default_host();
    let device = host.default_input_device().expect("Failed to get default input device");
    let config = device.default_input_config()?;

    let sample_format = config.sample_format();
    let sample_rate = config.sample_rate().0;
    let channels = config.channels();

    println!("Recording to {} with format {:?}", file_path, sample_format);
    let spec = hound::WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };
    let mut writer = hound::WavWriter::create(file_path, spec)?;

    let stream = match sample_format {
        cpal::SampleFormat::I16 => device.build_input_stream(
            &config.config(),
            move |data: &[i16], _| {
                for &sample in data {
                    writer.write_sample(sample).unwrap();
                }
            },
            |err| eprintln!("Error: {:?}", err),
            None,
        )?,
        cpal::SampleFormat::F32 => device.build_input_stream(
            &config.config(),
            move |data: &[f32], _| {
                for &sample in data {
                    // Convert f32 to i16
                    let sample = (sample * i16::MAX as f32) as i16;
                    writer.write_sample(sample).unwrap();
                }
            },
            |err| eprintln!("Error: {:?}", err),
            None,
        )?,
        cpal::SampleFormat::U16 => device.build_input_stream(
            &config.config(),
            move |data: &[u16], _| {
                for &sample in data {
                    // Convert u16 to i16
                    let sample = sample as i16 - i16::MAX;
                    writer.write_sample(sample).unwrap();
                }
            },
            |err| eprintln!("Error: {:?}", err),
            None,
        )?,
        _ => return Err("Unsupported sample format".into()),
    };

    stream.play()?;
    println!("Recording... Press Enter to stop.");
    let _ = std::io::stdin().read_line(&mut String::new());
    drop(stream);
    Ok(())
}
