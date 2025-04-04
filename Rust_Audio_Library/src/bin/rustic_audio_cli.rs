use rustic_audio_tool::RusticAudio;
use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();
    
    if args.len() < 2 {
        println!("Usage: rustic_audio_cli [record|process|play] [file_path]");
        return;
    }
    
    let mut audio = RusticAudio::new();
    
    match args[1].as_str() {
        "record" => {
            if args.len() < 3 {
                println!("Please provide an output file path");
                return;
            }
            
            println!("Recording to {}. Press Enter to stop...", args[2]);
            if let Err(e) = audio.start_recording(&args[2]) {
                println!("Error starting recording: {}", e);
                return;
            }
            
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).unwrap();
            
            if let Err(e) = audio.stop_recording() {
                println!("Error stopping recording: {}", e);
            }
        },
        "process" => {
            if args.len() < 4 {
                println!("Please provide input and output file paths");
                return;
            }
            
            println!("Processing {} to {}", args[2], args[3]);
            if let Err(e) = audio.process_file(&args[2], &args[3]) {
                println!("Error processing file: {:?}", e);
            }
        },
        "play" => {
            if args.len() < 3 {
                println!("Please provide a file path to play");
                return;
            }
            
            println!("Playing {}. Press Enter to stop...", args[2]);
            if let Err(e) = audio.play_processed_wav(&args[2]) {
                println!("Error playing file: {}", e);
                return;
            }
            
            let mut input = String::new();
            std::io::stdin().read_line(&mut input).unwrap();
            
            if let Err(e) = audio.stop_playback() {
                println!("Error stopping playback: {}", e);
            }
        },
        _ => println!("Unknown command: {}", args[1]),
    }
}