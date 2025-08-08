use std::process::{ExitCode, Termination};
use std::time::{Duration, Instant};
use std::thread;

use argh::FromArgs;

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
    //#[argh(switch, short='q')]
    //quiet: bool,

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
    let nums: Vec<&str> = parsed_args.number.iter().map(|s| s.as_str()).collect();

    // FIXME: error into SnoozeResult
    let desired_runtime = sum_pause_args(&nums).unwrap();
    println!("{:?}", desired_runtime);
    // invalid arg - any - is SnoozeResult::Bad
    //parsed_args.number.iter().try_fold(init, f)

    // calculate from args
    //let desired_runtime = Duration::from_millis(2500);
    let end_time = start_time + desired_runtime;

    // jakaś logika, że mniej niż sekunda to po prostu śpimy z quiet

    loop {
        let remaining = end_time - Instant::now();
        if remaining.is_zero() {
            break;
        }
        println!("Left: {:?}", remaining);
        thread::sleep(remaining.min(REFRESH_TIME));
    }

    SnoozeResult::Good
}
