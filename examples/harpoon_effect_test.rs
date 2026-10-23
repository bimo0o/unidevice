use std::{sync::Arc, time::Duration};
use unidevice::{
    corsair,
    desktop::{Command, Event, Worker},
    effects::{Effect, Pattern},
    error::{Error, Result},
};
fn run() -> Result<()> {
    let mut session = corsair::open_harpoon()?;
    let before = session.state()?;
    drop(session);
    let worker = Worker::start(Arc::new(|| {}))?;
    worker.send(Command::Scan)?;
    let Event::Devices(rows) = worker
        .events
        .recv_timeout(Duration::from_secs(5))
        .map_err(|_| Error::Timeout)?
    else {
        return Err(Error::Protocol("expected scan".into()));
    };
    let row = rows?
        .into_iter()
        .find(|r| r.eligible && r.title.contains("Harpoon"))
        .ok_or(Error::NotFound)?;
    worker.send(Command::Probe(row))?;
    let Event::Ready(ready) = worker
        .events
        .recv_timeout(Duration::from_secs(5))
        .map_err(|_| Error::Timeout)?
    else {
        return Err(Error::Protocol("expected probe".into()));
    };
    ready?;
    worker.send(Command::Effect(Some(Effect {
        pattern: Pattern::Gradient,
        first: before.rgb,
        second: [0, 0, 255],
        seconds: 4.0,
    })))?;
    let mut frames = 0;
    while frames < 12 {
        match worker
            .events
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| Error::Timeout)?
        {
            Event::Effect(result) => {
                result?;
            }
            Event::Frame(color) => {
                frames += 1;
                println!("frame {frames}: {color:?}");
            }
            _ => return Err(Error::Protocol("unexpected animation event".into())),
        }
    }
    worker.send(Command::Effect(None))?;
    loop {
        match worker
            .events
            .recv_timeout(Duration::from_secs(5))
            .map_err(|_| Error::Timeout)?
        {
            Event::Effect(result) => {
                if !result? {
                    break;
                }
            }
            Event::Frame(_) => {}
            _ => return Err(Error::Protocol("unexpected stop event".into())),
        }
    }
    if worker
        .events
        .recv_timeout(Duration::from_millis(700))
        .is_ok()
    {
        return Err(Error::Safety("frame after stop".into()));
    }
    worker.send(Command::SetRgb(before.rgb))?;
    let Event::Applied(result) = worker
        .events
        .recv_timeout(Duration::from_secs(5))
        .map_err(|_| Error::Timeout)?
    else {
        return Err(Error::Protocol("expected restoration".into()));
    };
    let restored = result?;
    if restored.current_dpi != before.x || restored.rgb != before.rgb {
        return Err(Error::Protocol("restoration mismatch".into()));
    }
    println!(
        "Stopped; restored DPI {} RGB {:?}; no frame for 700ms",
        restored.current_dpi, restored.rgb
    );
    Ok(())
}
fn main() {
    if let Err(e) = run() {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
