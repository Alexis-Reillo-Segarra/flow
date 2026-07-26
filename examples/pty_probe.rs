//! Sonda de diagnóstico: lanza un comando en un PTY exactamente como hace
//! `agent.rs` y vuelca por stdout lo que llega, con marca de tiempo.
//!
//!     cargo run --example pty_probe -- "cargo test --color always"

use std::io::Read;
use std::time::Instant;

use portable_pty::{native_pty_system, CommandBuilder, PtySize};

fn main() -> anyhow::Result<()> {
    let cmdline = std::env::args().nth(1).unwrap_or_else(|| "dir".to_owned());
    let cwd = std::env::current_dir()?;
    println!("[probe] cmdline = {cmdline:?}");
    println!("[probe] cwd     = {}", cwd.display());

    let pty = native_pty_system();
    let pair = pty.openpty(PtySize {
        rows: 32,
        cols: 110,
        pixel_width: 0,
        pixel_height: 0,
    })?;

    let mut cmd = if cfg!(windows) {
        let mut c = CommandBuilder::new("cmd.exe");
        c.arg("/C");
        c.arg(&cmdline);
        c
    } else {
        let mut c = CommandBuilder::new("/bin/sh");
        c.arg("-c");
        c.arg(&cmdline);
        c
    };
    cmd.cwd(&cwd);
    cmd.env("TERM", "xterm-256color");

    let mut child = pair.slave.spawn_command(cmd)?;
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader()?;
    let start = Instant::now();

    let handle = std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        let mut total = 0usize;
        loop {
            match reader.read(&mut buf) {
                Ok(0) => {
                    println!("\n[probe] EOF tras {total} bytes");
                    break;
                }
                Ok(n) => {
                    total += n;
                    println!(
                        "[probe] +{n} bytes a los {:?} :: {:?}",
                        start.elapsed(),
                        String::from_utf8_lossy(&buf[..n.min(160)])
                    );
                }
                Err(e) => {
                    println!("[probe] ERROR de lectura: {e}");
                    break;
                }
            }
        }
        total
    });

    let status = child.wait()?;
    println!("[probe] el hijo salió con {}", status.exit_code());
    let total = handle.join().unwrap();
    println!(
        "[probe] total leído: {total} bytes en {:?}",
        start.elapsed()
    );
    Ok(())
}
