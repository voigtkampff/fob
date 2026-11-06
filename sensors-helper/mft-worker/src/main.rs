#[path = "../../../src-tauri/src/mft_records.rs"]
mod mft_records;
#[path = "../../../src-tauri/src/mft_wire.rs"]
mod mft_wire;
use std::io::{self, Write};

fn main() {
    if run().is_err() {
        std::process::exit(1);
    }
}
fn run() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 2 || args[1].len() != 1 || !args[1].as_bytes()[0].is_ascii_uppercase() {
        return Err(io::Error::other("drive"));
    }
    #[cfg(windows)]
    std::thread::spawn(|| {
        use std::io::Read;
        let _ = std::io::stdin().read(&mut [0u8; 1]);
        std::process::exit(2);
    });
    let root = format!("{}:\\", args[1]);
    if mft_wire::request(&root).is_none() {
        return Err(io::Error::other("drive"));
    }
    let mut out = io::BufWriter::with_capacity(64 * 1024, io::stdout().lock());
    out.write_all(mft_wire::MAGIC)?;
    #[cfg(windows)]
    let result = {
        use windows::Win32::Storage::FileSystem::GetDriveTypeW;
        if unsafe { GetDriveTypeW(&windows::core::HSTRING::from(&root)) } != 3 {
            Err(io::Error::other("drive"))
        } else {
            let (mut count, mut names) = (0usize, 0usize);
            mft_records::read_volume(std::path::Path::new(&root), |link| {
                if let Some(link) = link {
                    count += 1;
                    names += link.name.len();
                    if count > mft_wire::MAX_LINKS || names > mft_wire::MAX_NAMES {
                        return Err(io::Error::other("limit"));
                    }
                    mft_wire::frame(&mut out, 1, &serde_json::to_vec(&link)?)
                } else {
                    mft_wire::frame(&mut out, 4, &[])?;
                    out.flush()
                }
            })
        }
    };
    #[cfg(not(windows))]
    let result: io::Result<()> = Err(io::Error::other("platform"));
    match result {
        Ok(()) => mft_wire::frame(&mut out, 2, &[])?,
        Err(_) => mft_wire::frame(&mut out, 3, b"read_failed")?,
    }
    out.flush()
}
