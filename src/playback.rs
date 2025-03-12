use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use hound;

pub fn playback_audio(file_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let host = cpal::default_host();
    let device = host.default_output_device().expect("Failed to get default output device");
    let config = device.default_output_config()?;

    let mut reader = hound::WavReader::open(file_path)?;
    let _spec = reader.spec();
    let sample_format = config.sample_format();

    let stream = match sample_format {
        cpal::SampleFormat::I16 => device.build_output_stream(
            &config.config(),
            move |output: &mut [i16], _| {
                for (sample, out) in reader.samples::<i16>().zip(output.iter_mut()) {
                    *out = sample.unwrap_or(0);
                }
            },
            |err| eprintln!("Error: {:?}", err),
            None,
        )?,
        cpal::SampleFormat::F32 => device.build_output_stream(
            &config.config(),
            move |output: &mut [f32], _| {
                for (sample, out) in reader.samples::<i16>().zip(output.iter_mut()) {
                    // Convert i16 to f32
                    *out = sample.unwrap_or(0) as f32 / i16::MAX as f32;
                }
            },
            |err| eprintln!("Error: {:?}", err),
            None,
        )?,
        cpal::SampleFormat::U16 => device.build_output_stream(
            &config.config(),
            move |output: &mut [u16], _| {
                for (sample, out) in reader.samples::<i16>().zip(output.iter_mut()) {
                    // Convert i16 to u16
                    *out = (sample.unwrap_or(0) as i32 + i16::MAX as i32) as u16;
                }
            },
            |err| eprintln!("Error: {:?}", err),
            None,
        )?,
        _ => return Err("Unsupported sample format".into()),
    };

    stream.play()?;
    println!("Playing audio... Press Enter to stop.");
    let _ = std::io::stdin().read_line(&mut String::new());
    Ok(())
}
