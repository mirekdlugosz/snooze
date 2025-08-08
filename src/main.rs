use std::io::{Write, stdout};
use std::string::String;
use std::process::{ExitCode, Termination};
use std::time::{Duration, Instant};
use std::thread::{self, JoinHandle};

use argh::FromArgs;
use crossbeam_channel::{self, Sender, Receiver};
use crossterm::style::Print;
use crossterm::terminal::{Clear, ClearType};
use crossterm::{cursor, ExecutableCommand, QueueableCommand};
use signal_hook::low_level;
use signal_hook::iterator::{Handle, Signals};
use signal_hook::consts::signal;

use snooze::sum_pause_args;

const REFRESH_TIME: Duration = Duration::from_secs(1);

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


enum SnoozeMessage {
    PrintTime,
    Terminate(i32),
}


fn install_signal_handlers(
    loop_sender: Sender<SnoozeMessage>,
    ui_sender: Sender<SnoozeMessage>
) -> Option<(Handle, JoinHandle<()>)> {
    let known_signals = [
        signal::SIGUSR1,
        signal::SIGTERM,
        signal::SIGQUIT,
        signal::SIGINT,
    ];
    let mut signals = Signals::new(known_signals).ok()?;
    let handle = signals.handle();
    let thread = thread::spawn(move || {
        for signal in &mut signals {
            match signal {
                signal::SIGUSR1 => {
                    let _ = ui_sender.send(SnoozeMessage::PrintTime);
                },
                signal::SIGTERM | signal::SIGQUIT | signal::SIGINT => {
                    let _ = ui_sender.send(SnoozeMessage::Terminate(signal));
                    let _ = loop_sender.send(SnoozeMessage::Terminate(signal));
                }
                _ => (),
            }
        }
    });
    Some((handle, thread))
}


fn start_ui(end_time: Instant, ui_receiver: Receiver<SnoozeMessage>) -> JoinHandle<()> {
    let mut stdout = stdout();
    thread::spawn(move || {
        loop {
            match ui_receiver.recv() {
                Ok(SnoozeMessage::Terminate(_)) | Err(_) => break,
                Ok(SnoozeMessage::PrintTime) => {
                    let remaining = end_time - Instant::now();
                    stdout
                        .queue(Clear(ClearType::CurrentLine)).unwrap()
                        .queue(cursor::Hide).unwrap()
                        .queue(cursor::MoveToColumn(0)).unwrap()
                        .queue(Print(format!("Left: {remaining:?}"))).unwrap()
                        .flush().unwrap();
                }
            }
        }
        stdout.execute(cursor::Show).unwrap();
        println!();
    })
}

// FIXME: jakie errory zwraca sleep?
#[repr(u8)]
pub enum SnoozeResult {
    Good = 0,
    UserError = 1,
    OsError = 2,
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

    let num_args: Vec<&str> = parsed_args.number.iter().map(String::as_str).collect();
    let Some(desired_runtime) = sum_pause_args(&num_args) else {
        if parsed_args.number.is_empty() {
            println!("Missing mandatory arguments");
        } else {
            println!("Invalid time interval supplied");
        }
        println!("Run snooze --help for more information.");
        return SnoozeResult::UserError;
    };

    let end_time = start_time + desired_runtime;
    println!("{desired_runtime:?}");

    // jakaś logika, że mniej niż sekunda to po prostu śpimy z quiet

    let (loop_sender, loop_receiver) = crossbeam_channel::unbounded();
    let (ui_sender, ui_receiver) = crossbeam_channel::unbounded();

    let Some((signals_handle, signals_thread)) = install_signal_handlers(loop_sender, ui_sender.clone()) else {
        println!("Couldn't create signal handlers");
        return SnoozeResult::OsError;
    };

    let ui_thread = start_ui(end_time, ui_receiver);

    let mut close_signal: Option<i32> = None;

    loop {
        match loop_receiver.try_recv() {
            Ok(SnoozeMessage::Terminate(signal_)) => {
                close_signal = Some(signal_);
                break;
            },
            Ok(SnoozeMessage::PrintTime) => (),
            // FIXME: ok to ignore errors?
            Err(_) => (),
        }
        let remaining = end_time - Instant::now();
        if remaining.is_zero() {
            break;
        }
        if !parsed_args.quiet {
            let _ = ui_sender.try_send(SnoozeMessage::PrintTime);
        }
        thread::sleep(remaining.min(REFRESH_TIME));
    }

    let _ = ui_sender.send(SnoozeMessage::Terminate(0));

    signals_handle.close();
    ui_thread.join().unwrap();
    signals_thread.join().unwrap();
    if let Some(signal_) = close_signal {
        let _ = low_level::emulate_default_handler(signal_);
    }

    SnoozeResult::Good
}
