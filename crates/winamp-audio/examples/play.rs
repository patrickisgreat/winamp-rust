//! Headless playback smoke test: `cargo run -p winamp-audio --example play -- FILE [SECONDS]`
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};
use winamp_audio::{Event, Player, Source};

fn main() -> Result<()> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = args
        .first()
        .ok_or_else(|| anyhow!("usage: play FILE [SECONDS]"))?;
    let seconds: f64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(4.0);
    let mut player = Player::new()?;
    player.set_volume(0.2);
    player.load(Source::parse(path), true);
    let start = Instant::now();
    let mut last_half_second = -1i64;
    while start.elapsed().as_secs_f64() < seconds {
        for event in player.poll_events() {
            match event {
                Event::Opened(info) => println!(
                    "opened: {}ch {}Hz {:?}kbps dur={:?} codec={} title={:?}",
                    info.channels,
                    info.sample_rate,
                    info.bitrate_kbps,
                    info.duration,
                    info.codec,
                    info.title
                ),
                Event::Finished => println!("finished at {:?}", player.position()),
                Event::Error(e) => println!("error: {e}"),
                Event::StreamTitle(title) => println!("stream title: {title}"),
            }
        }
        let half_second = player.position().as_millis() as i64 / 500;
        if half_second != last_half_second {
            last_half_second = half_second;
            let vis = player.vis_samples(576);
            let rms = (vis.iter().map(|s| s * s).sum::<f32>() / vis.len() as f32).sqrt();
            println!(
                "t={:.1}s state={:?} rms={:.3}",
                player.position().as_secs_f64(),
                player.state(),
                rms
            );
        }
        if (start.elapsed().as_secs_f64() - 2.0).abs() < 0.02 {
            println!("seeking to 5s");
            player.seek(5.0);
        }
        std::thread::sleep(Duration::from_millis(40));
    }
    Ok(())
}
