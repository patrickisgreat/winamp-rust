//! Headless playback smoke test: `cargo run -p winamp-audio --example play -- FILE [SECONDS]`
use std::time::{Duration, Instant};
use winamp_audio::{Event, Player, Source};

fn main() {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));
    let args: Vec<String> = std::env::args().skip(1).collect();
    let path = args.first().expect("file");
    let secs: f64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(4.0);
    let mut player = Player::new().expect("player");
    player.set_volume(0.2);
    player.load(Source::from_str(path), true);
    let start = Instant::now();
    let mut last = -1i64;
    while start.elapsed().as_secs_f64() < secs {
        for ev in player.poll_events() {
            match ev {
                Event::Opened(info) => println!("opened: {}ch {}Hz {:?}kbps dur={:?} codec={} title={:?}", info.channels, info.sample_rate, info.bitrate_kbps, info.duration, info.codec, info.title),
                Event::Finished => println!("finished at {:?}", player.position()),
                Event::Error(e) => println!("error: {e}"),
                Event::StreamTitle(t) => println!("stream title: {t}"),
            }
        }
        let p = player.position().as_millis() as i64 / 500;
        if p != last {
            last = p;
            let vis = player.vis_samples(576);
            let rms = (vis.iter().map(|s| s * s).sum::<f32>() / vis.len() as f32).sqrt();
            println!("t={:.1}s state={:?} rms={:.3}", player.position().as_secs_f64(), player.state(), rms);
        }
        if (start.elapsed().as_secs_f64() - 2.0).abs() < 0.02 {
            println!("seeking to 5s");
            player.seek(5.0);
        }
        std::thread::sleep(Duration::from_millis(40));
    }
}
