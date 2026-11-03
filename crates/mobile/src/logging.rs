use std::fs::{self, File};
use std::io;
use std::path::Path;
use std::sync::Mutex;

use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::MakeWriter;

const LOG_FILE: &str = "log.txt";
const LOG_LIMIT: u64 = 4 * 1024 * 1024;
const FILTER: &str = "warn,mobile=info,foundry=info,codex=warn";
const VERBOSE: &str = "info,mobile=debug,foundry=debug,codex=info";

struct Mirror {
    file: Mutex<Option<File>>,
}

struct Pen<'a>(&'a Mirror);

impl io::Write for Pen<'_> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if let Ok(mut held) = self.0.file.lock()
            && let Some(file) = held.as_mut()
        {
            let _ = file.write_all(bytes);
        }

        io::stderr().write_all(bytes)?;

        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Ok(mut held) = self.0.file.lock()
            && let Some(file) = held.as_mut()
        {
            let _ = file.flush();
        }

        io::stderr().flush()
    }
}

impl<'a> MakeWriter<'a> for Mirror {
    type Writer = Pen<'a>;

    fn make_writer(&'a self) -> Self::Writer {
        Pen(self)
    }
}

pub fn init(verbose: bool) {
    let path = Path::new(LOG_FILE);
    let oversized = fs::metadata(path).is_ok_and(|meta| meta.len() > LOG_LIMIT);
    let file = if oversized { File::create(path) } else { fs::OpenOptions::new().create(true).append(true).open(path) };
    let mirror = Mirror { file: Mutex::new(file.ok()) };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(if verbose { VERBOSE } else { FILTER }));

    let _ = tracing_subscriber::fmt().with_env_filter(filter).with_ansi(false).with_writer(mirror).try_init();
}
