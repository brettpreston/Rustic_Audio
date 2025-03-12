mod record;
mod playback;

use record::record_audio;
use playback::playback_audio;
use cpal::traits::{HostTrait, DeviceTrait};

fn main() {
    // Print available audio devices for debugging
    let host = cpal::default_host();
    
    println!("Available input devices:");
    for device in host.input_devices().expect("Failed to get input devices") {
        println!("  {}", device.name().unwrap_or_else(|_| "Unknown".to_string()));
    }

    println!("\nAvailable output devices:");
    for device in host.output_devices().expect("Failed to get output devices") {
        println!("  {}", device.name().unwrap_or_else(|_| "Unknown".to_string()));
    }

    println!("\nAudio Recorder and Player CLI");
    println!("Enter 'record' to record audio or 'playback' to play audio:");

    let mut input = String::new();
    std::io::stdin().read_line(&mut input).unwrap();
    let command = input.trim();

    match command {
        "record" => {
            println!("Starting recording...");
            match record_audio("output.wav") {
                Ok(_) => println!("Recording completed successfully"),
                Err(e) => eprintln!("Error during recording: {:?}", e),
            }
        }
        "playback" => {
            println!("Starting playback...");
            match playback_audio("output.wav") {
                Ok(_) => println!("Playback completed successfully"),
                Err(e) => eprintln!("Error during playback: {:?}", e),
            }
        }
        _ => println!("Unknown command"),
    }
}
