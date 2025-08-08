use std::string::String;
use std::process::{ExitCode, Termination};
use std::time::{Duration, Instant};
use std::thread;

use argh::FromArgs;
use crossterm::style::Print;
use crossterm::terminal::{Clear, ClearType};
use std::io::{Write, stdout};
use crossterm::{cursor, ExecutableCommand, QueueableCommand};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use snooze::sum_pause_args;

const REFRESH_TIME: Duration = Duration::from_secs(1);
const HELP_REFERENCE: &str = "Run snooze --help for more information.";

/** Pause for specified time.
Like sleep, but print how much time is still left.
Positional arguments specify how long to pause. They need not to be an integer.
A number may be followed by a suffix: 's' for seconds (default if no suffix is
provided), 'm' for minutes, 'h' for hours or 'd' for days. Multiple arguments
are summed.
*/
#[derive(FromArgs)]
#[argh(help_triggers("-h", "--help", "help"))]
struct SnoozeArgs {
    /// sleep compatibility mode - don't output how much time is still left
    #[argh(switch, short='q')]
    quiet: bool,

    /// display wall-clock time when snooze is expected to finish
    //#[argh(switch, short='e')]
    //with_end: bool,

    /// time to pause
    #[argh(positional, greedy)]
    number: Vec<String>
}


// FIXME: jakie errory zwraca sleep?
#[repr(u8)]
pub enum SnoozeResult {
    Good = 0,
    Bad = 1,
    Abort = 120,
}

impl Termination for SnoozeResult {
    fn report(self) -> ExitCode {
        ExitCode::from(self as u8)
    }
}

fn main() -> SnoozeResult {
    let start_time = Instant::now();

    let parsed_args: SnoozeArgs = argh::from_env();

    if parsed_args.number.is_empty() {
        println!("Missing mandatory arguments");
        println!("{HELP_REFERENCE}");
        return SnoozeResult::Bad;
    }

    let nums: Vec<&str> = parsed_args.number.iter().map(String::as_str).collect();

    let Some(desired_runtime) = sum_pause_args(&nums) else {
        println!("Invalid time interval supplied");
        println!("{HELP_REFERENCE}");
        return SnoozeResult::Bad;
    };
    let end_time = start_time + desired_runtime;
    println!("{desired_runtime:?}");

    // jakaś logika, że mniej niż sekunda to po prostu śpimy z quiet

    let mut stdout = stdout();
    let sigusr_received = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(signal_hook::consts::SIGUSR1, Arc::clone(&sigusr_received)).unwrap();

    loop {
        let remaining = end_time - Instant::now();
        if remaining.is_zero() {
            break;
        }
        let should_print = sigusr_received.load(Ordering::Relaxed);
        if !parsed_args.quiet || should_print {
            stdout
                .queue(Clear(ClearType::CurrentLine)).unwrap()
                .queue(cursor::Hide).unwrap()
                .queue(cursor::MoveToColumn(0)).unwrap()
                .queue(Print(format!("Left: {remaining:?}"))).unwrap()
                .flush().unwrap();
            sigusr_received.store(false, Ordering::Relaxed);
        }
        thread::sleep(remaining.min(REFRESH_TIME));
    }

    stdout.execute(cursor::Show).unwrap();
    println!("");

    SnoozeResult::Good
}
