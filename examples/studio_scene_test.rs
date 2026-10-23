use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use unidevice::{
    corsair,
    desktop::{demo_rows, Command, Event, SceneSelection, Worker},
    effects::{Effect, Keyframe, Pattern},
    error::{Error, Result},
};
fn event(worker: &Worker) -> Result<Event> {
    worker
        .events
        .recv_timeout(Duration::from_secs(6))
        .map_err(|_| Error::Timeout)
}
fn run() -> Result<()> {
    let mut probe = corsair::open_harpoon()?;
    let before = probe.state()?;
    drop(probe);
    let worker = Worker::start(Arc::new(|| {}))?;
    worker.send(Command::Scan)?;
    let Event::Devices(rows) = event(&worker)? else {
        return Err(Error::Protocol("scan event".into()));
    };
    let row = rows?
        .into_iter()
        .find(|r| r.eligible && r.identity() == Some((0x1b1c, 0x1b75)))
        .ok_or(Error::NotFound)?;
    let targets = vec![
        SceneSelection {
            row: row.clone(),
            brightness: 1.0,
            phase_ms: 0,
        },
        SceneSelection {
            row: demo_rows().remove(1),
            brightness: 0.7,
            phase_ms: 25,
        },
    ];
    let effect = Effect {
        pattern: Pattern::Timeline,
        seconds: 4.0,
        keyframes: vec![
            Keyframe {
                at: 0.0,
                color: before.rgb,
            },
            Keyframe {
                at: 0.33,
                color: [0, 0, 255],
            },
            Keyframe {
                at: 0.66,
                color: [106, 205, 181],
            },
        ],
        ..Effect::default()
    };
    worker.send(Command::SceneStart(targets, effect))?;
    let Event::Scene(ready) = event(&worker)? else {
        return Err(Error::Protocol("scene start".into()));
    };
    ready?;
    let start = Instant::now();
    let mut last_print = 0;
    let (real_frames, demo_frames) = loop {
        if start.elapsed() > Duration::from_secs(15) {
            return Err(Error::Timeout);
        }
        match event(&worker)? {
            Event::SceneFrame(status) => {
                let real = &status[0];
                if real.dpi != before.x {
                    return Err(Error::Safety("scene modified DPI".into()));
                }
                if real.frames >= last_print + 30 {
                    last_print = real.frames;
                    println!(
                        "real_verified_frames={} virtual_verified_frames={}",
                        real.frames, status[1].frames
                    );
                }
                if real.frames >= 120 {
                    break (real.frames, status[1].frames);
                }
            }
            Event::Scene(result) => {
                result?;
            }
            _ => return Err(Error::Protocol("unexpected scene event".into())),
        }
    };
    let elapsed = start.elapsed();
    worker.send(Command::SceneStop)?;
    loop {
        match event(&worker)? {
            Event::Scene(result) => {
                if result?.is_empty() {
                    break;
                }
            }
            Event::SceneFrame(_) => {}
            _ => return Err(Error::Protocol("scene stop".into())),
        }
    }
    if worker
        .events
        .recv_timeout(Duration::from_millis(300))
        .is_ok()
    {
        return Err(Error::Safety("frame after stop".into()));
    }
    worker.send(Command::Probe(row))?;
    let Event::Ready(ready) = event(&worker)? else {
        return Err(Error::Protocol("restore probe".into()));
    };
    ready?;
    worker.send(Command::Transfer {
        dpi: None,
        rgb: Some(before.rgb),
    })?;
    let Event::Transfer(restored) = event(&worker)? else {
        return Err(Error::Protocol("profile transfer".into()));
    };
    let (caps, fields) = restored?;
    if caps.current_dpi != before.x || caps.rgb != before.rgb {
        return Err(Error::Protocol("restored state".into()));
    }
    println!("scene_seconds={:.3} real_frames={} real_fps={:.2} virtual_frames={} stopped_and_restored_dpi={} rgb={:?} fields={:?}",elapsed.as_secs_f64(),real_frames,real_frames as f64/elapsed.as_secs_f64(),demo_frames,caps.current_dpi,caps.rgb,fields);
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
