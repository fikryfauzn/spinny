use std::{env, error::Error, path::PathBuf};

use vdisc_core::{DraftDisc, load_draft, save_draft};

#[cfg(target_os = "linux")]
use vdisc_core::{LinuxAudioPlayer, PlayerState};

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut args = env::args().skip(1);

    match args.next().as_deref() {
        Some("new") => {
            let title = args.next().ok_or("missing CD title")?;
            let output = args.next().ok_or("missing output path")?;

            if args.next().is_some() {
                return Err("too many arguments for `new`".into());
            }

            let draft = DraftDisc::new(title)?;
            let path = PathBuf::from(output);

            save_draft(&path, &draft)?;

            println!("Created draft:");
            println!("  ID:    {}", draft.id());
            println!("  Title: {}", draft.title());
            println!("  Path:  {}", path.display());
        }

        Some("inspect") => {
            let input = args.next().ok_or("missing draft path")?;

            if args.next().is_some() {
                return Err("too many arguments for `inspect`".into());
            }

            let draft = load_draft(input)?;

            println!("Draft CD");
            println!("  ID:       {}", draft.id());
            println!("  Title:    {}", draft.title());
            println!("  Capacity: {}", draft.capacity());
            println!("  Created:  {}", draft.created_at_unix());
        }

        #[cfg(target_os = "linux")]
        Some("play") => {
            let input = args.next().ok_or("missing VDISC path")?;

            if args.next().is_some() {
                return Err("too many arguments for `play`".into());
            }

            let mut player = LinuxAudioPlayer::new();
            player.insert(input)?;

            let title = player
                .disc()
                .map(|disc| disc.title().to_owned())
                .ok_or("player did not retain inserted VDISC")?;
            println!("Playing: {title}");

            player.play_to_end()?;

            if player.state() != PlayerState::Stopped {
                return Err("playback ended in an unexpected state".into());
            }

            println!("Playback complete.");
        }

        _ => {
            eprintln!("Usage:");
            eprintln!("  vdisc new <title> <output.vdraft>");
            eprintln!("  vdisc inspect <input.vdraft>");
            #[cfg(target_os = "linux")]
            eprintln!("  vdisc play <input.vdisc>");

            std::process::exit(2);
        }
    }

    Ok(())
}
