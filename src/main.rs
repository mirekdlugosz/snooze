use std::env;
use std::io::{Write, stdin, stdout};
use std::process::{ExitCode, Termination};
use std::string::String;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use argh::FromArgs;
use crossbeam_channel::{self, Receiver, Sender};
use crossterm::style::Print;
use crossterm::terminal::{Clear, ClearType};
use crossterm::{ExecutableCommand, QueueableCommand, cursor};
use nix::unistd;
use signal_hook::consts::signal;
use signal_hook::iterator::{Handle, Signals};
use signal_hook::low_level;

use snooze::{format_remaining_time, sum_pause_args, wall_clock_end_time};

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
    #[argh(switch, short = 'q')]
    quiet: bool,

    /// display wall-clock time when snooze is expected to finish
    #[argh(switch, short = 't')]
    only_timer: bool,

    /// time to pause
    #[argh(positional, greedy)]
    number: Vec<String>,
}

enum SnoozeMessage {
    PrintTime,
    Suspend,
    Terminate(i32),
}

fn install_signal_handlers(
    loop_sender: Sender<SnoozeMessage>,
    ui_sender: Sender<SnoozeMessage>,
) -> Option<(Handle, JoinHandle<()>)> {
    let known_signals = [
        signal::SIGUSR1,
        signal::SIGTSTP,
        signal::SIGTERM,
        signal::SIGQUIT,
        signal::SIGINT,
    ];
    let mut signals = Signals::new(known_signals).ok()?;
    let handle = signals.handle();
    let thread = thread::spawn(move || {
        for signalid in &mut signals {
            match signalid {
                signal::SIGUSR1 => {
                    let _ = ui_sender.send(SnoozeMessage::PrintTime);
                }
                signal::SIGTSTP => {
                    let _ = ui_sender.send(SnoozeMessage::Suspend);
                    let _ = loop_sender.send(SnoozeMessage::Suspend);
                }
                signal::SIGTERM | signal::SIGQUIT | signal::SIGINT => {
                    let _ = ui_sender.send(SnoozeMessage::Terminate(signalid));
                    let _ = loop_sender.send(SnoozeMessage::Terminate(signalid));
                }
                _ => (),
            }
        }
    });
    Some((handle, thread))
}

fn is_foreground() -> bool {
    unistd::tcgetpgrp(stdin())
        .ok()
        .is_some_and(|pid| pid == unistd::getpgrp())
}

fn print_remaining_time(msg: &str) -> std::io::Result<()> {
    let mut stdout = stdout();
    stdout
        .queue(cursor::Hide)?
        .queue(Clear(ClearType::CurrentLine))?
        .queue(cursor::MoveToColumn(0))?
        .queue(Print(msg))?
        .flush()?;
    Ok(())
}

fn start_ui(
    end_time: Instant,
    formatted_end_time: String,
    ui_receiver: Receiver<SnoozeMessage>,
) -> JoinHandle<()> {
    let mut stdout = stdout();
    thread::spawn(move || {
        let mut did_print = false;
        let mut clean_exit = true;
        loop {
            match ui_receiver.recv() {
                Ok(SnoozeMessage::Terminate(signal)) => {
                    clean_exit = signal == 0;
                    break;
                }
                Ok(SnoozeMessage::Suspend) => {
                    let _ = stdout.execute(cursor::Show);
                }
                Ok(SnoozeMessage::PrintTime) => {
                    if !is_foreground() {
                        continue;
                    }

                    let remaining = end_time - Instant::now();
                    let formatted_remaining = format_remaining_time(remaining);
                    let msg = format!("\t{formatted_remaining}\t{formatted_end_time}");
                    if print_remaining_time(msg.as_str()).is_ok() {
                        did_print = true;
                    }
                }
                Err(_) => break,
            }
        }
        if clean_exit && did_print && is_foreground() {
            println!();
        }
        let _ = stdout.execute(cursor::Show);
    })
}

#[repr(u8)]
pub enum SnoozeResult {
    Good = 0,
    UserError = 1,
    OsError = 2,
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
    let formatted_end_time = (!parsed_args.only_timer)
        .then(|| wall_clock_end_time(desired_runtime))
        .flatten()
        .unwrap_or_default();

    let short_sleep = REFRESH_TIME > desired_runtime;
    let invoked_as_sleep = env::current_exe()
        .ok()
        .and_then(|p| p.file_name().map(|fname| fname == "sleep"))
        .unwrap_or(false);
    let quiet_mode = parsed_args.quiet || short_sleep || invoked_as_sleep;

    let (loop_sender, loop_receiver) = crossbeam_channel::unbounded();
    let (ui_sender, ui_receiver) = crossbeam_channel::unbounded();

    let Some((signals_handle, signals_thread)) =
        install_signal_handlers(loop_sender, ui_sender.clone())
    else {
        println!("Couldn't create signal handlers");
        return SnoozeResult::OsError;
    };

    let ui_thread = start_ui(end_time, formatted_end_time, ui_receiver);

    let mut close_signal: Option<i32> = None;

    loop {
        match loop_receiver.try_recv() {
            Ok(SnoozeMessage::Suspend) => {
                let _ = low_level::emulate_default_handler(signal::SIGTSTP);
            }
            Ok(SnoozeMessage::Terminate(signal)) => {
                close_signal = Some(signal);
                break;
            }
            Ok(_) | Err(_) => (),
        }
        let remaining = end_time - Instant::now();
        if remaining.is_zero() {
            break;
        }
        if !quiet_mode {
            let _ = ui_sender.try_send(SnoozeMessage::PrintTime);
        }
        thread::sleep(remaining.min(REFRESH_TIME));
    }

    let _ = ui_sender.send(SnoozeMessage::Terminate(close_signal.unwrap_or(0)));
    signals_handle.close();
    let _ = ui_thread.join();
    let _ = signals_thread.join();
    if let Some(signal_) = close_signal {
        let _ = low_level::emulate_default_handler(signal_);
    }

    SnoozeResult::Good
}
